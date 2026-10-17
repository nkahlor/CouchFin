//! Newtypes: server URL normalisation, device ID round-trip, and secrets that
//! never show up in `Debug` output.

use couchfin_api::{
    AccessToken, DeviceId, Password, ServerId, ServerUrl, ServerUrlError, Session, UserId,
};

fn url(input: &str) -> Result<String, ServerUrlError> {
    input.parse::<ServerUrl>().map(|u| u.as_str().to_owned())
}

#[test]
fn server_url_adds_a_trailing_slash() {
    assert_eq!(
        url("http://jellyfin.lan:8096").unwrap(),
        "http://jellyfin.lan:8096/"
    );
    assert_eq!(
        url("https://example.com/jellyfin").unwrap(),
        "https://example.com/jellyfin/"
    );
}

#[test]
fn server_url_defaults_to_http_and_trims_whitespace() {
    assert_eq!(
        url("  192.168.1.20:8096 ").unwrap(),
        "http://192.168.1.20:8096/"
    );
}

#[test]
fn server_url_drops_the_web_client_path_query_and_fragment() {
    assert_eq!(
        url("http://jellyfin.lan:8096/web/index.html#!/home.html").unwrap(),
        "http://jellyfin.lan:8096/"
    );
    assert_eq!(
        url("https://example.com/jellyfin/web/?x=1").unwrap(),
        "https://example.com/jellyfin/"
    );
}

#[test]
fn server_url_lowercases_the_scheme_and_host() {
    assert_eq!(
        url("HTTP://Jellyfin.LAN:8096").unwrap(),
        "http://jellyfin.lan:8096/"
    );
}

#[test]
fn server_url_rejects_bad_input() {
    assert_eq!(url("   "), Err(ServerUrlError::Empty));
    assert_eq!(
        url("ftp://jellyfin.lan"),
        Err(ServerUrlError::UnsupportedScheme("ftp".into()))
    );
    assert_eq!(
        url("http://alice:pw@jellyfin.lan"),
        Err(ServerUrlError::HasCredentials)
    );
    assert!(matches!(
        url("http://exa mple.com"),
        Err(ServerUrlError::Invalid(_))
    ));
    for error in [
        ServerUrlError::Empty,
        ServerUrlError::Invalid("x".into()),
        ServerUrlError::UnsupportedScheme("ftp".into()),
        ServerUrlError::HasCredentials,
    ] {
        assert_ne!(error.to_string(), "");
    }
}

#[test]
fn server_url_displays_as_its_normal_form() {
    let server: ServerUrl = "jellyfin.lan:8096".parse().unwrap();
    assert_eq!(server.to_string(), "http://jellyfin.lan:8096/");
}

#[test]
fn device_id_round_trips_through_a_string() {
    let id = DeviceId::generate();
    let restored: DeviceId = id.as_str().parse().unwrap();
    assert_eq!(restored, id);
    assert_eq!(restored.to_string(), id.as_str());
}

#[test]
fn generated_device_ids_are_unique() {
    assert_ne!(DeviceId::generate(), DeviceId::generate());
}

#[test]
fn ids_reject_characters_that_could_break_the_header() {
    for bad in [
        "",
        "a\"b",
        "a,b",
        "a b",
        "ü",
        "a=b",
        "a%41",
        &"x".repeat(257),
    ] {
        assert!(bad.parse::<DeviceId>().is_err(), "{bad:?}");
        assert!(AccessToken::new(bad).is_err(), "{bad:?}");
        assert!(bad.parse::<UserId>().is_err(), "{bad:?}");
        assert!(bad.parse::<ServerId>().is_err(), "{bad:?}");
    }
}

#[test]
fn invalid_value_messages_name_the_value_without_a_wrong_article() {
    let rule = "must be 1 to 256 characters of ASCII letters, digits, '.', '_', '~' or '-'";
    let cases = [
        ("Device ID", "a b".parse::<DeviceId>().unwrap_err()),
        ("Access token", AccessToken::new("a b").unwrap_err()),
        ("User ID", "a b".parse::<UserId>().unwrap_err()),
        ("Server ID", "a b".parse::<ServerId>().unwrap_err()),
    ];
    for (what, error) in cases {
        assert_eq!(error.to_string(), format!("{what} {rule}"));
    }
}

#[test]
fn ids_accept_uuids_and_jellyfin_hex() {
    for good in [
        "3f2b9c1e-8d4a-4b6f-9e2d-7a1c5b8e0f42",
        "38a5a5bbdc3549ccb0b07fc7e43c1d43",
        "a.b_c~d",
    ] {
        assert_eq!(good.parse::<DeviceId>().unwrap().as_str(), good);
        assert_eq!(AccessToken::new(good).unwrap().expose_secret(), good);
        assert_eq!(good.parse::<UserId>().unwrap().as_str(), good);
        assert_eq!(good.parse::<ServerId>().unwrap().to_string(), good);
    }
}

#[test]
fn access_token_debug_is_redacted() {
    let token = AccessToken::new("0123456789abcdef").unwrap();
    let debug = format!("{token:?}");
    assert!(!debug.contains("0123456789abcdef"), "{debug}");
    assert!(debug.contains("redacted"), "{debug}");
}

#[test]
fn password_debug_is_redacted() {
    let password = Password::new("hunter2");
    let debug = format!("{password:?}");
    assert!(!debug.contains("hunter2"), "{debug}");
    assert_eq!(password.expose_secret(), "hunter2");
}

#[test]
fn session_debug_hides_the_token_but_shows_the_server() {
    let session = Session::new(
        "jellyfin.lan:8096".parse().unwrap(),
        AccessToken::new("0123456789abcdef").unwrap(),
    );
    let debug = format!("{session:?}");
    assert!(!debug.contains("0123456789abcdef"), "{debug}");
    assert!(debug.contains("jellyfin.lan"), "{debug}");
    assert_eq!(session.server().as_str(), "http://jellyfin.lan:8096/");
    assert_eq!(session.token().expose_secret(), "0123456789abcdef");
}

#[test]
fn item_ids_and_image_tags_cannot_escape_a_url_path() {
    use couchfin_api::{ImageTag, ItemId};

    for bad in ["", "a/b", "..%2F", "a b", "a?x=1", "a#b"] {
        assert!(bad.parse::<ItemId>().is_err(), "{bad:?}");
        assert!(bad.parse::<ImageTag>().is_err(), "{bad:?}");
    }
    let id: ItemId = "a1b2c3d4-e5f6".parse().unwrap();
    assert_eq!(id.to_string(), "a1b2c3d4-e5f6");
    assert!(serde_json::from_str::<ItemId>("\"../x\"").is_err());
}
