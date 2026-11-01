use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use auto_subtitle::settings::SettingsStore;

pub struct AppState {
    pub cancel_senders: Arc<Mutex<HashMap<String, broadcast::Sender<()>>>>,
    pub settings: Arc<std::sync::Mutex<SettingsStore>>,
}

impl AppState {
    pub fn new(settings: SettingsStore) -> Self {
        Self {
            cancel_senders: Arc::new(Mutex::new(HashMap::new())),
            settings: Arc::new(std::sync::Mutex::new(settings)),
        }
    }
}
