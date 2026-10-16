//! Helpers shared by the login-flow tests. Each test binary uses a different
//! subset, so unused ones are allowed.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use couchfin_core::fake::{FakeJellyfin, MemorySettings, MemoryTokens};
use couchfin_core::{
    AccessToken, DeviceId, LoginFlow, LoginScreen, ServerUrl, Settings, SettingsStore, TokenStore,
};

pub const DEVICE_ID: &str = "3f2b9c1e-8d4a-4b6f-9e2d-7a1c5b8e0f42";

pub type FakeFlow = LoginFlow<FakeJellyfin, MemorySettings, MemoryTokens>;

pub fn url(text: &str) -> ServerUrl {
    text.parse().expect("test URL is valid")
}

pub fn token(text: &str) -> AccessToken {
    AccessToken::new(text).expect("test token is valid")
}

pub fn device_id() -> DeviceId {
    DEVICE_ID.parse().expect("DEVICE_ID is valid")
}

/// A flow over a fake server, with handles to everything it owns and a log
/// of every screen it published.
pub struct Harness {
    pub flow: Arc<FakeFlow>,
    pub fake: FakeJellyfin,
    pub settings: MemorySettings,
    pub tokens: MemoryTokens,
    pub seen: Arc<Mutex<Vec<LoginScreen>>>,
}

impl Harness {
    /// A flow whose stored settings are `settings`, as after a restart.
    pub fn new(fake: FakeJellyfin, settings: &Settings, tokens: MemoryTokens) -> Self {
        let store = MemorySettings::new();
        store.save(settings).expect("memory store saves");
        Self::with_stores(fake, store, tokens)
    }

    pub fn with_stores(fake: FakeJellyfin, settings: MemorySettings, tokens: MemoryTokens) -> Self {
        let loaded = Settings::load_or_create(&settings);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let flow = LoginFlow::new(fake.clone(), loaded, settings.clone(), tokens.clone())
            .on_change(move |screen| {
                log.lock()
                    .expect("lock is not poisoned")
                    .push(screen.clone());
            });
        Self {
            flow: Arc::new(flow),
            fake,
            settings,
            tokens,
            seen,
        }
    }

    /// A first run: nothing stored yet.
    pub fn first_run(fake: FakeJellyfin) -> Self {
        Self::with_stores(fake, MemorySettings::new(), MemoryTokens::new())
    }

    pub fn screen(&self) -> LoginScreen {
        self.flow.screen()
    }

    pub fn seen(&self) -> Vec<LoginScreen> {
        self.seen.lock().expect("lock is not poisoned").clone()
    }

    pub fn stored(&self) -> Settings {
        self.settings
            .load()
            .expect("memory store loads")
            .expect("settings were saved")
    }

    pub fn stored_token(&self, server: &ServerUrl) -> Option<AccessToken> {
        self.tokens.load(server).expect("memory store loads")
    }
}

/// Settings with a server and username, as left by an earlier login.
pub fn saved(server: &ServerUrl, username: &str) -> Settings {
    let mut settings = Settings::new(device_id());
    settings.server = Some(server.clone());
    settings.username = Some(username.to_owned());
    settings
}

/// A token store holding `token` for `server`.
pub fn tokens_with(server: &ServerUrl, token: &AccessToken) -> MemoryTokens {
    let tokens = MemoryTokens::new();
    tokens.save(server, token).expect("memory store saves");
    tokens
}
