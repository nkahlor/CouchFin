//! Controller events in SDL's gamepad terms, without depending on SDL, so
//! the mapping can be fed by a fake.

/// One connected controller, as SDL numbers it (its joystick instance ID).
///
/// IDs are never reused while the program runs, so a pad that is unplugged
/// and plugged back in gets a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PadId(pub u32);

/// A gamepad button by its position on the pad, as SDL reports it. SDL
/// applies the controller's mapping, so the same button means the same
/// position on every supported pad, whatever its label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    /// The bottom face button: A on Xbox, Cross on Sony pads, B on Nintendo.
    South,
    /// The right face button: B on Xbox, Circle on Sony pads, A on Nintendo.
    East,
    /// The left face button: X on Xbox, Square on Sony pads, Y on Nintendo.
    West,
    /// The top face button: Y on Xbox, Triangle on Sony pads, X on Nintendo.
    North,
    /// SDL's "back" button: Xbox View, Sony Share or Create, Nintendo Minus.
    View,
    /// The logo button. Steam and the desktop usually claim it.
    Guide,
    /// Xbox Menu, Sony Options, Nintendo Plus.
    Start,
    /// Pressing the left stick in.
    LeftStick,
    /// Pressing the right stick in.
    RightStick,
    /// The left bumper.
    LeftShoulder,
    /// The right bumper.
    RightShoulder,
    /// D-pad up.
    DPadUp,
    /// D-pad down.
    DPadDown,
    /// D-pad left.
    DPadLeft,
    /// D-pad right.
    DPadRight,
    /// Anything else: paddles, touchpad clicks, capture and other extra buttons.
    Other,
}

/// A gamepad axis. Sticks run from -32768 to 32767, negative being left or
/// up; triggers from 0 to 32767.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Left stick, horizontal.
    LeftX,
    /// Left stick, vertical (negative is up).
    LeftY,
    /// Right stick, horizontal.
    RightX,
    /// Right stick, vertical (negative is up).
    RightY,
    /// Left trigger.
    LeftTrigger,
    /// Right trigger.
    RightTrigger,
}

/// Whether a button went down or came up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonState {
    /// The button was pressed.
    Down,
    /// The button was released.
    Up,
}

/// Something that happened on a controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PadEvent {
    /// A controller was connected (or was already connected at start-up).
    Added(PadId),
    /// A controller was disconnected. Anything it was holding is released.
    Removed(PadId),
    /// A button was pressed or released.
    Button {
        /// Which controller.
        pad: PadId,
        /// Which button.
        button: Button,
        /// Pressed or released.
        state: ButtonState,
    },
    /// An axis moved.
    Axis {
        /// Which controller.
        pad: PadId,
        /// Which axis.
        axis: Axis,
        /// Its new position.
        value: i16,
    },
}
