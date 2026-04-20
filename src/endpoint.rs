use crate::message::Message;
use crate::router::Router;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

pub struct BroadcastEndpoint {
    router: Arc<Router>,
    receiver: Option<broadcast::Receiver<Message>>,
}

impl BroadcastEndpoint {
    pub fn with_receiver(router: Arc<Router>) -> Self {
        Self {
            receiver: Some(router.subscribe()),
            router,
        }
    }

    pub fn send_only(router: Arc<Router>) -> Self {
        Self { receiver: None, router }
    }

    pub fn send(&self, msg: Message) -> Result<(), broadcast::error::SendError<Message>> {
        self.router.broadcast(msg)
    }

    pub async fn recv(&mut self) -> Result<Message, broadcast::error::RecvError> {
        match &mut self.receiver {
            Some(r) => r.recv().await,
            None => Err(broadcast::error::RecvError::Closed),
        }
    }
}

impl Clone for BroadcastEndpoint {
    fn clone(&self) -> Self {
        BroadcastEndpoint {
            router: self.router.clone(),
            receiver: None,
        }
    }
}

pub struct PrivateEndpoint {
    sender: Option<mpsc::Sender<Message>>,
    receiver: Option<mpsc::Receiver<Message>>,
}

impl PrivateEndpoint {
    pub fn with_sender_receiver(s: mpsc::Sender<Message>, r: mpsc::Receiver<Message>) -> Self {
        Self {
            sender: Some(s),
            receiver: Some(r),
        }
    }

    pub fn try_send(&self, msg: Message) -> Result<(), mpsc::error::TrySendError<Message>> {
        match &self.sender {
            Some(s) => s.try_send(msg),
            None => Err(mpsc::error::TrySendError::Closed(msg)),
        }
    }

    pub async fn send(&self, msg: Message) -> Result<(), mpsc::error::SendError<Message>> {
        match &self.sender {
            Some(s) => s.send(msg).await,
            None => Err(tokio::sync::mpsc::error::SendError(msg)),
        }
    }

    pub async fn recv(&mut self) -> Option<Message> {
        match &mut self.receiver {
            Some(r) => r.recv().await,
            None => None,
        }
    }
}

impl Clone for PrivateEndpoint {
    fn clone(&self) -> Self {
        PrivateEndpoint {
            sender: self.sender.clone(),
            receiver: None,
        }
    }
}
