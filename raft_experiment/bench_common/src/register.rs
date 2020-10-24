use std::sync::Arc;
use std::io;
use tokio::sync;
use tokio::io::AsyncWriteExt;
use liblog::storage;


pub struct Register<AWAL, L> {
    value: sync::Mutex<u64>,
    log_writer: Arc<L>,
    _menace: std::marker::PhantomData<AWAL>,
}

impl<AWAL, L>  Register<AWAL, L>
where AWAL: storage::AsyncWAL,
      L: storage::LogWriter<AWAL>,
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
            _menace: Default::default()
        }
    }

    pub async fn add_value(self: Arc<Self>, add: u64) -> io::Result<u64> {
        self.log_writer.command(move |mut w| async move {
            // w.write_all(&add.to_ne_bytes()).await.map(|_| (add, w))
            w.write_all(&format!("{}\n", add).as_bytes()).await.map(|_| (add, w))
        }).await?;
        let mut value_guard = self.value.lock().await;
        *value_guard += add;
        Ok(*value_guard)
    }
}
