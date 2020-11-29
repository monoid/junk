use std::error::Error;
use std::future::Future;
use std::io::SeekFrom;
use std::ops::DerefMut;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;
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

// TODO: rename, it is not only about buffered files.
#[async_trait]
pub trait AsyncBufFile: AsyncWrite {
    async fn sync_data(&mut self) -> io::Result<()>;
    async fn seek(&mut self, from: SeekFrom) -> io::Result<u64>;
    async fn tell(&mut self) -> io::Result<u64>;
}

/// Position-tracking file.  Implements AsyncBufFile::tell without any
/// syscall.
pub struct TrackingBufFile {
    nested: io::BufWriter<fs::File>,
    pos: u64,
}

impl TrackingBufFile {
    pub(crate) async fn new(mut file: fs::File) -> io::Result<Self> {
        let pos = file.seek(SeekFrom::Current(0)).await?;
        Ok(Self {
            nested: io::BufWriter::new(file),
            pos,
        })
    }
}

impl AsyncWrite for TrackingBufFile {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        let poll = Pin::new(&mut self.nested).poll_write(cx, buf);
        if let std::task::Poll::Ready(Ok(len)) = &poll {
            self.pos += *len as u64;
        }
        poll
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        Pin::new(&mut self.nested).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        Pin::new(&mut self.nested).poll_shutdown(cx)
    }
}

#[async_trait]
impl AsyncBufFile for TrackingBufFile {
    async fn sync_data(&mut self) -> io::Result<()> {
        self.nested.flush().await?;
        self.nested.get_mut().sync_data().await
    }

    async fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.nested.flush().await?;
        let newpos = self.nested.get_mut().seek(from).await?;
        self.pos = newpos;
        Ok(newpos)
    }

    async fn tell(&mut self) -> io::Result<u64> {
        Ok(self.pos)
    }
}

#[async_trait]
impl AsyncBufFile for fs::File {
    async fn sync_data(&mut self) -> io::Result<()> {
        fs::File::sync_data(self).await
    }

    async fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        fs::File::seek(self, from).await
    }

    async fn tell(&mut self) -> io::Result<u64> {
        fs::File::seek(self, SeekFrom::Current(0)).await
    }
}

/// Safety wrapper to hide underlying file (e.g. to hide file methods
/// like seek, thus stream is append-only for the user) or other type.
/// Implements AsyncWrite.
pub struct AsyncWriteWrapper<W: AsyncWrite + Unpin, H: DerefMut<Target = W> + Send + Sync>(
    pub(crate) H,
);

impl<W: AsyncWrite + Unpin, H: DerefMut<Target = W> + Send + Sync + Unpin> AsyncWrite
    for AsyncWriteWrapper<W, H>
{
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
    type Write: AsyncWrite + Unpin;
    type WriteHolder: DerefMut<Target = Self::Write> + Send + Sync + Unpin;
    type CommandPos: Clone + Send + Sync + 'static;
    type Error: Error + Send + Sync + 'static;

    /// Executes command that writes data to AsyncWriteWrapper.
    async fn command<I, T, F>(
        &mut self, // TODO Pin?
        serializer: I,
    ) -> Result<(Self::CommandPos, T), Self::Error>
    where
        I: FnOnce(AsyncWriteWrapper<Self::Write, Self::WriteHolder>) -> F + Send + Sync,
        F: Future<
                Output = (
                    io::Result<T>,
                    AsyncWriteWrapper<Self::Write, Self::WriteHolder>,
                ),
            > + Send
            + Sync,
        T: Send + 'static;
    /// Appends data(s) to index file.  For durabale file store, it
    /// has to flush data file, write index and then flush index.  But
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
    type Write: AsyncWrite + Unpin;
    type WriteHolder: DerefMut<Target = Self::Write> + Send + Sync + Unpin;
    type Error: Error + Send + Sync + 'static;
    /// Write data to the log, committing it after that.  serializer
    /// is an async function that gets an AsyncWrite to write data,
    /// e.g. with tokio-serde.
    async fn command<I, F>(
        &self, // TODO Pin?
        serializer: I,
    ) -> Result<V, Self::Error>
    where
        I: FnOnce(AsyncWriteWrapper<Self::Write, Self::WriteHolder>) -> F + Send + Sync,
        F: Future<
                Output = (
                    io::Result<V>,
                    AsyncWriteWrapper<Self::Write, Self::WriteHolder>,
                ),
            > + Send
            + Sync
            + 'async_trait;
}

#[async_trait]
pub trait FileSyncer: Send + Sync {
    async fn sync<W: AsyncBufFile + Send + Sync>(&self, file: &mut W) -> io::Result<()>;
}

#[derive(Default)]
pub struct NoopFileSyncer {}

