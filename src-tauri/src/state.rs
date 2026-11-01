use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub struct AppState {
    pub cancel_sender: Arc<Mutex<Option<broadcast::Sender<()>>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            cancel_sender: Arc::new(Mutex::new(None)),
        }
    }
}
