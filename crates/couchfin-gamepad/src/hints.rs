//! SDL hints: settings SDL reads at start-up, such as which drivers to use.

/// A hint Couchfin won't set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HintError {
    /// Steam sets this hint, as an environment variable, to hide the
    /// physical controller behind its virtual one. Setting it here could
    /// make every press arrive twice.
    #[error("{0} is Steam's to set")]
    SteamOwned(String),
}

/// The hints Steam passes to games, which SDL must read untouched.
const STEAM_OWNED: &[&str] = &[
    "SDL_GAMECONTROLLER_IGNORE_DEVICES",
    "SDL_GAMECONTROLLER_IGNORE_DEVICES_EXCEPT",
    "SDL_GAMECONTROLLERCONFIG",
    "SDL_GAMECONTROLLERCONFIG_FILE",
];

/// SDL hints to set before SDL starts, as name and value.
///
/// They are defaults: SDL lets an environment variable of the same name
/// win, so `SDL_JOYSTICK_HIDAPI_STEAMDECK=0 couchfin` overrides one without
/// a rebuild. The hints Steam uses to hide devices can't be set here at
/// all (see [`HintError::SteamOwned`]).
///
/// The default has one hint, `SDL_NO_SIGNAL_HANDLERS=1`: without it SDL
/// would turn Ctrl+C and SIGTERM into a quit event that nothing reads, so
/// the app couldn't be stopped that way.
///
/// ```
/// use couchfin_gamepad::SdlHints;
///
/// let hints = SdlHints::default().with("SDL_JOYSTICK_HIDAPI_STEAMDECK", "0")?;
/// assert!(hints.iter().any(|(name, _)| name == "SDL_JOYSTICK_HIDAPI_STEAMDECK"));
/// assert!(hints.with("SDL_GAMECONTROLLER_IGNORE_DEVICES", "").is_err());
/// # Ok::<(), couchfin_gamepad::HintError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdlHints {
    hints: Vec<(String, String)>,
}

impl SdlHints {
    /// No hints at all, not even the default.
    #[must_use]
    pub fn none() -> Self {
        Self { hints: Vec::new() }
    }

    /// These hints plus `name = value`, replacing any earlier value.
    ///
    /// # Errors
    ///
    /// [`HintError::SteamOwned`] for a hint Steam sets.
    pub fn with(mut self, name: &str, value: &str) -> Result<Self, HintError> {
        if STEAM_OWNED.contains(&name) {
            return Err(HintError::SteamOwned(name.to_owned()));
        }
        self.hints.retain(|(existing, _)| existing != name);
        self.hints.push((name.to_owned(), value.to_owned()));
        self.hints.sort();
        Ok(self)
    }

    /// Each hint as name and value, sorted by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.hints
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }
}

impl Default for SdlHints {
    fn default() -> Self {
        Self {
            hints: vec![("SDL_NO_SIGNAL_HANDLERS".to_owned(), "1".to_owned())],
        }
    }
}
