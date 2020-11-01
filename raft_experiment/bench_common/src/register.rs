use liblog::storage;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync;

pub struct Register<L> {
    value: sync::Mutex<u64>,
    log_writer: Arc<L>,
}

impl<L> Register<L>
where
    L: storage::LogWriter<u64>,
{
    pub fn new(init: u64, log_writer: L) -> Self {
        Self {
            // TODO: In the real library, value is part of LogWriter
            // and is kept under same lock as AsyncWAL, and
            // modification is applied immediately after it is durably
            // serialized.  But for sketch + benchmark, it is OK,
            // giving our operation is commutative.
            value: sync::Mutex::new(init),
            log_writer: Arc::new(log_writer),
        }
    }

    pub async fn add_value(self: Arc<Self>, add: u64) -> Result<u64, L::Error> {
        self.log_writer
            .command(move |mut w| async move {
                // w.write_all(&add.to_ne_bytes()).await.map(|_| (add, w))
                (
                    w.write_all(&format!("{}\n", add).as_bytes())
                        .await
                        .map(|_| add),
                    w,
                )
            })
            .await?;
        let mut value_guard = self.value.lock().await;
        *value_guard += add;
        Ok(*value_guard)
    }
}
