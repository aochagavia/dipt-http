//! This is a http client using Quinn QUIC stack
//! tailored for deep space usage
//! Code has been copied from quinn/examples/client.rs
//! and then modified accordingly
//! While not specific to deepspace, it also skips
//! cert verification
//!

use std::{
    fs,
    io::{self, Write},
    net::{SocketAddr, ToSocketAddrs},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use clap::Parser;
use quinn::{Connection, Endpoint};
use quinn_proto::crypto::rustls::QuicClientConfig;
use quinn_proto::{TransportConfig, VarInt};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio::time::{interval, MissedTickBehavior};
use tracing::{error, info};
use url::Url;
use chrono::Utc;
use crate::common::{set_maxout, set_cc, set_maxrtt};

pub mod common;

/// HTTP/0.9 over QUIC client
#[derive(Parser, Debug)]
#[clap(name = "client")]
struct Opt {
    /// Perform NSS-compatible TLS key logging to the file specified in `SSLKEYLOGFILE`.
    #[clap(long = "keylog")]
    keylog: bool,

    url: Url,

    /// Override hostname used for certificate verification
    #[clap(long = "host")]
    host: Option<String>,

    /// Custom certificate authority to trust, in DER format
    #[clap(long = "ca")]
    ca: Option<PathBuf>,

    /// Simulate NAT rebinding after connecting
    #[clap(long = "rebind")]
    rebind: bool,

    /// Address to bind on
    #[clap(long = "bind", default_value = "[::]:0")]
    bind: SocketAddr,

    // below are our new options that were not in the quinn examples client code

    /// expected max rtt (in seconds) of the connection:
    /// used to set various quic transport parameters
    /// such as: initial_rtt, max_idle_timeout
    #[clap(long = "maxrtt")]
    maxrtt: Option<u64>,

    /// window size in bytes
    #[clap(long = "window")]
    window: Option<u32>,

    /// congestion initial window size in bytes passed to cc
    #[clap(long = "cc_initial_window")]
    ccwindow: Option<u64>,

    /// sets the congestion control: "bbr", "cubic" or "noop".
    /// "noop" is a no operation congestion control (deep space use case)
    #[clap(long = "cc")]
    cc: Option<String>,

    /// if set, do not check the TLS cert from the server
    #[clap(long = "notlsverify", default_value = "false")]
    notlsverify: bool,

    /// repeat x time the same http request within the same connection
    #[clap(long = "repeat")]
    repeat: Option<u32>,

    /// interval in seconds between repeats. default = 1s
    #[clap(long = "repeat-interval")]
    repeat_interval: Option<u64>,

    /// define the alpn to interop with other stacks
    #[clap(long = "alpn")]
    alpn:  Option<String>,

    /// max out most transport config parameters (for debug/testing purposes)
    #[clap(long = "maxout", default_value = "false")]
    maxout: bool,
}

fn main() {
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish(),
    )
    .unwrap();
    let opt = Opt::parse();
    let code = {
        if let Err(e) = run(opt) {
            eprintln!("ERROR: {e}");
            1
        } else {
            0
        }
    };
    std::process::exit(code);
}

