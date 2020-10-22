use std::future::Future;
use std::io::SeekFrom;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{self, AsyncReadExt, AsyncWrite};
use tokio::{fs, sync};

/**
 * Raft request (i.e. command) Log.
 *
 * LogWriter implements writing strategy.  Writing after each record,
 * sync after several records/on timeout.
 *
 * AsyncWAL incapsulates writing and syncing method, be it memory,
 * files without sync, files+f(data)sync, DIRECT_IO etc.
 *
 */

/// Safety wrapper to hide underlying file (e.g. to hide file methods
/// like seek, thus stream is append-only for the user).  You can use
/// it as any AsyncWrite.
pub struct AsyncWriteWrapper<T: AsyncWrite + Send + Sync>(sync::OwnedMutexGuard<T>);

impl<T: AsyncWrite + Send + Sync> AsyncWriteWrapper<T> {
    pub async fn new(w: &Arc<sync::Mutex<T>>) -> Self {
        Self(w.clone().lock_owned().await)
    }
}

impl<T: AsyncWrite + Send + Sync + Unpin> AsyncWrite for AsyncWriteWrapper<T> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        Pin::new(&mut *self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        Pin::new(&mut *self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        Pin::new(&mut *self.0).poll_shutdown(cx)
    }
}

/// Async log writer.  The write_command async method gets an FnOnce that can
/// write data, and all written data will be recorded as log command.  Its size and
/// position will be durably recorded as well.
///
/// Please note that LogWriter can group writes into batches in data
/// size and time manner, and command will return on when
/// requested data is durably written or error is detected.  See
/// documentation for particular implementation.  Of course, order of
/// commands is respected.
#[async_trait]
pub trait LogWriter {
    type DataWrite: AsyncWrite + Send + Sync;
    /// Write data to the log, committing it after that.  serializer
    /// is an async function that gets an AsyncWrite to write data,
    /// e.g. with tokio-serde.
    async fn command<I, T, F>(
        &mut self, // TODO Pin?
        serializer: I,
    ) -> F::Output
    where
        I: FnOnce(AsyncWriteWrapper<Self::DataWrite>) -> F + Send + Sync,
        F: Future<Output = io::Result<T>> + Send + Sync + 'async_trait,
        T: Send + 'static;
}

/// WAL aka log.  Implementations differ by storage, syncing method etc.
// TODO rename to storage?  But it is not only storage, but sync method.
#[async_trait]
pub trait AsyncWAL {
    /// Type for writing data.
    // The alternative is dyn Write.  Associated type doesn't allow
    // using same F for different AsyncWAL.  OTOH using different
    // AsyncWAL in same project out of testing scope is not expected.
    type DataWrite: AsyncWrite + Send + Sync;
    type CommandPos: Clone + Send + Sync + 'static;

    /// Executes command that writes data to AsyncWriteWrapper.
    async fn command<I, T, F>(
        &mut self, // TODO Pin?
        serializer: I,
    ) -> io::Result<(Self::CommandPos, T)>
    where
        I: FnOnce(AsyncWriteWrapper<Self::DataWrite>) -> F + Send + Sync,
        F: Future<Output = io::Result<T>> + Send + Sync,
        T: Send + 'static;
    /// Appends data to index file.  For durabale file store, it has
    /// to flush data file, write index and then flush index.  But
    /// lightweight implementations for non-durable tests may skip
    /// flushing.  Of course, in-memory implementation do not need
    /// flush at all.
    async fn indices(&mut self, pos: &[Self::CommandPos]) -> io::Result<()>;
}

/// Simple writer that commits each command instantly.
/// Useful for non-durability tests.
pub struct NoopLogWriter<Wal> {
    // TODO: abstract WAL: memory, normal files, direct write
    wal: Wal,
}

/// Simple log with data and index files.
struct SimpleWAL {
    data_file: fs::File,
    offsets_file: fs::File,
}

