use std::io::SeekFrom;
use std::path::Path;
use std::pin::Pin;

use async_trait::async_trait;
use tokio::fs;
use tokio::io::{self, AsyncWrite, AsyncReadExt, AsyncWriteExt};

#[async_trait]
pub trait Commiter {
    // TODO get_writer.  Should Commiter own the buffer and
    // write/sync by itself?
    async fn commit(&mut self, file: &mut fs::File) -> io::Result<()>;
}

pub struct NoopCommiter {
}

/// Noop commiter that does not sync.  Useful for unit tests where
/// crashes are not tested (i.e. all unit tests).
#[async_trait]
impl Commiter for NoopCommiter {
    async fn commit(&mut self, _file: &mut fs::File) -> io::Result<()> {
        Ok(())
    }
}

/// Commiter with `File::sync_data(...)`.  We do not rely on timestamps,
/// thus `sync_data` aka `fsyncdata(2)` is enough for us.
pub struct SyncDataCommiter {
}

#[async_trait]
impl Commiter for SyncDataCommiter {
    async fn commit(&mut self, file: &mut fs::File) -> io::Result<()> {
        file.flush().await?;
        file.sync_data().await
    }
}


pub struct DoubleWAL<T> {
    data_file: fs::File,
    offsets_file: fs::File,
    commiter: T,
}


pub struct DoubleWALWriter<'a, T> {
    data_rollback_pos: u64,
    offset_rollback_pos: u64,
    parent: Option<&'a mut DoubleWAL<T>>,
}

impl<T> DoubleWAL<T> {
    /// Parse offsets_file, finding last commited data position.
    /// Truncate offset file and data file if incomplete or uncommited
    /// data is found.
    pub async fn new(mut data_file: fs::File, mut offsets_file: fs::File, commiter: T)
               -> io::Result<Self> {
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
            commiter,
        })
    }

    pub async fn open<P: AsRef<Path>>(data_path: P, offsets_path: P, commiter: T)
                                -> io::Result<Self> {
        let log_options = {
            let mut log_options = fs::OpenOptions::new();
            log_options.read(true).write(true).create(true);
            log_options
        };
        // TODO advisory lock?
        let data_file = log_options.open(data_path).await?;
        let offsets_file = log_options.open(offsets_path).await?;
        Self::new(data_file, offsets_file, commiter).await
    }

    pub async fn get_writer<'a>(&'a mut self) -> io::Result<DoubleWALWriter<'a, T>> {
        let data_rollback_pos = self.data_file.seek(SeekFrom::Current(0)).await?;
        let offset_rollback_pos = self.offsets_file.seek(SeekFrom::Current(0)).await?;

        Ok(DoubleWALWriter {
            data_rollback_pos,
            offset_rollback_pos,
            parent: Some(self),
        })
    }
}

impl<T> DoubleWALWriter<'_, T> where T: Commiter {
    pub async fn abort(&mut self) -> io::Result<()> {
        let parent = self.parent.take().expect("abort is called on destructed DoubleWALWriter");
        parent.offsets_file.set_len(self.offset_rollback_pos).await?;
        parent.data_file.set_len(self.data_rollback_pos).await
    }

    pub async fn commit(&mut self) -> io::Result<()> {
        let parent = self.parent.take().expect("commit is called on destructed DoubleWALWriter");
        parent.commiter.commit(&mut parent.data_file).await?;
        let data_new_pos = parent.data_file.seek(SeekFrom::Current(0)).await?;
        parent.offsets_file.write_u64(data_new_pos - self.data_rollback_pos).await?;
        parent.commiter.commit(&mut parent.offsets_file).await
    }
}

impl<T> Drop for DoubleWALWriter<'_, T> {
    fn drop(&mut self) {
        match self.parent {
            None => {},
            Some(_) => {
                panic!("Call either abort or commit on the DoubleWALWriter");
            }
        }
    }
}

impl<'a, T> AsyncWrite for DoubleWALWriter<'a, T> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.parent.as_mut().unwrap().data_file).poll_write(cx, buf)
    }

    fn poll_flush(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), io::Error>> {
        Pin::new(&mut self.parent.as_mut().unwrap().data_file).poll_flush(cx)
    }

    fn poll_shutdown(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), io::Error>> {
        Pin::new(&mut self.parent.as_mut().unwrap().data_file).poll_shutdown(cx)
    }
}
