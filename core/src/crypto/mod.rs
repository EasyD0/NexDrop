//! Device identity: a self-signed TLS certificate created on first run and
//! reused forever, so its SHA-256 fingerprint acts as a stable device key
//! (TOFU pinning happens in `pairing`).

use anyhow::Context;
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use sha2::{Digest, Sha256};

/// Long-lived device identity, persisted as raw DER next to the settings.
#[derive(Clone)]
pub struct Identity {
    pub cert_der: Vec<u8>,
    pub key_der: Vec<u8>,
    /// Hex SHA-256 of `cert_der`, used as the device id and pin.
    pub fingerprint: String,
}

impl Identity {
    /// Load `cert.der`/`key.der` from `dir`, creating them on first run.
    pub fn load_or_create(dir: &std::path::Path) -> anyhow::Result<Self> {
        let cert_path = dir.join("cert.der");
        let key_path = dir.join("key.der");
        let (cert_der, key_der) = match (std::fs::read(&cert_path), std::fs::read(&key_path)) {
            (Ok(c), Ok(k)) if !c.is_empty() && !k.is_empty() => (c, k),
            _ => {
                let key = rcgen::generate_simple_self_signed(vec!["nexdrop".into()])
                    .context("generate identity cert")?;
                let cert_der = key.cert.der().to_vec();
                let key_der = key.signing_key.serialize_der();
                std::fs::create_dir_all(dir)?;
                std::fs::write(&cert_path, &cert_der)?;
                std::fs::write(&key_path, &key_der)?;
                (cert_der, key_der)
            }
        };
        let mut hasher = Sha256::new();
        hasher.update(&cert_der);
        Ok(Self {
            fingerprint: format!("{:x}", hasher.finalize()),
            cert_der,
            key_der,
        })
    }

    /// TLS config for the listening side. Peers are authenticated out-of-band
    /// (pairing code + TOFU fingerprint), so the handshake accepts anything.
    pub fn tls_server_config(&self) -> anyhow::Result<rustls::ServerConfig> {
        let certs = vec![CertificateDer::from(self.cert_der.clone())];
        let key = PrivatePkcs8KeyDer::from(self.key_der.clone());
        let mut cfg = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key.into())?;
        cfg.alpn_protocols = crate::model::ALPN
            .iter()
            .map(|p| p.as_bytes().to_vec())
            .collect();
        Ok(cfg)
    }

    /// TLS config for the dialing side, accepting any cert (TOFU, see above).
    pub fn tls_client_config(&self) -> anyhow::Result<rustls::ClientConfig> {
        let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
        let mut cfg = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()?
            .dangerous()
            .with_custom_certificate_verifier(std::sync::Arc::new(NoVerify))
            .with_no_client_auth();
        cfg.alpn_protocols = crate::model::ALPN
            .iter()
            .map(|p| p.as_bytes().to_vec())
            .collect();
        Ok(cfg)
    }
}

#[derive(Debug)]
struct NoVerify;

impl rustls::client::danger::ServerCertVerifier for NoVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        use rustls::SignatureScheme::*;
        vec![
            RSA_PKCS1_SHA256,
            RSA_PKCS1_SHA384,
            RSA_PKCS1_SHA512,
            ECDSA_NISTP256_SHA256,
            ECDSA_NISTP384_SHA384,
            ECDSA_NISTP521_SHA512,
            RSA_PSS_SHA256,
            RSA_PSS_SHA384,
            RSA_PSS_SHA512,
            ED25519,
        ]
    }
}

/// Hex SHA-256 fingerprint of a certificate DER blob.
pub fn fingerprint(cert_der: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cert_der);
    format!("{:x}", hasher.finalize())
}
