use std::future::Future;
use std::io::SeekFrom;
use std::path::Path;
use std::pin::Pin;

use async_trait::async_trait;
use tokio::fs;
use tokio::io::{self, AsyncReadExt, AsyncWrite};

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

/// Safety wrapper to hide implementation (e.g. to hide file methods).
/// You can use it as any AsyncWrite.
pub struct AsyncWriteWrapper<'a, T: AsyncWrite + Send + Sync>(&'a mut T);

impl<'a, T: AsyncWrite + Send + Sync + Unpin> AsyncWrite for AsyncWriteWrapper<'a, T> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), io::Error>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
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
trait LogWriter {
    type W: AsyncWrite + Send + Sync;
    /// Write data to the log, committing it after that.  serializer
    /// is an async function that gets an AsyncWrite to write data,
    /// e.g. with tokio-serde.
    async fn command<I, T, F>(
        &'async_trait mut self, // TODO Pin?
        serializer: I,
    ) -> F::Output
    where
        I: Fn(AsyncWriteWrapper<'async_trait, Self::W>) -> F + Send + Sync,
        F: Future<Output = io::Result<T>> + Send + Sync;
}

/// WAL aka log.
// TODO rename to storage?
pub trait AsyncWAL {
    type W: AsyncWrite + Send + Sync;
    type CommandPos: Send + Sync;
    // TODO: command that returns (CommandPos, F::Output), like LogWriter.
    // TODO: commit_data -- it seems the only reasonable way to make nocommit log is create nocommit
    //       AsyncWAL.  Thus it is indices who will commit first commands, then indices.
    //       And commit_methods should not exist.
    // TODO: indices(&[CommandPos])
    // TODO: commit_indices
    fn get_data_writer(&mut self) -> &mut Self::W;
}

/// Stupid writer that doesn't bother with syncing at all.
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
    type W = TWal::W;
    async fn command<I, T, F>(
        &'async_trait mut self, // TODO Pin?
        serializer: I,
    ) -> F::Output
    where
        I: Fn(AsyncWriteWrapper<'async_trait, Self::W>) -> F + Send + Sync,
        F: Future<Output = io::Result<T>> + Send + Sync,
    {
        // TODO it should handle multiple records.
        // It seems WAL should contain queue of record indices
        // let recidx = self.wal.TODO_begin_record()?;
        let val = serializer(AsyncWriteWrapper(self.wal.get_data_writer())).await?;
        // recidx.TODO_finish()?;
        // recidx.flush_all();
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
            data: Vec<u8>,
        }

        impl AsyncWAL for MemWal {
            type W = Vec<u8>;
            type CommandPos = ();

            fn get_data_writer(&mut self) -> &mut Self::W {
                &mut self.data
            }
        }

        let mut noop = NoopLogWriter {
            wal: MemWal {
                data: Default::default(),
            },
        };

        noop.command(|mut w| async move { w.write_all("Hello!".as_bytes()).await }).await?;
        assert!(String::from_utf8(noop.wal.data) == Ok("Hello!".to_string()));
        Ok(())
    }
}
