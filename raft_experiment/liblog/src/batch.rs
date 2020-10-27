use std::future::Future;
use std::io;
/// Special LogWriter that writes commands in batches.
/// Batching is both record count and timeout-based.
use std::sync::Arc;
use std::time::Duration;

use crate::storage;
use async_trait::async_trait;
use futures_util::future::{abortable, AbortHandle, Aborted};
use tokio::{sync, time};

struct Queue<AWAL: storage::AsyncWAL, T> {
    wal: AWAL,
    index_buf: Vec<AWAL::CommandPos>,
    data_buf: Vec<(T, sync::oneshot::Sender<io::Result<T>>)>,
    flusher: Option<(
        Box<
            dyn Future<Output = Result<Result<(), Aborted>, tokio::task::JoinError>>
                + Send
                + 'static,
        >,
        AbortHandle,
    )>,
}

impl<AWAL: storage::AsyncWAL + Send + 'static, T: Sync + Send + 'static> Queue<AWAL, T> {
    fn new(wal: AWAL, capacity: usize) -> Self {
        Self {
            wal,
            index_buf: Vec::with_capacity(capacity),
            data_buf: Vec::with_capacity(capacity),
            flusher: None,
        }
    }

    async fn flush(guard: &mut sync::OwnedMutexGuard<Queue<AWAL, T>>) -> io::Result<()> {
        let queue: &mut Queue<AWAL, T> = &mut *guard;
        let index_buf = &mut queue.index_buf;
        let wal = &mut queue.wal;
        let indices_res = wal.indices(index_buf).await;
        index_buf.clear();

        match &indices_res {
            Ok(()) => {
                // TODO: Possible improvement: replace Vec with new one, and
                // send data in another thread.
                for (val, tx) in guard.data_buf.drain(..) {
                    // There is no point of handling the .send result.  The
                    // receiver has gone?  I couldn't care less.
                    let _ = tx.send(Ok(val));
                }
            }
            Err(e) => {
                for (_, tx) in guard.data_buf.drain(..) {
                    // There is no point of handling the .send result.  The
                    // receiver has gone?  I couldn't care less.
                    // KLUDGE: reconsider error handling.
                    // TODO error context?
                    let _ = tx.send(Err(io::Error::new(e.kind(), "WAL indices flush failed.")));
                }
            }
        }
        indices_res
    }

    fn get_flusher(
        queue: Arc<sync::Mutex<Self>>,
        flush_timeout: Duration,
    ) -> (
        Box<dyn Future<Output = Result<Result<(), Aborted>, tokio::task::JoinError>> + Send>,
        AbortHandle,
    ) {
        let flusher = async move {
            time::delay_for(flush_timeout).await;
            let mut guard = queue.lock_owned().await;
            // TODO what to do with the error?  Set it somewhere.
            let _ = Queue::flush(&mut guard).await;
            // TODO is it even safe?
            guard.flusher = None;
        };
        // We use spawn_local, as there is no point for
        // running it in a separate thread, as all work
        // is done under lock gurad.
        let (flusher, handle) = abortable(flusher);
        (Box::new(tokio::task::spawn_local(flusher)), handle)
    }
}

pub struct BatchLogWriter<AWAL: storage::AsyncWAL, T> {
    // TODO: how to change timeout? Should it be behind
    // mutex as well?
    timeout: Option<Box<dyn Future<Output = ()> + Send + Sync>>,
    /// Both WAL and CommandPos buffer.
    // TODO: Vec is autoresizable.  Find something not resizable, perhaps.
    queue: Arc<sync::Mutex<Queue<AWAL, T>>>,
    config: BatchLogConfig,
}

pub struct BatchLogConfig {
    pub record_count: usize,
    pub flush_timeout: Duration,
}

impl<AWAL: storage::AsyncWAL + Sync + Send + 'static, T: Sync + Send + 'static>
    BatchLogWriter<AWAL, T>
{
    pub fn new(wal: AWAL, config: BatchLogConfig) -> Self {
        // TODO: check record_count and fail if it is zero; or use
        // max(1, record_count).
        Self {
            timeout: None,
            queue: Arc::new(sync::Mutex::new(Queue::new(wal, config.record_count))),
            config,
        }
    }
}

#[async_trait]
impl<AWAL, V> storage::LogWriter<V> for BatchLogWriter<AWAL, V>
where
    AWAL: storage::AsyncWAL + Sync + Send + 'static,
    V: Sync + Send + 'static,
{
    type DataWrite = AWAL::DataWrite;

    async fn command<I, F>(
        &self, // TODO Pin?  Arc?
        serializer: I,
    ) -> std::io::Result<V>
    where
        I: FnOnce(storage::AsyncWriteWrapper<Self::DataWrite>) -> F + Send + Sync,
        F: Future<Output = std::io::Result<(V, storage::AsyncWriteWrapper<Self::DataWrite>)>>
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
        // requests in case of io error?

        // Invariant: now buffer has some space.
        let (pos, val) = guard.wal.command(serializer).await?;
        let (sender, receiver) = sync::oneshot::channel();
        guard.index_buf.push(pos);
        guard.data_buf.push((val, sender));

        if self.timeout.is_none() {
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
            // KLUDGE

            // TODO: Error type is an associated type of LogWriter
            // (and AsyncAWAIT?); pass some context for Error handling.
            // Define Command trait that both provides context and
            // defines data to store.  Box<dyn Command>, like this.
            // Or, as all implementation of LogWriter are internal,
            // we may define a common error type for all cases.
            // TODO: unwrap?  Is it even possible that sender is dropped?
            Err(_) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "oneshot failed")),
        }
    }
}
