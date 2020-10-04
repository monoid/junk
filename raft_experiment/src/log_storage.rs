use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::Path;
use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};

type LogEndian = NetworkEndian;

pub trait Commiter {
    // TODO async
    // TODO get_writer.  Should Commiter own the buffer and
    // write/sync by itself?
    fn commit(&mut self, file: &mut fs::File) -> io::Result<()>;
}

pub struct NoopCommiter {
}

/// Noop commiter that does nothing.  Useful for unit tests where
/// crashes are not tested (i.e. all unit tests).
impl Commiter for NoopCommiter {
    fn commit(&mut self, _file: &mut fs::File) -> io::Result<()> {
        Ok(())
    }
}

/// Commiter with `File::sync_data(...)`.  We do not rely on timestamps,
/// thus `sync_data` aka `fsyncdata(2)` is enough for us.
pub struct SyncDataCommiter {
}

impl Commiter for SyncDataCommiter {
    fn commit(&mut self, file: &mut fs::File) -> io::Result<()> {
        file.flush()?;
        file.sync_data()
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
    pub fn new(mut data_file: fs::File, mut offsets_file: fs::File, commiter: T)
               -> io::Result<Self> {
        let mut data_committed_pos: u64 = 0;
        // We have learned the size, but move position to the end.
        // We get proper position from the offsets_file, and then
        // seek to it later (actually, this is a point of this method).
        let data_len = data_file.seek(io::SeekFrom::End(0))?;

        offsets_file.seek(io::SeekFrom::Start(0))?;

        // TODO refactor to a separate function to test it.  It needs
        // only Read to be tested.
        let mut data_size: u64;
        // Valid offset known so far.
        let mut offset_offset: u64 = 0;

        // TODO add buffering only for reading.
        loop {
            data_size = match offsets_file.read_u64::<LogEndian>() {
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
        offsets_file.seek(io::SeekFrom::Start(offset_offset))?;
        // TODO: log offsets truncation.
        offsets_file.set_len(offset_offset)?;
        offsets_file.sync_data()?;

        data_file.seek(io::SeekFrom::Start(data_committed_pos))?;
        // TODO: log data truncation.
        data_file.set_len(data_committed_pos)?;
        data_file.sync_data()?;

        Ok(Self {
            data_file,
            offsets_file,
            commiter,
        })
    }

    pub fn open<P: AsRef<Path>>(data_path: P, offsets_path: P, commiter: T)
                                -> io::Result<Self> {
        let log_options = {
            let mut log_options = fs::OpenOptions::new();
            log_options.read(true).write(true).create(true);
            log_options
        };
        // TODO advisory lock?
        let data_file = log_options.open(data_path)?;
        let offsets_file = log_options.open(offsets_path)?;
        Self::new(data_file, offsets_file, commiter)
    }

    pub fn get_writer(&mut self) -> io::Result<DoubleWALWriter<T>> {
        let data_rollback_pos = self.data_file.seek(io::SeekFrom::Current(0))?;
        let offset_rollback_pos = self.offsets_file.seek(io::SeekFrom::Current(0))?;

        Ok(DoubleWALWriter {
            data_rollback_pos,
            offset_rollback_pos,
            parent: Some(self),
        })
    }
}

impl<T> DoubleWALWriter<'_, T> where T: Commiter {
    pub fn abort(&mut self) -> io::Result<()> {
        let parent = self.parent.take().expect("abort is called on destructed DoubleWALWriter");
        parent.offsets_file.set_len(self.offset_rollback_pos)?;
        parent.data_file.set_len(self.data_rollback_pos)
    }

    pub fn commit(&mut self) -> io::Result<()> {
        let parent = self.parent.take().expect("commit is called on destructed DoubleWALWriter");
        parent.commiter.commit(&mut parent.data_file)?;
        let data_new_pos = parent.data_file.seek(io::SeekFrom::Current(0))?;
        parent.offsets_file.write(&(data_new_pos - self.data_rollback_pos).to_ne_bytes())?;
        parent.commiter.commit(&mut parent.offsets_file)
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

impl<'a, T> io::Write for DoubleWALWriter<'a, T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.parent.as_mut().unwrap().data_file.write(buf)
    }
    
    fn flush(&mut self) -> io::Result<()> {
        self.parent.as_mut().unwrap().data_file.flush()
    }
}
