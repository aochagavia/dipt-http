//! Commonly used code in most examples.

pub mod rate_limit_based_cc;
pub mod noop_cc;

use quinn::{ClientConfig, Endpoint, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};

use std::{error::Error, net::SocketAddr, sync::Arc};

/// Constructs a QUIC endpoint configured for use a client only.
///
/// ## Args
///
/// - server_certs: list of trusted certificates.
#[allow(unused)]
pub fn make_client_endpoint(
    bind_addr: SocketAddr,
    server_certs: &[&[u8]],
) -> Result<Endpoint, Box<dyn Error + Send + Sync + 'static>> {
    let client_cfg = configure_client(server_certs)?;
    let mut endpoint = Endpoint::client(bind_addr)?;
    endpoint.set_default_client_config(client_cfg);
    Ok(endpoint)
}

/// Constructs a QUIC endpoint configured to listen for incoming connections on a certain address
/// and port.
///
/// ## Returns
///
/// - a stream of incoming QUIC connections
/// - server certificate serialized into DER format
#[allow(unused)]
pub fn make_server_endpoint(
    bind_addr: SocketAddr,
) -> Result<(Endpoint, CertificateDer<'static>), Box<dyn Error + Send + Sync + 'static>> {
    let (server_config, server_cert) = configure_server()?;
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok((endpoint, server_cert))
}

/// Builds default quinn client config and trusts given certificates.
///
/// ## Args
///
/// - server_certs: a list of trusted certificates in DER format.
fn configure_client(
    server_certs: &[&[u8]],
) -> Result<ClientConfig, Box<dyn Error + Send + Sync + 'static>> {
    let mut certs = rustls::RootCertStore::empty();
    for cert in server_certs {
        certs.add(CertificateDer::from(*cert))?;
    }

    Ok(ClientConfig::with_root_certificates(Arc::new(certs))?)
}

/// Returns default server configuration along with its certificate.
fn configure_server()
-> Result<(ServerConfig, CertificateDer<'static>), Box<dyn Error + Send + Sync + 'static>> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = CertificateDer::from(cert.cert);
    let priv_key = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());

    let mut server_config =
        ServerConfig::with_single_cert(vec![cert_der.clone()], priv_key.into())?;
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());

    Ok((server_config, cert_der))
}

#[allow(unused)]
pub const ALPN_QUIC_HTTP: &[&[u8]] = &[b"hq-29"];

use quinn::TransportConfig;
use quinn::VarInt;
use std::time::Duration;
use quinn_proto::{AckFrequencyConfig, IdleTimeout, MtuDiscoveryConfig};
use quinn_proto::congestion::{BbrConfig, CubicConfig};
use crate::common::noop_cc::NoopCCConfig;
use crate::common::rate_limit_based_cc::RateLimitBasedCCConfig;

// sets transport to max out config parameters
pub fn set_maxout(transport_config: &mut TransportConfig) {
    transport_config.max_idle_timeout(Some(VarInt::MAX.into()));
    transport_config.initial_rtt(Duration::new(100_000, 0));
    transport_config.receive_window(VarInt::MAX);
    transport_config.datagram_send_buffer_size(usize::MAX);
    transport_config.send_window(u64::MAX);
    transport_config.datagram_receive_buffer_size(Some(usize::MAX));
    transport_config.stream_receive_window(VarInt::MAX);
    transport_config.congestion_controller_factory(Arc::new(NoopCCConfig::default()));
    let mut ack_frequency_config = AckFrequencyConfig::default();
    ack_frequency_config.max_ack_delay(Some(Duration::MAX));
    transport_config.ack_frequency_config(Some(ack_frequency_config));
    // disable mtu discovery
    let mut mtu_discovery_config = MtuDiscoveryConfig::default();
    mtu_discovery_config.upper_bound(1200);  //should be INITIAL_MTU
    mtu_discovery_config.interval(Duration::new(1000000,0));
    transport_config.mtu_discovery_config(Some(mtu_discovery_config));
    // max_concurrent_*_streams to VarInt::MAX crashes the process
    //transport_config.max_concurrent_bidi_streams(VarInt::MAX);
    //transport_config.max_concurrent_uni_streams(VarInt::MAX);
    transport_config.packet_threshold(u32::MAX);
    // setting time_threshold to f32::MAX creates a crash at runtime
    // so setting a large value
    transport_config.time_threshold(100000.0);
    // connection_id pool
}

pub fn set_maxrtt(transport_config: &mut TransportConfig, maxrtt: u64) {
    transport_config.max_idle_timeout(Some(
        // maxrtt in seconds, but idletimeout is expecting ms
        IdleTimeout::from(VarInt::from_u64(maxrtt * 1000).unwrap())));
    transport_config.initial_rtt(Duration::new(maxrtt, 0));
}

pub fn set_cc(transport_config: &mut TransportConfig, cc: String, ccwin: Option<u64>) {
    let mut window = 100000000;
    if let Some(ccwin) = ccwin {
        window = ccwin;
    }
    if cc == "bbr" {
        let mut bbr_config = BbrConfig::default();
        bbr_config.initial_window(window);
        transport_config.congestion_controller_factory(Arc::new(bbr_config));
    } else if cc == "cubic" {
        let mut cubic_config = CubicConfig::default();
        cubic_config.initial_window(window);
        transport_config.congestion_controller_factory(Arc::new(cubic_config));
    } else if cc == "noop" {
        transport_config.congestion_controller_factory(Arc::new(NoopCCConfig::default()));
    } else if cc == "rate-limit" {
        transport_config.max_bytes_per_second(Some(1_000 * 1_000 * 25 / 8)); // 25 Mbps
        transport_config.congestion_controller_factory(Arc::new(RateLimitBasedCCConfig));
    }
}