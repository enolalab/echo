use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub struct AppState {
    pub cancel_senders: Arc<Mutex<HashMap<String, broadcast::Sender<()>>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            cancel_senders: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
