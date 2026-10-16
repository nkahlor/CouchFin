//! A tiny HTTPS server for the redirect tests, since wiremock only speaks
//! plain HTTP. It serves a certificate for 127.0.0.1 signed by a throwaway
//! CA, which the client trusts through
//! `JellyfinClientBuilder::add_root_certificates_pem`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use couchfin_api::ServerUrl;
use rcgen::{BasicConstraints, CertificateParams, CertifiedIssuer, IsCa, KeyPair};
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};

pub struct TlsServer {
    /// `https://127.0.0.1:<port>/`
    pub url: ServerUrl,
    /// The CA certificate that signed the server's certificate.
    pub ca_pem: String,
}

/// Starts a server that answers every request with `respond(method, path)`,
/// a full HTTP/1.1 response such as `"HTTP/1.1 301 Moved\r\nLocation: ...\r\n"`
/// (the server adds `Content-Length: 0` and `Connection: close` when the
/// response has no blank line yet).
pub fn start(respond: impl Fn(&str, &str) -> String + Send + 'static) -> TlsServer {
    let ca_key = KeyPair::generate().expect("CA key");
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).expect("CA params");
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca = CertifiedIssuer::self_signed(ca_params, ca_key).expect("CA certificate");

    let leaf_key = KeyPair::generate().expect("server key");
    let leaf = CertificateParams::new(vec!["127.0.0.1".to_owned()])
        .expect("server params")
        .signed_by(&leaf_key, &ca)
        .expect("server certificate");

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("TLS versions")
        .with_no_client_auth()
        .with_single_cert(
            vec![leaf.der().clone()],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der())),
        )
        .expect("server TLS config");
    let config = Arc::new(config);

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("address").port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let connection = ServerConnection::new(Arc::clone(&config)).expect("connection");
            let tls = StreamOwned::new(connection, stream);
            // A failed handshake or a client hanging up is the test's
            // business, not the server's.
            let _ = serve(tls, &respond);
        }
    });

    TlsServer {
        url: format!("https://127.0.0.1:{port}")
            .parse()
            .expect("HTTPS URL"),
        ca_pem: ca.pem(),
    }
}

fn serve(
    tls: StreamOwned<ServerConnection, std::net::TcpStream>,
    respond: &impl Fn(&str, &str) -> String,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(tls);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();

    let mut content_length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;

    let mut response = respond(&method, &path);
    if !response.contains("\r\n\r\n") {
        response.push_str("Content-Length: 0\r\nConnection: close\r\n\r\n");
    }
    let mut tls = reader.into_inner();
    tls.write_all(response.as_bytes())?;
    tls.conn.send_close_notify();
    tls.flush()
}

/// A `200 OK` response with a JSON body.
pub fn ok_json(body: &serde_json::Value) -> String {
    let body = body.to_string();
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

/// A redirect response without a body.
pub fn redirect(status: u16, location: &str) -> String {
    format!("HTTP/1.1 {status} Redirect\r\nLocation: {location}\r\n")
}
