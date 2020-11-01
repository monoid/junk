use std::borrow::Cow;
use std::error::Error;
use std::future::Future;
use std::io;
/// Special LogWriter that writes commands in batches.
/// Batching is both record count and timeout-based.
use std::sync::Arc;
use std::time::Duration;

use crate::storage::{self, SimpleFileWALError};
use async_trait::async_trait;
use futures_util::future::{abortable, AbortHandle};
use thiserror::Error;
use tokio::task::JoinHandle;
use tokio::{sync, time};

// TODO: pass some context for Error handling.
// Define Command trait that both provides context and
// defines data to store.  Box<dyn Command>, like this.
#[derive(Error, Debug)]
pub enum BatchLogError {
    #[error(transparent)]
    Nested(Arc<SimpleFileWALError>),
    #[error("{file}:{line} CANTHAPPEN: {msg}: {nested}")]
    CantHappen {
        file: &'static str,
        line: u32,
        msg: Cow<'static, str>,
        nested: Arc<Box<dyn Error + Send + Sync + 'static>>,
    },
}

struct Queue<AWAL: storage::AsyncWAL, T> {
    wal: AWAL,
    index_buf: Vec<AWAL::CommandPos>,
    data_buf: Vec<(T, sync::oneshot::Sender<Result<T, BatchLogError>>)>,
    flusher: Option<(JoinHandle<()>, AbortHandle)>,
}

impl<
        AWAL: storage::AsyncWAL<Error = SimpleFileWALError> + Send + 'static,
        T: Sync + Send + 'static,
    > Queue<AWAL, T>
{
    fn new(wal: AWAL, capacity: usize) -> Self {
        Self {
            wal,
            index_buf: Vec::with_capacity(capacity),
            data_buf: Vec::with_capacity(capacity),
            flusher: None,
        }
    }

    async fn flush(guard: &mut sync::OwnedMutexGuard<Queue<AWAL, T>>) -> Result<(), BatchLogError> {
        let queue: &mut Queue<AWAL, T> = &mut *guard;
        let index_buf = &mut queue.index_buf;
        let wal = &mut queue.wal;
        let indices_res = wal.indices(index_buf).await;
        index_buf.clear();

        match indices_res {
            Ok(()) => {
                // TODO: Possible improvement: replace Vec with new one, and
                // send data in another thread.  There are no fatal errors.
                for (val, tx) in guard.data_buf.drain(..) {
                    // There is no point of handling the .send result.  The
                    // receiver has gone?  I couldn't care less.
                    let _ = tx.send(Ok(val));
                }
                Ok(())
            }
            Err(e) => {
                let e = Arc::new(e);
                for (_, tx) in guard.data_buf.drain(..) {
                    // There is no point of handling the .send result.  The
                    // receiver has gone?  I couldn't care less.

                    // TODO error context for Nested?
                    let _ = tx.send(Err(BatchLogError::Nested(e.clone())));
                }
                Err(BatchLogError::Nested(e))
            }
        }
    }

    fn get_flusher(
        queue: Arc<sync::Mutex<Self>>,
        flush_timeout: Duration,
    ) -> (JoinHandle<()>, AbortHandle) {
        let delay = time::delay_for(flush_timeout);
        let (delay, handle) = abortable(delay);

        let flusher = async move {
            if delay.await.is_ok() {
                let mut guard = queue.lock_owned().await;
                // TODO what to do with the error?  Set it somewhere.
                let _ = Queue::flush(&mut guard).await;
                // Dropping nor JoinHandle nor AbortHandle does not
                // affect the current task.
                guard.flusher = None;
            }
        };
        (tokio::task::spawn(flusher), handle)
    }
}

pub struct BatchLogConfig {
    pub record_count: usize,
    pub flush_timeout: Duration,
}

pub struct BatchLogWriter<AWAL: storage::AsyncWAL, T> {
    /// Both WAL and CommandPos buffer.
    // TODO: Vec is autoresizable.  Find something not resizable, perhaps.
    queue: Arc<sync::Mutex<Queue<AWAL, T>>>,
    config: BatchLogConfig,
}

impl<
        AWAL: storage::AsyncWAL<Error = SimpleFileWALError> + Sync + Send + 'static,
        T: Sync + Send + 'static,
    > BatchLogWriter<AWAL, T>
{
    pub fn new(wal: AWAL, config: BatchLogConfig) -> Self {
        // TODO: check record_count and fail if it is zero; or use
        // max(1, record_count).
        Self {
            queue: Arc::new(sync::Mutex::new(Queue::new(wal, config.record_count))),
            config,
        }
    }
}

#[async_trait]
impl<AWAL, V> storage::LogWriter<V> for BatchLogWriter<AWAL, V>
where
    AWAL: storage::AsyncWAL<Error = SimpleFileWALError> + Sync + Send + 'static,
    V: Sync + Send + 'static,
{
    type DataWrite = AWAL::DataWrite;
    type Error = BatchLogError;

    async fn command<I, F>(
        &self, // TODO Pin?  Arc?
        serializer: I,
    ) -> Result<V, Self::Error>
    where
        I: FnOnce(storage::AsyncWriteWrapper<Self::DataWrite>) -> F + Send + Sync,
        F: Future<Output = (io::Result<V>, storage::AsyncWriteWrapper<Self::DataWrite>)>
            + Send
            + Sync
            + 'async_trait,
    {
        let mut guard = self.queue.clone().lock_owned().await;
        if guard.index_buf.len() == guard.index_buf.capacity() {
            // The buffer is full and have to be flushed.
            // First, remove timeout.
            if let Some((_, handle)) = guard.flusher.take() {
                handle.abort();
            }
            // Now flush and empty the buffer.  Should it be done
            // in a separate thread?  Should timeout aligned to
            // begin of flush or end of flush?
            Queue::flush(&mut guard).await?;

            // Reinstall timeout.
            guard.flusher = Some(Queue::get_flusher(
                self.queue.clone(),
                self.config.flush_timeout,
            ));
        }
        // TODO: check vector is full after adding the element,
        // and flush instantly.  It makes benching easier.  Now
        // optimal number of bench request threads is capacity + 1, with
        // such modification it is exactly capacity.

        // TODO reconsider error handling.  Send err to batched
        // requests in case of io error? The requests get stuck.
        // On another hand, the only possible thing is to restart?
        // TODO: AsyncWAL has to rollback on failure.

        // Invariant: now buffer has some space.
        let (pos, val) = guard
            .wal
            .command(serializer)
            .await
            .map_err(|e| BatchLogError::Nested(Arc::new(e)))?;
        let (sender, receiver) = sync::oneshot::channel();
        guard.index_buf.push(pos);
        guard.data_buf.push((val, sender));

        if guard.flusher.is_none() {
            // We have written some data, timeout has to be
            // installed.
            guard.flusher = Some(Queue::get_flusher(
                self.queue.clone(),
                self.config.flush_timeout,
            ));
        }
        drop(guard); // Explicitly

        match receiver.await {
            Ok(r) => r,
            // Please note that this is not a fatal error, as it relates
            // single request only.
            // tokio::sync::oneshot::error::RecvError
            Err(e) => Err(BatchLogError::CantHappen {
                file: file!(),
                line: line!(),
                msg: Cow::from("found closed oneshot::Sender in the BatchLogWriter"),
                nested: Arc::new(Box::new(e)),
            }),
        }
    }
}
