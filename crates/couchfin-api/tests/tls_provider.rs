//! Building a client must not change process-wide TLS state: a library that
//! installs rustls's default crypto provider can clash with whatever else in
//! the process installs one. This test has its own binary, so no other test
//! can install a provider first.

use couchfin_api::{DeviceId, JellyfinClient};
use rustls::crypto::CryptoProvider;

#[test]
fn building_a_client_leaves_the_default_crypto_provider_unset() {
    assert!(CryptoProvider::get_default().is_none(), "precondition");

    let _client = JellyfinClient::new(DeviceId::generate()).unwrap();

    assert!(CryptoProvider::get_default().is_none());
}