#[async_trait]
impl FileSyncer for NoopFileSyncer {
    async fn sync<W: AsyncBufFile + Send + Sync>(&self, _file: &mut W) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct SyncDataFileSyncer {}

#[async_trait]
impl FileSyncer for SyncDataFileSyncer {
    async fn sync<W: AsyncBufFile + Send + Sync>(&self, file: &mut W) -> io::Result<()> {
        file.sync_data().await
    }
}

async fn trim_log<S: FileSyncer>(file: &mut fs::File, len: u64, sync: &S) -> io::Result<()> {
    file.seek(SeekFrom::Start(len)).await?;
    file.set_len(len).await?;
    sync.sync(file).await
}

async fn trim_buf_log<S: FileSyncer>(
    file: &mut TrackingBufFile,
    len: u64,
    sync: &S,
) -> io::Result<()> {
    // It is more about cleaning buffer to be not appended to file later.
    file.flush().await?;
    file.seek(SeekFrom::Start(len)).await?;
    file.nested.get_mut().set_len(len).await?;
    sync.sync(file).await
}

#[derive(Error, Debug)]
pub enum SimpleFileWALError {
    #[error("failed to write log data")]
    DataFailure(io::Error),
    #[error("failed to write index data")]
    IndexFailure(io::Error),
    #[error("failed to write index data: {0}, and failed to rollback: {1}")]
    DataFatalFailure(io::Error, io::Error),
}

use SimpleFileWALError::*;

/// Simple log with data and index files; synced with FileSyncer.
pub struct SimpleFileWAL<S> {
    data_file: Arc<sync::Mutex<TrackingBufFile>>,
    index_file: TrackingBufFile,
    sync: S,
}

impl<S: FileSyncer> SimpleFileWAL<S> {
    /// Parse index_file, finding last commited data position.
    /// Truncate offset file and data file if incomplete or uncommited
    /// data is found.
    // TODO: feed parsed data into some callback to restore data.
    pub async fn new(
        mut data_file: fs::File,
        mut index_file: fs::File,
        sync: S,
    ) -> Result<Self, SimpleFileWALError> {
        let mut data_committed_pos: u64 = 0;
        // We have learned the size, but move position to the end.
        // We get proper position from the index_file, and then
        // seek to it later (actually, this is a point of this method).
        let data_len = data_file
            .seek(SeekFrom::End(0))
            .await
            .map_err(DataFailure)?;

        index_file
            .seek(SeekFrom::Start(0))
            .await
            .map_err(IndexFailure)?;

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
                // TODO: read state
                data_committed_pos += data_size;
                offset_offset += std::mem::size_of::<u64>() as u64;
            } else {
                break;
            }
        }

        // TODO: log offsets truncation.
        trim_log(&mut index_file, offset_offset, &sync)
            .await
            .map_err(IndexFailure)?;
        // TODO: log data truncation.
        trim_log(&mut data_file, data_committed_pos, &sync)
            .await
            .map_err(DataFailure)?;

        // 8kb is enough for everyone.
        Ok(Self {
            index_file: TrackingBufFile::new(index_file)
                .await
                .map_err(IndexFailure)?,
            data_file: Arc::new(sync::Mutex::new(
                TrackingBufFile::new(data_file).await.map_err(DataFailure)?,
            )),
            sync,
        })
    }

    pub async fn open<P: AsRef<Path>>(
        data_path: P,
        index_path: P,
        sync: S,
    ) -> Result<Self, SimpleFileWALError> {
        let log_options = {
            let mut log_options = fs::OpenOptions::new();
            log_options.read(true).write(true).create(true);
            log_options
        };
        // TODO advisory lock?
        let data_file = log_options.open(data_path).await.map_err(DataFailure)?;
        let index_file = log_options.open(index_path).await.map_err(IndexFailure)?;
        Self::new(data_file, index_file, sync).await
    }
}

#[async_trait]
impl<S: FileSyncer> AsyncWAL for SimpleFileWAL<S> {
    type Write = TrackingBufFile;
    type WriteHolder = sync::OwnedMutexGuard<TrackingBufFile>;
    type CommandPos = u64;
    type Error = SimpleFileWALError;

    async fn command<I, T, F>(
        &mut self, // TODO Pin?
        serializer: I,
    ) -> Result<(Self::CommandPos, T), Self::Error>
    where
        I: FnOnce(AsyncWriteWrapper<Self::Write, Self::WriteHolder>) -> F + Send + Sync,
        F: Future<
                Output = (
                    io::Result<T>,
                    AsyncWriteWrapper<Self::Write, Self::WriteHolder>,
                ),
            > + Send
            + Sync,
        T: Send + 'static,
    {
        let mut data_write: Self::WriteHolder = self.data_file.clone().lock_owned().await;
        let orig_pos = data_write.tell().await.map_err(DataFailure)?;
        let (v, mut data_write) = match serializer(AsyncWriteWrapper(data_write)).await {
            (Ok(v), AsyncWriteWrapper(data_write)) => (Ok((v, data_write))),
            (Err(e), AsyncWriteWrapper(mut data_write)) => {
                let rollback_res = trim_buf_log(&mut data_write, orig_pos, &self.sync).await;
                match rollback_res {
                    Ok(()) => Err(DataFailure(e)),
                    Err(rollback_err) => Err(DataFatalFailure(e, rollback_err)),
                }
            }
        }?;
        let new_pos = data_write.tell().await.map_err(DataFailure)?;
        Ok((new_pos - orig_pos, v))
    }

