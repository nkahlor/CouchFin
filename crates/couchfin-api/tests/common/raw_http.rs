//! A plain-HTTP server that sends fixed bytes, for replies wiremock can't
//! produce, such as a body shorter than its `Content-Length`.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use couchfin_api::ServerUrl;

/// Starts a server that reads each request's headers, writes `response`,
/// then waits `hold` before closing the connection.
pub fn start(response: &'static str, hold: Duration) -> ServerUrl {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("address").port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            // Requests here are GETs, so the headers end at the first blank line.
            while reader.read_line(&mut line).is_ok_and(|n| n > 0) && line != "\r\n" {
                line.clear();
            }
            let mut stream = reader.into_inner();
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
            thread::sleep(hold);
        }
    });
    format!("http://127.0.0.1:{port}")
        .parse()
        .expect("local URL is valid")
}
