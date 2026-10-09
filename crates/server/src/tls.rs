//! Development TLS: a short-lived self-signed certificate whose SHA-256 hash the browser pins
//! through `serverCertificateHashes` (WebTransport allows at most 14 days of validity).

use anyhow::Result;
use rcgen::{CertificateParams, KeyPair};
use salvo::conn::rustls::{Keycert, RustlsConfig};
use salvo::prelude::*;
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};

#[derive(Debug)]
pub struct CertificateHash([u8; 32]);

#[handler]
impl CertificateHash {
    async fn handle(&self) -> Json<[u8; 32]> {
        Json(self.0)
    }
}

pub fn local_certificate() -> Result<(RustlsConfig, CertificateHash)> {
    let mut params = CertificateParams::new(vec!["localhost".into(), "127.0.0.1".into()])?;
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::minutes(1);
    params.not_after = now + Duration::days(13);
    let key = KeyPair::generate()?;
    let certificate = params.self_signed(&key)?;
    let hash = Sha256::digest(certificate.der().as_ref()).into();
    let config = RustlsConfig::new(
        Keycert::new().cert(certificate.pem().into_bytes()).key(key.serialize_pem().into_bytes()),
    );
    Ok((config, CertificateHash(hash)))
}
