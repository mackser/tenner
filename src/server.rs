use crate::error::Error;
use crate::handler::Handler;
use crate::router::Router;
use log::{error, info};
use std::sync::Arc;
use tokio::{
    io::{AsyncRead, AsyncWrite, BufReader as TokioBufReader, split},
    net::{TcpListener, TcpStream},
};
use tokio_rustls::TlsAcceptor;

#[derive(Clone)]
pub struct Server {
    addr: String,
    router: Arc<Router>,
    tls_acceptor: Option<TlsAcceptor>,
}

impl Server {
    pub fn new(addr: impl Into<String>, tls_acceptor: Option<TlsAcceptor>) -> Self {
        let router = Router::new();
        let router: Arc<Router> = Arc::new(router);

        Self {
            addr: addr.into(),
            router,
            tls_acceptor,
        }
    }

    pub async fn serve_connection(&self, socket: TcpStream, peer: String) -> Result<(), Error> {
        if let Some(acceptor) = &self.tls_acceptor {
            let tls_stream = acceptor.accept(socket).await.map_err(|e| {
                error!("TLS handshake failed for {}: {}", peer, e);
                e
            })?;

            info!("connected (TLS) {}", peer);
            self.handle_client_stream(tls_stream, peer.clone()).await?;
        } else {
            info!("connected (Plain) {}", peer);
            self.handle_client_stream(socket, peer.clone()).await?;
        }

        info!("disconnected {}", peer);
        Ok(())
    }

    async fn handle_client_stream<S>(&self, stream: S, peer: String) -> Result<(), Error>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (tx_private, rx_private) = self.router.new_private_channel();
        self.router.register_client(&peer, tx_private.clone()).await;

        let (read_half, write_half) = split(stream);
        let reader = TokioBufReader::new(read_half);

        let handler = Handler::new(
            reader,
            write_half,
            self.router.clone(),
            tx_private,
            rx_private,
            peer.clone(),
        );

        let res = handler.run().await?;
        self.router.unregister_client(&peer).await;

        Ok(res)
    }

    pub async fn run(self) -> Result<(), Error> {
        let listener = TcpListener::bind(&self.addr).await?;
        let mode = if self.tls_acceptor.is_some() {
            "TLS"
        } else {
            "Plain"
        };

        info!("listening ({}) on {}", mode, &self.addr);

        loop {
            let (socket, addr) = listener.accept().await?;
            let srv = self.clone();
            let peer = addr.to_string();

            tokio::spawn(async move {
                if let Err(e) = srv.serve_connection(socket, peer.clone()).await {
                    error!("client {} error: {}", peer, e);
                }
            });
        }
    }
}
