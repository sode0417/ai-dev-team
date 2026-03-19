use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

#[derive(Clone)]
pub struct WsHub {
    channels: Arc<RwLock<HashMap<Uuid, broadcast::Sender<String>>>>,
    global_sender: broadcast::Sender<String>,
}

impl WsHub {
    pub fn new() -> Self {
        let (global_sender, _) = broadcast::channel(64);
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            global_sender,
        }
    }

    pub async fn subscribe(&self, task_id: Uuid) -> broadcast::Receiver<String> {
        let mut channels = self.channels.write().await;
        let sender = channels
            .entry(task_id)
            .or_insert_with(|| broadcast::channel(64).0);
        sender.subscribe()
    }

    pub async fn broadcast(&self, task_id: Uuid, message: &str) {
        let channels = self.channels.read().await;
        if let Some(sender) = channels.get(&task_id) {
            let _ = sender.send(message.to_string());
        }
    }

    pub async fn remove_channel(&self, task_id: &Uuid) {
        self.channels.write().await.remove(task_id);
    }

    pub fn subscribe_global(&self) -> broadcast::Receiver<String> {
        self.global_sender.subscribe()
    }

    pub fn broadcast_global(&self, message: &str) {
        let _ = self.global_sender.send(message.to_string());
    }
}
