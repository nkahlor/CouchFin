//! HTTPS offers HTTP/2 by ALPN (decision 016): with one connection per
//! server, a screenful of posters multiplexes instead of queueing.

mod common;

use std::time::Duration;

use common::tls::{self, trusting};
use couchfin_api::JellyfinApi;

#[tokio::test]
async fn the_client_offers_h2_and_prefers_it() {
    let (server, chosen) = tls::start_alpn_probe();

    // The probe hangs up after the handshake, so the call itself fails.
    let _ = trusting(&server).check_server(&server.url).await;

    let protocol = chosen
        .recv_timeout(Duration::from_secs(5))
        .expect("a handshake happened");
    assert_eq!(protocol.as_deref(), Some(b"h2".as_slice()));
}
