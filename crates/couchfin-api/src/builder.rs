//! Configuring a [`JellyfinClient`]: timeout and extra trusted certificates.

use std::sync::Arc;
use std::time::Duration;

use reqwest::redirect::Policy;
use rustls::ClientConfig;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;
use rustls_platform_verifier::Verifier;

use crate::client::JellyfinClient;
use crate::ids::DeviceId;

/// Building the HTTP client failed: TLS couldn't be set up on this system,
/// or a certificate given to the builder isn't valid PEM.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("couldn't set up the HTTP client: {0}")]
pub struct BuildError(String);

/// Configures a [`JellyfinClient`]; start from [`JellyfinClient::builder`].
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use couchfin_api::{DeviceId, JellyfinClient};
///
/// let client = JellyfinClient::builder(DeviceId::generate())
///     .timeout(Duration::from_secs(5))
///     .build()?;
/// # drop(client);
///
/// // Something that isn't a PEM certificate is refused up front.
/// let refused = JellyfinClient::builder(DeviceId::generate())
///     .add_root_certificates_pem(b"not a certificate");
/// assert!(refused.is_err());
/// # Ok::<(), couchfin_api::BuildError>(())
/// ```
#[derive(Debug)]
pub struct JellyfinClientBuilder {
    device_id: DeviceId,
    timeout: Duration,
    roots: Vec<CertificateDer<'static>>,
}

impl JellyfinClientBuilder {
    pub(crate) fn new(device_id: DeviceId) -> Self {
        Self {
            device_id,
            timeout: JellyfinClient::DEFAULT_TIMEOUT,
            roots: Vec::new(),
        }
    }

    /// How long a request may take, from connecting to reading the whole
    /// reply. Defaults to [`JellyfinClient::DEFAULT_TIMEOUT`].
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Trusts the CA certificates in `pem` (one or more `BEGIN CERTIFICATE`
    /// blocks) as well as the system's, for a server whose certificate comes
    /// from a home CA or is self-signed.
    ///
    /// # Errors
    ///
    /// [`BuildError`] if `pem` holds no certificate or isn't valid PEM.
    pub fn add_root_certificates_pem(mut self, pem: &[u8]) -> Result<Self, BuildError> {
        let certs = CertificateDer::pem_slice_iter(pem)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| BuildError(format!("invalid PEM certificate: {e}")))?;
        if certs.is_empty() {
            return Err(BuildError("no PEM certificate found".to_owned()));
        }
        self.roots.extend(certs);
        Ok(self)
    }

    /// Builds the client.
    ///
    /// # Errors
    ///
    /// [`BuildError`] if TLS can't be set up, for example if the system's
    /// trust store can't be read.
    pub fn build(self) -> Result<JellyfinClient, BuildError> {
        let http = reqwest::Client::builder()
            .timeout(self.timeout)
            // Each call handles redirects itself (see redirect.rs): following
            // them silently turns a login POST into an anonymous GET.
            .redirect(Policy::none())
            .tls_backend_preconfigured(tls_config(self.roots)?)
            .build()
            .map_err(|e| BuildError(e.to_string()))?;
        Ok(JellyfinClient::from_parts(http, self.device_id))
    }
}

/// The TLS setup, with ring passed in explicitly. Leaving it to reqwest would
/// mean installing ring as rustls's process-wide default provider, global
/// state a library shouldn't touch, and which silently loses if something
/// else installs a provider first.
///
/// Certificates are checked the way reqwest checks them by default: by
/// rustls-platform-verifier, against the system trust store plus `roots`.
fn tls_config(roots: Vec<CertificateDer<'static>>) -> Result<ClientConfig, BuildError> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = Verifier::new_with_extra_roots(roots, Arc::clone(&provider))
        .map_err(|e| BuildError(format!("couldn't load trusted certificates: {e}")))?;
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| BuildError(e.to_string()))?
        // "dangerous" only because it takes a custom verifier; this one does
        // full certificate and hostname verification.
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok(config)
}
