use crate::message::Message;
use log::{debug, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};

const BROADCAST_CAPACITY: usize = 64;
const PRIVATE_CAPACITY: usize = 64;

#[derive(Clone)]
pub struct Router {
    broadcaster: broadcast::Sender<Message>,
    clients: Arc<Mutex<HashMap<String, mpsc::Sender<Message>>>>,
}

impl Router {
    pub fn new() -> Self {
        let (broadcaster, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            broadcaster,
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Message> {
        self.broadcaster.subscribe()
    }

    pub fn broadcast(&self, msg: Message) -> Result<(), broadcast::error::SendError<Message>> {
        log::debug!("router: broadcasting message: {:?}", msg);
        self.broadcaster.send(msg).map(|_| ())
    }

    pub fn new_private_channel(&self) -> (mpsc::Sender<Message>, mpsc::Receiver<Message>) {
        mpsc::channel(PRIVATE_CAPACITY)
    }

    pub async fn register_client(&self, peer: &str, tx: mpsc::Sender<Message>) {
        let mut clients = self.clients.lock().await;
        clients.insert(peer.to_string(), tx);
        debug!("router: registered client {}", peer);
    }

    pub async fn unregister_client(&self, peer: &str) {
        let mut clients = self.clients.lock().await;
        clients.remove(peer);
        debug!("router: unregistered client {}", peer);
    }

    pub async fn list_clients(&self) -> String {
        let clients = self.clients.lock().await;
        clients.keys().cloned().collect::<Vec<_>>().join(", ")
    }

    pub async fn send_private(&self, from: &str, to: &str, body: &str) -> Result<(), String> {
        let tx_opt = {
            let clients = self.clients.lock().await;
            clients.get(to).cloned()
        };

        if let Some(tx) = tx_opt {
            let msg = Message::private(from.to_string(), body.to_string());

            match tx.send(msg.clone()).await {
                Ok(()) => {
                    log::debug!("router: delivered private message from {} to {}", from, to);
                    Ok(())
                }
                Err(tokio::sync::mpsc::error::SendError(_msg)) => {
                    log::debug!("router: private receiver closed for {}", to);
                    Err(format!("user '{}' not connected", to))
                }
            }
        } else {
            log::debug!("router: send_private target not found: {}", to);
            Err(format!("user '{}' not connected", to))
        }
    }
}
