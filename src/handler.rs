use crate::error::Error;
use crate::message::Message;
use crate::router::Router;
use log::{debug, warn};
use std::sync::Arc;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc};

pub struct Peer {
    pub addr: String,
    pub nick: String,
}

impl Peer {
     
}

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
        Self {
            receiver: None,
            router,
        }
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

pub struct Handler<R, W>
where
    R: AsyncBufRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    reader: R,
    writer: W,
    router: Arc<Router>,
    broadcast: BroadcastEndpoint,
    private: PrivateEndpoint,
    peer: String,
}

pub struct HandlerReader<R>
where
    R: AsyncBufRead + Unpin + Send + 'static,
{
    pub reader: R,
    pub router: Arc<Router>,
    pub broadcast: BroadcastEndpoint,
    pub private: PrivateEndpoint,
    pub peer: String,
}

pub struct HandlerWriter<W>
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    pub writer: W,
    pub broadcast: BroadcastEndpoint,
    pub private: PrivateEndpoint,
    pub peer: String,
}

impl<R, W> Handler<R, W>
where
    R: AsyncBufRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(
        reader: R,
        writer: W,
        router: Arc<Router>,
        private_tx: mpsc::Sender<Message>,
        private_rx: mpsc::Receiver<Message>,
        peer: String,
    ) -> Self {
        let broadcast_with_receiver = BroadcastEndpoint::with_receiver(router.clone());
        let private_both = PrivateEndpoint::with_sender_receiver(private_tx, private_rx);

        Self {
            reader,
            writer,
            router,
            broadcast: broadcast_with_receiver,
            private: private_both,
            peer,
        }
    }

    pub fn split(self) -> (HandlerReader<R>, HandlerWriter<W>) {
        let reader_part = HandlerReader {
            reader: self.reader,
            router: self.router.clone(),
            broadcast: self.broadcast.clone(),
            private: self.private.clone(),
            peer: self.peer.clone(),
        };

        let writer_part = HandlerWriter {
            writer: self.writer,
            broadcast: self.broadcast,
            private: self.private,
            peer: self.peer,
        };

        (reader_part, writer_part)
    }

    pub async fn run(self) -> Result<(), Error> {
        let (reader_part, writer_part) = self.split();

        tokio::try_join!(async move { reader_part.reader_loop().await }, async move {
            writer_part.writer_loop().await
        })?;

        Ok(())
    }
}

impl<R> HandlerReader<R>
where
    R: AsyncBufRead + Unpin + Send + 'static,
{
    pub async fn reader_loop(mut self) -> Result<(), Error> {
        let mut buf = String::new();

        loop {
            buf.clear();
            let n = self.reader.read_line(&mut buf).await?;
            if n == 0 {
                return Ok(());
            }

            let line = buf.trim_end().to_string();
            debug!("recv {}: {}", self.peer, line);

            if line.starts_with('/') {
                let rest = &line[1..];
                let mut parts = rest.splitn(2, ' ');
                let cmd = parts.next().unwrap_or("").trim();
                let args = parts.next().unwrap_or("").trim();
                self.handle_command(cmd, args).await;
                continue;
            }

            self.broadcast_text(line).await;
        }
    }

    async fn handle_command(&self, cmd: &str, args: &str) {
        match cmd {
            "msg" => {
                let mut parts = args.splitn(2, ' ');
                let target = parts.next().unwrap_or("").trim();
                let body = parts.next().unwrap_or("").trim();

                if target.is_empty() || body.is_empty() {
                    let _ = self.private.try_send(Message::system(
                        "usage: /msg <target> <message>".to_string(),
                    ));
                    return;
                }

                match self.router.send_private(&self.peer, target, body).await {
                    Ok(()) => {
                        let _ = self
                            .private
                            .try_send(Message::system(format!("private to {} sent", target)));
                    }
                    Err(e) => {
                        let _ = self.private.try_send(Message::system(format!("{}", e)));
                    }
                }
            }

            "echo" => {
                let _ = self
                    .private
                    .try_send(Message::system(format!("echo {}", args)));
            }

            "list" => {
                let list = self.router.list_clients().await;
                let _ = self
                    .private
                    .try_send(Message::system(format!("clients [{}]", list)));
            }

            _ => {
                let _ = self
                    .private
                    .try_send(Message::system(format!("unknown command '/{}'", cmd)));
            }
        }

        debug!("Handled command from {}: /{} {}", self.peer, cmd, args);
    }

    async fn broadcast_text(&self, text: String) {
        let msg = Message::broadcast(self.peer.clone(), text);
        if let Err(e) = self.broadcast.send(msg) {
            warn!("failed to broadcast message from {}: {:?}", self.peer, e);
        }
    }
}

impl<W> HandlerWriter<W>
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    pub async fn writer_loop(mut self) -> Result<(), Error> {
        loop {
            tokio::select! {
                biased;

                maybe_priv = self.private.recv() => {
                    if let Some(msg) = maybe_priv {
                        self.write_out(msg).await?;
                    } else {
                        return Ok(());
                    }
                }

                res = self.broadcast.recv() => {
                    match res {
                        Ok(msg) => {
                            if let Message::Broadcast { ref from, .. } = msg {
                                if from == &self.peer { continue; }
                                self.write_out(msg).await?;
                            } else {
                                log::warn!("handler: ignoring non-broadcast on broadcast channel for {}", self.peer);
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            debug!("{} missed {} broadcast messages", self.peer, n);
                        }
                        Err(broadcast::error::RecvError::Closed) => return Ok(()),
                    }
                }
            }
        }
    }

    async fn write_out(&mut self, msg: Message) -> Result<(), Error> {
        let out = msg.to_display_for(&self.peer);
        self.writer.write_all(out.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        Ok(())
    }
}
