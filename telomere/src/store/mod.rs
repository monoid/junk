use std::{
    fs::File,
    io::Write,
    ops::DerefMut,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use teloxide::types::Message;

/** Message store. */
#[async_trait::async_trait]
pub trait Store: Clone {
    type Error;

    async fn store(&self, msg: Message) -> Result<(), Self::Error>;
}

/** Store messages to a newline-separated JSON text file, message per line.
*/
#[derive(Clone)]
pub struct JsonFileStore(Arc<Mutex<File>>);

impl JsonFileStore {
    pub fn new(path: &PathBuf) -> Result<Self, std::io::Error> {
        let output = std::sync::Arc::new(std::sync::Mutex::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?,
        ));
        Ok(Self(output))
    }
}

#[async_trait::async_trait]
impl Store for JsonFileStore {
    type Error = std::io::Error;
    async fn store(&self, msg: Message) -> Result<(), Self::Error> {
        let output = self.0.clone();

        // Just run writing operation on the pool...  The pool and
        // the disk are the (unlikely) bottlenecks, but it is
        // unevitable, unless architectured differently.
        tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
            let mut output = output.lock().unwrap_or_else(|e| e.into_inner());

            serde_json::to_writer(output.deref_mut(), &msg)?;
            writeln!(&mut output)?;

            output.flush()?;
            // Sync may fail if output file is /dev/stdout.  Or one
            // can selectively ignore code 25 ENOTTY.
            let _ = output.sync_all();

            Ok(())
        })
        .await
        .expect("Failed to run a blocking writing operation")
    }
}
