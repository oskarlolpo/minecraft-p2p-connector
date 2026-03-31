use std::{sync::Arc, time::Duration};

use anyhow::Result;
use quinn::{ClientConfig, ServerConfig, TransportConfig, VarInt};
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider},
    pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
    DigitallySignedStruct, SignatureScheme,
};

pub fn build_server_config() -> Result<(ServerConfig, Vec<u8>)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into(), "mc-p2p.local".into()])?;
    let cert_der = CertificateDer::from(cert.cert);
    let cert_bytes = cert_der.as_ref().to_vec();
    let private_key = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());

    let mut server_config = ServerConfig::with_single_cert(vec![cert_der], private_key.into())?;
    server_config.transport_config(Arc::new(tuned_transport()?));

    Ok((server_config, cert_bytes))
}

pub fn build_insecure_client_config() -> Result<ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let rustls_config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipServerVerification { provider }))
        .with_no_client_auth();

    let mut client_config = ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(rustls_config)?,
    ));
    client_config.transport_config(Arc::new(tuned_transport()?));
    Ok(client_config)
}

#[derive(Debug)]
struct SkipServerVerification {
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
            .map(|_| HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
            .map(|_| HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

fn tuned_transport() -> Result<TransportConfig> {
    let mut transport = TransportConfig::default();
    transport.max_concurrent_uni_streams(0_u8.into());
    transport.keep_alive_interval(Some(Duration::from_secs(2)));
    transport.max_idle_timeout(Some(Duration::from_secs(20).try_into()?));
    transport.stream_receive_window(VarInt::from_u32(2 * 1024 * 1024));
    transport.receive_window(VarInt::from_u32(8 * 1024 * 1024));
    transport.send_window(8 * 1024 * 1024);
    transport.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
    Ok(transport)
}
