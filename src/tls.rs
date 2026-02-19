use std::{collections::HashMap, fs::File, io::BufReader, path::Path, sync::Arc};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        ServerConfig,
        pki_types::{CertificateDer, PrivateKeyDer},
    },
};

use crate::Error;

pub(crate) fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>, Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}

pub(crate) fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>, Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    loop {
        match rustls_pemfile::read_one(&mut reader)? {
            Some(rustls_pemfile::Item::Pkcs1Key(key)) => return Ok(key.into()),
            Some(rustls_pemfile::Item::Pkcs8Key(key)) => return Ok(key.into()),
            Some(rustls_pemfile::Item::Sec1Key(key)) => return Ok(key.into()),
            None => break,
            _ => continue,
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, "private key not found").into())
}

pub(crate) fn make_tls_acceptor(
    cert_path: impl AsRef<Path>,
    key_path: impl AsRef<Path>,
) -> Result<TlsAcceptor, Error> {
    let certs = load_certs(cert_path.as_ref())?;
    let key = load_key(key_path.as_ref())?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}
