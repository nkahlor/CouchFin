//! What a controller asks for, independent of which pad or button it came from.

/// One request from a controller. [`Mapper`](crate::Mapper) turns pad events
/// into these.
///
/// Directions repeat while held; the others fire once per press. Playback
/// controls (seek, play/pause) will join this enum when the player needs
/// them. It is deliberately exhaustive, so adding one is a compile error
/// wherever inputs are handled rather than a silently ignored case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Input {
    /// D-pad up or the left stick pushed up.
    Up,
    /// D-pad down or the left stick pushed down.
    Down,
    /// D-pad left or the left stick pushed left.
    Left,
    /// D-pad right or the left stick pushed right.
    Right,
    /// The south face button (A on Xbox and Steam pads, Cross on Sony pads).
    Select,
    /// The east face button (B on Xbox and Steam pads, Circle on Sony pads).
    Back,
    /// Start (Xbox Menu, Sony Options, Nintendo Plus).
    Menu,
}
