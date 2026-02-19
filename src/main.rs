use env_logger::Env;
use error::Error;
pub mod endpoint;
pub mod error;
pub mod handler;
pub mod message;
pub mod router;
pub mod server;
pub mod tls;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let use_tls = true;

    let acceptor = if use_tls {
        Some(tls::make_tls_acceptor("cert.pem", "key.pem")?)
    } else {
        None
    };

    let server = server::Server::new("127.0.0.1:8080", acceptor);
    server.run().await
}
