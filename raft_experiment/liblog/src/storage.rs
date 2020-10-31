use std::convert::From;
use std::error::Error;
use std::future::Future;
use std::io::SeekFrom;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{self, AsyncReadExt, AsyncWrite, AsyncWriteExt};
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
pub struct AsyncWriteWrapper<T: AsyncWrite + Send + Sync>(pub(crate) sync::OwnedMutexGuard<T>);

impl<T: AsyncWrite + Send + Sync + Unpin> AsyncWrite for AsyncWriteWrapper<T> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        Pin::new(&mut *self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        Pin::new(&mut *self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        Pin::new(&mut *self.0).poll_shutdown(cx)
    }
}

/// WAL aka log.  Implementations differ by storage, syncing method etc.
// TODO rename to storage?  But it is not only storage, but sync method.
#[async_trait]
pub trait AsyncWAL {
    /// Type for writing data.
    // The alternative is dyn Write.  Associated type doesn't allow
    // using same F for different AsyncWAL.  OTOH using different
    // AsyncWAL in same project out of testing scope is not expected.
    type DataWrite: AsyncWrite + Send + Sync + Unpin;
    type CommandPos: Clone + Send + Sync + 'static;
    type Error: From<io::Error> + Error + Send + Sync + 'static;

    /// Executes command that writes data to AsyncWriteWrapper.
    async fn command<I, T, F>(
        &mut self, // TODO Pin?
        serializer: I,
    ) -> Result<(Self::CommandPos, T), Self::Error>
    where
        I: FnOnce(AsyncWriteWrapper<Self::DataWrite>) -> F + Send + Sync,
        // TODO: return (Result<T>, AWW), not Result<(T, AWW)>.
        // OTOH, it is neither convenient to handle nor required.
        F: Future<Output = io::Result<(T, AsyncWriteWrapper<Self::DataWrite>)>> + Send + Sync,
        T: Send + 'static;
    /// Appends data to index file.  For durabale file store, it has
    /// to flush data file, write index and then flush index.  But
    /// lightweight implementations for non-durable tests may skip
    /// flushing.  Of course, in-memory implementation do not need
    /// flush at all.
    async fn indices(&mut self, pos: &[Self::CommandPos]) -> Result<(), Self::Error>;
}

/// Async log writer built upon AsyncWAL.  The command async method
/// gets an FnOnce that can write data (serialized "command" like
/// "dict[K] = V"), and all written data will be recorded as log
/// command.  Its position will be durably recorded as well.
///
/// Please note that LogWriter can group writes into batches basing on
/// data size or time, and command' future will complete when
/// requested data is written (presumably durably, it depends on
/// underlying AsyncWAL) or error is detected.  See documentation for
/// particular implementation.  Of course, commands are written in the
/// command() call order.  And as command takes &mut self, only one
/// command can be written at once.
#[async_trait]
pub trait LogWriter<V: Send + Sync + 'static> {
    type DataWrite: AsyncWrite + Send + Sync + Unpin;
    type Error: Error + Send + Sync + 'static;
    /// Write data to the log, committing it after that.  serializer
    /// is an async function that gets an AsyncWrite to write data,
    /// e.g. with tokio-serde.
    async fn command<I, F>(
        &self, // TODO Pin?
        serializer: I,
    ) -> Result<V, Self::Error>
    where
        I: FnOnce(AsyncWriteWrapper<Self::DataWrite>) -> F + Send + Sync,
        F: Future<Output = io::Result<(V, AsyncWriteWrapper<Self::DataWrite>)>>
            + Send
            + Sync
            + 'async_trait;
}

#[async_trait]
pub trait FileSyncer: Send + Sync {
    async fn sync(&self, file: &mut fs::File) -> io::Result<()>;
}

#[derive(Default)]
pub struct NoopFileSyncer {}

