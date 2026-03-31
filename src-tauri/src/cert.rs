use std::{sync::Arc, time::Duration};

use anyhow::Result;
use quinn::{ClientConfig, ServerConfig, TransportConfig, VarInt};
use rustls::{
    pki_types::{CertificateDer, PrivatePkcs8KeyDer},
    RootCertStore,
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

pub fn build_client_config(server_cert: &[u8]) -> Result<ClientConfig> {
    let mut roots = RootCertStore::empty();
    roots.add(CertificateDer::from(server_cert.to_vec()))?;

    let mut client_config = ClientConfig::with_root_certificates(Arc::new(roots))?;
    client_config.transport_config(Arc::new(tuned_transport()?));

    Ok(client_config)
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