#[tokio::main]
async fn run(options: Opt) -> Result<()> {
    let url = options.url;
    let url_host = strip_ipv6_brackets(url.host_str().unwrap());
    let remote = (url_host, url.port().unwrap_or(4433))
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| anyhow!("couldn't resolve to an address"))?;

    let mut roots = rustls::RootCertStore::empty();
    if let Some(ca_path) = options.ca {
        roots.add(CertificateDer::from(fs::read(ca_path)?))?;
    } else {
        let dirs = directories_next::ProjectDirs::from("org", "quinn", "quinn-examples").unwrap();
        match fs::read(dirs.data_local_dir().join("cert.der")) {
            Ok(cert) => {
                roots.add(CertificateDer::from(cert))?;
            }
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                info!("local server certificate not found");
            }
            Err(e) => {
                error!("failed to open local server certificate: {}", e);
            }
        }
    }

    let mut client_crypto = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    if options.notlsverify {
        client_crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(SkipServerVerification::new())
            //.with_root_certificates(roots)
            .with_no_client_auth();
    }

    if let Some(alpn) = options.alpn {
        client_crypto.alpn_protocols = vec![alpn.as_bytes().to_vec()];
    } else {
        client_crypto.alpn_protocols = common::ALPN_QUIC_HTTP.iter().map(|&x| x.into()).collect();
    }
    if options.keylog {
        client_crypto.key_log = Arc::new(rustls::KeyLogFile::new());
    }

    let mut client_config =
        quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(client_crypto)?));

    let mut transport_config = TransportConfig::default();

    if options.maxout {
        set_maxout(&mut transport_config);
    }
    // if user sets maxout = true and also sets other variables,
    // then latter takes precedence
    if let Some(maxrtt) = options.maxrtt {
        set_maxrtt(
            &mut transport_config,
            maxrtt
        );
    }

    if let Some(cc) = options.cc {
        set_cc(&mut transport_config, cc, options.ccwindow);
    }

    if let Some(window) = options.window {
        transport_config.receive_window(VarInt::from_u32(window));
        transport_config.send_window(window.into());
    }

    client_config.transport_config(Arc::new(transport_config));

    let mut endpoint = Endpoint::client("[::]:0".parse().unwrap())?;
    endpoint.set_default_client_config(client_config);

    let request = format!("GET {}\r\n", url.path());
    let start = Instant::now();
    let rebind = options.rebind;
    let host = options.host.as_deref().unwrap_or(url_host);

    info!("connecting to {host} at {remote}");
    let conn = endpoint
        .connect(remote, host)?
        .await
        .map_err(|e| anyhow!("failed to connect: {}", e))?;
    let conn = Arc::new(conn);
    let request = Arc::new(request);
    let endpoint = Arc::new(endpoint);

    info!("connected at {:?}", start.elapsed());
    info!("clock: {:?}", Utc::now());
    let mut repeat = 1;
    if let Some(repeating) = options.repeat { repeat = repeating; }
    let mut repeat_interval = 1;
    if let Some(repeating_interval) = options.repeat_interval { repeat_interval = repeating_interval; }

    let mut ticker = interval(Duration::from_secs(repeat_interval));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    for n in 0..repeat {
        let conn = conn.clone();
        let request = request.clone();
        let endpoint = endpoint.clone();
        let rebind = rebind;

        //tokio::spawn(async move {
            info!(" sending request #{} to remote at: {:?}", n, Utc::now());
            if let Err(e) = perform_request(conn, request, endpoint, rebind, n).await {
                    info!("Request failed: {:?}", e);
            }
        //});
        if repeat > 1 {
            ticker.tick().await;
        }
    }

    conn.close(0u32.into(), b"done");
    info!("total time from start to after close: {:?}", start.elapsed());
    info!("clock: {:?}", Utc::now());
    // Give the server a fair chance to receive the close packet
    //endpoint.wait_idle().await;
    Ok(())
}
async fn perform_request(conn: Arc<Connection>, request: Arc<String>,
                         endpoint: Arc<Endpoint>, rebind: bool, n: u32) -> Result<()> {
    let (mut send, mut recv) = conn
        .open_bi()
        .await
        .map_err(|e| anyhow!("failed to open stream: {}", e))?;
    if rebind {
        let socket = std::net::UdpSocket::bind("[::]:0").unwrap();
        let addr = socket.local_addr().unwrap();
        info!("rebinding to {addr}");
        endpoint.rebind(socket).expect("rebind failed");
    }

    let start = Instant::now();
    info!("request #{}: sending at {:?}", n, Utc::now());

    send.write_all(request.as_bytes())
        .await
        .map_err(|e| anyhow!("failed to send request #{}: {}", n, e))?;

    send.finish()
        .map_err(|e| anyhow!("failed to finish stream of request #{}: {}", n, e))?;

    info!("request #{} sent at {:?}", n, Utc::now());

    let resp = recv
        .read_to_end(usize::MAX)
        .await
        .map_err(|e| anyhow!("failed to read response: {}", e))?;

    let duration = start.elapsed();
    info!(
        "request #{}: response received in {:?} from request start",
        n,
        duration
    );
    io::stdout().write_all(&resp)?;
    io::stdout().flush()?;
    info!("request #{}: duration: {:?}", n, duration);
    info!("request #{}: clock: {:?}", n, Utc::now());
    Ok(())
}

fn strip_ipv6_brackets(host: &str) -> &str {
    // An ipv6 url looks like eg https://[::1]:4433/Cargo.toml, wherein the host [::1] is the
    // ipv6 address ::1 wrapped in brackets, per RFC 2732. This strips those.
    if host.starts_with('[') && host.ends_with(']') {
        &host[1..host.len() - 1]
    } else {
        host
    }
}

// copied from insecure_connection.rs. no it is not the right way to do this, but for now, fast enabling testing.
//
// Dummy certificate verifier that treats any certificate as valid.
/// NOTE, such verification is vulnerable to MITM attacks, but convenient for testing.
#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