#[async_trait]
impl FileSyncer for NoopFileSyncer {
    async fn sync(&self, _file: &mut fs::File) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct SyncDataFileSyncer {}

#[async_trait]
impl FileSyncer for SyncDataFileSyncer {
    async fn sync(&self, file: &mut fs::File) -> io::Result<()> {
        file.sync_data().await
    }
}

/// Simple log with data and index files.
pub struct SimpleFileWAL<S> {
    data_file: Arc<sync::Mutex<fs::File>>,
    index_file: fs::File,
    sync: S,
}

impl<S> SimpleFileWAL<S> {
    /// Parse index_file, finding last commited data position.
    /// Truncate offset file and data file if incomplete or uncommited
    /// data is found.
    // TODO: feed parsed data into some callback to restore data.
    pub async fn new(
        mut data_file: fs::File,
        mut index_file: fs::File,
        sync: S,
    ) -> io::Result<Self> {
        let mut data_committed_pos: u64 = 0;
        // We have learned the size, but move position to the end.
        // We get proper position from the index_file, and then
        // seek to it later (actually, this is a point of this method).
        let data_len = data_file.seek(SeekFrom::End(0)).await?;

        index_file.seek(SeekFrom::Start(0)).await?;

        let mut data_size: u64;
        // Valid offset known so far.
        let mut offset_offset: u64 = 0;

        loop {
            data_size = match index_file.read_u64().await {
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
        index_file.seek(SeekFrom::Start(offset_offset)).await?;
        // TODO: log offsets truncation.
        index_file.set_len(offset_offset).await?;
        index_file.sync_data().await?;

        data_file.seek(SeekFrom::Start(data_committed_pos)).await?;
        // TODO: log data truncation.
        data_file.set_len(data_committed_pos).await?;
        data_file.sync_data().await?;

        Ok(Self {
            data_file: Arc::new(sync::Mutex::new(data_file)),
            index_file,
            sync,
        })
    }

    pub async fn open<P: AsRef<Path>>(data_path: P, offsets_path: P, sync: S) -> io::Result<Self> {
        let log_options = {
            let mut log_options = fs::OpenOptions::new();
            log_options.read(true).write(true).create(true);
            log_options
        };
        // TODO advisory lock?
        let data_file = log_options.open(data_path).await?;
        let offsets_file = log_options.open(offsets_path).await?;
        Self::new(data_file, offsets_file, sync).await
    }
}

#[async_trait]
impl<S: FileSyncer> AsyncWAL for SimpleFileWAL<S> {
    type DataWrite = fs::File;
    type CommandPos = u64;
    type Error = io::Error;

    async fn command<I, T, F>(
        &mut self, // TODO Pin?
        serializer: I,
    ) -> Result<(Self::CommandPos, T), Self::Error>
    where
        I: FnOnce(AsyncWriteWrapper<Self::DataWrite>) -> F + Send + Sync,
        F: Future<Output = io::Result<(T, AsyncWriteWrapper<Self::DataWrite>)>> + Send + Sync,
        T: Send + 'static,
    {
        let mut data_write = self.data_file.clone().lock_owned().await;
        let orig_pos = data_write.seek(SeekFrom::Current(0)).await?;
        let (v, AsyncWriteWrapper(mut data_write)) =
            serializer(AsyncWriteWrapper(data_write)).await?;
        let new_pos = data_write.seek(SeekFrom::Current(0)).await?;
        Ok((new_pos - orig_pos, v))
    }

    async fn indices(&mut self, pos: &[Self::CommandPos]) -> Result<(), Self::Error> {
        {
            let mut data_write = self.data_file.lock().await;
            self.sync.sync(&mut data_write).await?;
        }
        for p in pos {
            self.index_file.write_all(&p.to_ne_bytes()).await?;
        }
        self.sync.sync(&mut self.index_file).await?;
        Ok(())
    }
}

/// Simple writer that commits each command instantly.
/// Useful for non-durability tests.
pub struct InstantLogWriter<AWAL> {
    wal: sync::Mutex<AWAL>,
}

impl<AWAL> InstantLogWriter<AWAL> {
    pub fn new(wal: AWAL) -> Self {
        Self {
            wal: sync::Mutex::new(wal),
        }
    }
}

#[async_trait]
impl<AWAL: AsyncWAL<Error = io::Error> + Send + Sync, V: Send + Sync + 'static> LogWriter<V>
    for InstantLogWriter<AWAL>
{
    type DataWrite = AWAL::DataWrite;
    type Error = AWAL::Error;

    async fn command<I, F>(
        &self, // TODO Pin?
        serializer: I,
    ) -> Result<V, Self::Error>
    where
        I: FnOnce(AsyncWriteWrapper<Self::DataWrite>) -> F + Send + Sync,
        F: Future<Output = io::Result<(V, AsyncWriteWrapper<Self::DataWrite>)>>
            + Send
            + Sync
            + 'async_trait,
    {
        let mut guard = self.wal.lock().await;
        let (pos, val) = guard.command(serializer).await?;
        guard.indices(std::slice::from_ref(&pos)).await?; // TODO how to tell if error is from data or index?
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
            type Error = io::Error;

            /// Executes command that writes data to AsyncWriteWrapper.
            async fn command<I, T, F>(
                &mut self, // TODO Pin?
                serializer: I,
            ) -> io::Result<(Self::CommandPos, T)>
            where
                I: FnOnce(AsyncWriteWrapper<Self::DataWrite>) -> F + Send + Sync,
                F: Future<Output = io::Result<(T, AsyncWriteWrapper<Self::DataWrite>)>>
                    + Send
                    + Sync,
                T: Send + 'static,
            {
                let data_write = self.data.clone().lock_owned().await;
                let orig_size = data_write.len();
                match serializer(AsyncWriteWrapper(data_write)).await {
                    Ok((res, AsyncWriteWrapper(data_write))) => {
                        Ok((data_write.len() - orig_size, res))
                    }
                    Err(e) => Err(e),
                }
            }

            async fn indices(&mut self, pos: &[Self::CommandPos]) -> io::Result<()> {
                self.indices.extend_from_slice(pos);
                Ok(())
            }
        }

        let noop = InstantLogWriter {
            wal: sync::Mutex::new(MemWal {
                data: Default::default(),
                indices: Default::default(),
            }),
        };

        // async fn hello<'a>(mut w: AsyncWriteWrapper<Vec<u8>>) -> io::Result<()> {
        //     w.write_all("Hello!".as_bytes()).await.map(move |x| (x, w))
        // }

        let hello = move |mut w: AsyncWriteWrapper<Vec<u8>>| async move {
            w.write_all("Hello!".as_bytes()).await.map(move |x| (x, w))
        };

        noop.command(hello).await?;
        let data = Arc::try_unwrap(noop.wal.into_inner().data)
            .unwrap()
            .into_inner();
        assert!(String::from_utf8(data) == Ok("Hello!".to_string()));
        Ok(())
    }
}
