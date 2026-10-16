//! Tuning for the mapping: repeat timing and the stick deadzone.

use std::time::Duration;

/// A setting that can't work.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// A zero delay or interval would repeat without time passing.
    #[error("the repeat delay and interval must both be longer than zero")]
    ZeroRepeat,
    /// The press threshold must be above zero and the release threshold
    /// at or below it, or the stick would never settle.
    #[error(
        "the stick thresholds need 0 < release <= press (got press {press}, release {release})"
    )]
    Thresholds {
        /// The requested press threshold.
        press: u16,
        /// The requested release threshold.
        release: u16,
    },
}

/// How a held direction repeats: once at the press, again after `delay`,
/// then every `interval` until it is released.
///
/// The default, 400 ms then every 60 ms, is close to a desktop keyboard's
/// repeat, so a held D-pad feels like a held arrow key.
///
/// ```
/// use std::time::Duration;
/// use couchfin_gamepad::Repeat;
///
/// let repeat = Repeat::new(Duration::from_millis(300), Duration::from_millis(50))?;
/// assert_eq!(repeat.delay(), Duration::from_millis(300));
/// assert!(Repeat::new(Duration::from_millis(300), Duration::ZERO).is_err());
/// assert!(Repeat::new(Duration::ZERO, Duration::from_millis(50)).is_err());
/// # Ok::<(), couchfin_gamepad::ConfigError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repeat {
    delay: Duration,
    interval: Duration,
}

impl Repeat {
    /// A repeat that waits `delay` after the press, then fires every
    /// `interval`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::ZeroRepeat`] if either is zero.
    pub fn new(delay: Duration, interval: Duration) -> Result<Self, ConfigError> {
        if delay.is_zero() || interval.is_zero() {
            return Err(ConfigError::ZeroRepeat);
        }
        Ok(Self { delay, interval })
    }

    /// The wait between the press and the first repeat.
    #[must_use]
    pub fn delay(&self) -> Duration {
        self.delay
    }

    /// The wait between later repeats.
    #[must_use]
    pub fn interval(&self) -> Duration {
        self.interval
    }
}

impl Default for Repeat {
    fn default() -> Self {
        Self {
            delay: Duration::from_millis(400),
            interval: Duration::from_millis(60),
        }
    }
}

/// When the left stick counts as a direction, in SDL axis units (a stick
/// reads up to 32767 either way).
///
/// The stick engages once its stronger axis reaches `press`, and lets go
/// only when that axis falls below `release`. The gap between the two
/// (hysteresis) stops a stick resting near one threshold from flickering
/// on and off. The default engages at half travel (16384) and releases
/// at about a third (10923).
///
/// ```
/// use couchfin_gamepad::Deadzone;
///
/// let deadzone = Deadzone::new(20_000, 12_000)?;
/// assert_eq!((deadzone.press(), deadzone.release()), (20_000, 12_000));
/// assert!(Deadzone::new(12_000, 20_000).is_err());
/// # Ok::<(), couchfin_gamepad::ConfigError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadzone {
    press: u16,
    release: u16,
}

impl Deadzone {
    /// A deadzone that engages at `press` and releases below `release`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::Thresholds`] unless `0 < release <= press`.
    pub fn new(press: u16, release: u16) -> Result<Self, ConfigError> {
        if release == 0 || release > press {
            return Err(ConfigError::Thresholds { press, release });
        }
        Ok(Self { press, release })
    }

    /// How far the stick must move to engage.
    #[must_use]
    pub fn press(&self) -> u16 {
        self.press
    }

    /// How far back it must come to release.
    #[must_use]
    pub fn release(&self) -> u16 {
        self.release
    }
}

impl Default for Deadzone {
    fn default() -> Self {
        Self {
            press: 16_384,
            release: 10_923,
        }
    }
}

/// Everything [`Mapper`](crate::Mapper) can be tuned with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MapperConfig {
    /// How held directions repeat.
    pub repeat: Repeat,
    /// When the left stick counts as a direction.
    pub deadzone: Deadzone,
}