    async fn indices(&mut self, pos: &[Self::CommandPos]) -> Result<(), Self::Error> {
        {
            let mut data_write = self.data_file.lock().await;
            self.sync
                .sync(data_write.deref_mut())
                .await
                .map_err(DataFailure)?;
        }
        for p in pos {
            self.index_file
                .write_all(&p.to_ne_bytes())
                .await
                .map_err(IndexFailure)?;
        }
        self.sync
            .sync(&mut self.index_file)
            .await
            .map_err(IndexFailure)?;
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
impl<AWAL: AsyncWAL + Send + Sync, V: Send + Sync + 'static> LogWriter<V>
    for InstantLogWriter<AWAL>
{
    type Write = AWAL::Write;
    type WriteHolder = AWAL::WriteHolder;
    type Error = AWAL::Error;

    async fn command<I, F>(
        &self, // TODO Pin?
        serializer: I,
    ) -> Result<V, Self::Error>
    where
        I: FnOnce(AsyncWriteWrapper<Self::Write, Self::WriteHolder>) -> F + Send + Sync,
        F: Future<
                Output = (
                    io::Result<V>,
                    AsyncWriteWrapper<Self::Write, Self::WriteHolder>,
                ),
            > + Send
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
            type Write = Vec<u8>;
            type WriteHolder = sync::OwnedMutexGuard<Vec<u8>>;
            type CommandPos = usize;
            type Error = io::Error;

            /// Executes command that writes data to AsyncWriteWrapper.
            async fn command<I, T, F>(
                &mut self, // TODO Pin?
                serializer: I,
            ) -> io::Result<(Self::CommandPos, T)>
            where
                I: FnOnce(AsyncWriteWrapper<Self::Write, Self::WriteHolder>) -> F + Send + Sync,
                F: Future<
                        Output = (
                            io::Result<T>,
                            AsyncWriteWrapper<Self::Write, Self::WriteHolder>,
                        ),
                    > + Send
                    + Sync,
                T: Send + 'static,
            {
                let data_write = self.data.clone().lock_owned().await;
                let orig_size = data_write.len();
                match serializer(AsyncWriteWrapper(data_write)).await {
                    (Ok(res), aw) => (Ok((aw.0.len() - orig_size, res))),
                    (Err(e), _) => Err(e),
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

        // async fn hello<'a>(mut w: AsyncWriteWrapper<_, _>) -> io::Result<()>
        // {
        //     w.write_all("Hello!".as_bytes()).await.map(move |x| (x, w))
        // }

        let hello =
            |mut w: AsyncWriteWrapper<_, _>| async { (w.write_all("Hello!".as_bytes()).await, w) };

        noop.command(hello).await?;
        let data = Arc::try_unwrap(noop.wal.into_inner().data)
            .unwrap()
            .into_inner();
        assert!(String::from_utf8(data) == Ok("Hello!".to_string()));
        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_basic_tracking_writer() -> io::Result<()> {
        let tmpdir = tempfile::tempdir()?;
        std::env::set_current_dir(&tmpdir)?;
        let file = fs::File::create("myfile").await?;
        let mut track_file = TrackingBufFile::new(file).await?;

        let data1 = "test\n".as_bytes();
        track_file.write_all(data1).await?;
        assert_eq!(track_file.tell().await?, data1.len() as u64);

        track_file.seek(SeekFrom::Start(2)).await?;
        assert_eq!(track_file.tell().await?, 2);

        track_file.seek(SeekFrom::End(0)).await?;
        assert_eq!(track_file.tell().await?, data1.len() as u64);

        let mut data2 = Vec::new();
        data2.resize(16 * 2048, 0xFFu8);
        track_file.write_all(&data2[..]).await?;
        assert_eq!(track_file.tell().await?, (data1.len() + data2.len()) as u64);

        track_file.flush();
        drop(track_file);

        let mut read = fs::File::open("myfile").await?;
        let mut first_pack = Vec::new();
        first_pack.resize(data1.len(), 0);
        read.read_exact(&mut first_pack[..]).await?;
        assert_eq!(data1, &first_pack[..]);

        let mut second_pack = Vec::new();
        read.read_to_end(&mut second_pack).await?;
        assert_eq!(data2, &second_pack[..]);
        Ok(())
    }
}
