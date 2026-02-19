use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("TLS Error: {0}")]
    Tls(#[from] tokio_rustls::rustls::Error),
}
