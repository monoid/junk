use std::{
    fs::{File, Metadata},
    io::{self, Read as _},
    os::unix::fs::MetadataExt,
    path::Path,
    time::{Duration, SystemTime},
};

const REFRESH_TIME: Duration = Duration::from_secs(1);

#[derive(Debug, PartialEq, Eq)]
struct FileState {
    size: u64,
    modified: SystemTime,
}

#[derive(Debug)]
struct FileWatcher<P> {
    path: P,
    prev_state: Option<FileState>,
}

impl<P> FileWatcher<P> {
    fn new(path: P) -> Self {
        Self {
            path,
            prev_state: None,
        }
    }
}

impl<P: AsRef<Path>> FileWatcher<P> {
    fn try_refresh(&mut self) -> io::Result<Option<String>> {
        let mut file = match File::open(&self.path) {
            Ok(file) => file,
            // If file is not found, it is not an error: file may be
            // non-existent when it is swapped and will be available on the next iteration.
            // But it may cause problem on the first iteration.
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };

        let new_state = file.metadata()?.try_into()?;

        if Some(&new_state) == self.prev_state.as_ref() {
            Ok(None)
        } else {
            self.prev_state = Some(new_state);

            let mut buf = String::new();
            file.read_to_string(&mut buf)?;
            Ok(Some(buf))
        }
    }
}

impl TryFrom<Metadata> for FileState {
    type Error = io::Error;
    fn try_from(meta: Metadata) -> io::Result<Self> {
        Ok(Self {
            size: meta.size(),
            modified: meta.modified()?,
        })
    }
}

fn main() -> io::Result<()> {
    let path = Path::new("Cargo.toml");

    let mut watcher = FileWatcher::new(path);

    loop {
        if let Some(content) = watcher.try_refresh()? {
            eprintln!("{content}");
            eprintln!("---------------")
        }
        std::thread::sleep(REFRESH_TIME);
    }
}
