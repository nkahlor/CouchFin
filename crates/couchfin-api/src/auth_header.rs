//! The `Authorization: MediaBrowser ...` header Jellyfin uses to identify
//! the client, the device and (once logged in) the user's token.

use reqwest::header::HeaderValue;

use crate::ids::DeviceId;
use crate::secret::AccessToken;

/// The client name the server shows for Couchfin's sessions.
pub const CLIENT_NAME: &str = "Couchfin";

/// The device name the server lists under Dashboard > Devices.
pub const DEVICE_NAME: &str = "Couchfin";

/// The client version sent to the server: this crate's version.
pub const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Builds the header value.
///
/// Jellyfin splits the header on commas outside quotes, strips the quotes and
/// URL-decodes each value. The names and version are constants, and
/// `DeviceId` and `AccessToken` only hold URL-unreserved characters, so every
/// value passes through that unchanged and none can close its quotes or add
/// a part. Nothing needs escaping.
pub(crate) fn authorization(device_id: &DeviceId, token: Option<&AccessToken>) -> HeaderValue {
    let mut header = format!(
        "MediaBrowser Client=\"{CLIENT_NAME}\", Device=\"{DEVICE_NAME}\", \
         DeviceId=\"{device_id}\", Version=\"{CLIENT_VERSION}\""
    );
    if let Some(token) = token {
        header.push_str(", Token=\"");
        header.push_str(token.expose_secret());
        header.push('"');
    }
    let mut value =
        HeaderValue::from_str(&header).expect("the header holds only printable ASCII (see above)");
    // Keeps the token out of reqwest's and hyper's debug output.
    value.set_sensitive(true);
    value
}