impl SimpleWAL {
    /// Parse offsets_file, finding last commited data position.
    /// Truncate offset file and data file if incomplete or uncommited
    /// data is found.
    pub async fn new(mut data_file: fs::File, mut offsets_file: fs::File) -> io::Result<Self> {
        let mut data_committed_pos: u64 = 0;
        // We have learned the size, but move position to the end.
        // We get proper position from the offsets_file, and then
        // seek to it later (actually, this is a point of this method).
        let data_len = data_file.seek(SeekFrom::End(0)).await?;

        offsets_file.seek(SeekFrom::Start(0)).await?;

        // TODO refactor to a separate function to test it.  It needs
        // only Read to be tested.
        let mut data_size: u64;
        // Valid offset known so far.
        let mut offset_offset: u64 = 0;

        // TODO add buffering only for reading.
        loop {
            data_size = match offsets_file.read_u64().await {
                Ok(n) => n,
                Err(_) => {
                    break;
                }
            };

            if data_committed_pos + data_size <= data_len {
                data_committed_pos += data_size;
                offset_offset += std::mem::size_of::<u64>() as u64;
            } else {
                break;
            }
        }
        offsets_file.seek(SeekFrom::Start(offset_offset)).await?;
        // TODO: log offsets truncation.
        offsets_file.set_len(offset_offset).await?;
        offsets_file.sync_data().await?;

        data_file.seek(SeekFrom::Start(data_committed_pos)).await?;
        // TODO: log data truncation.
        data_file.set_len(data_committed_pos).await?;
        data_file.sync_data().await?;

        Ok(Self {
            data_file,
            offsets_file,
        })
    }

    pub async fn open<P: AsRef<Path>>(data_path: P, offsets_path: P) -> io::Result<Self> {
        let log_options = {
            let mut log_options = fs::OpenOptions::new();
            log_options.read(true).write(true).create(true);
            log_options
        };
        // TODO advisory lock?
        let data_file = log_options.open(data_path).await?;
        let offsets_file = log_options.open(offsets_path).await?;
        Self::new(data_file, offsets_file).await
    }
}

#[async_trait]
impl<TWal: AsyncWAL + Send + Sync> LogWriter for NoopLogWriter<TWal> {
    type DataWrite = TWal::DataWrite;
    async fn command<I, T, F>(
        &mut self, // TODO Pin?
        serializer: I,
    ) -> F::Output
    where
        I: FnOnce(AsyncWriteWrapper<Self::DataWrite>) -> F + Send + Sync,
        F: Future<Output = io::Result<T>> + Send + Sync + 'async_trait,
        T: Send + 'static,
    {
        let (pos, val) = self.wal.command(serializer).await?;
        self.wal.indices(std::slice::from_ref(&pos)).await?; // TODO how to tell if error is from data or index?
        Ok(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test(threaded_scheduler)]
    async fn test_async_write_simple() -> io::Result<()> {
        // A simple test that proofs that whole idea is implementable.
        struct MemWal {
            data: Arc<sync::Mutex<Vec<u8>>>,
            indices: Vec<usize>,
        }

        #[async_trait]
        impl AsyncWAL for MemWal {
            type DataWrite = Vec<u8>;
            type CommandPos = usize;

            /// Executes command that writes data to AsyncWriteWrapper.
            async fn command<I, T, F>(
                &mut self, // TODO Pin?
                serializer: I,
            ) -> io::Result<(Self::CommandPos, T)>
            where
                I: FnOnce(AsyncWriteWrapper<Self::DataWrite>) -> F + Send + Sync,
                F: Future<Output = io::Result<T>> + Send + Sync,
                T: Send + 'static,
            {
                let orig_size = self.data.lock().await.len();
                match serializer(AsyncWriteWrapper::new(&self.data).await).await {
                    Ok(res) => Ok((self.data.lock().await.len() - orig_size, res)),
                    Err(e) => Err(e),
                }
            }

            async fn indices(&mut self, pos: &[Self::CommandPos]) -> io::Result<()> {
                self.indices.extend_from_slice(pos);
                Ok(())
            }
        }

        let mut noop = NoopLogWriter {
            wal: MemWal {
                data: Default::default(),
                indices: Default::default(),
            },
        };

        // async fn hello<'a>(mut w: AsyncWriteWrapper<Vec<u8>>) -> io::Result<()> {
        //     w.write_all("Hello!".as_bytes()).await
        // }

        let hello = move |mut w: AsyncWriteWrapper<Vec<u8>>| async move {
            w.write_all("Hello!".as_bytes()).await
        };

        noop.command(hello).await?;
        let data = Arc::try_unwrap(noop.wal.data).unwrap().into_inner();
        assert!(String::from_utf8(data) == Ok("Hello!".to_string()));
        Ok(())
    }
}
