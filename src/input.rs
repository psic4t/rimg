use xkbcommon_dl::keysyms;

use crate::wayland::KeyEvent;

// Evdev keycodes for vim pan keys (layout-independent, unaffected by Ctrl)
const KEY_H: u32 = 35;
const KEY_J: u32 = 36;
const KEY_K: u32 = 37;
const KEY_L: u32 = 38;
const KEY_W: u32 = 17;
const KEY_0: u32 = 11;

/// Pan direction indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanDirection {
    Left = 0,
    Right = 1,
    Up = 2,
    Down = 3,
}

/// Actions the application can perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    ToggleMode,
    EscapeOrQuit,

    // Viewer actions
    NextImage,
    PrevImage,
    FirstImage,
    LastImage,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    PanStart(PanDirection),
    PanStop(PanDirection),
    Fullscreen,
    RotateCW,
    RotateCCW,
    ToggleExif,
    FitToWindow,
    ActualSize,

    // Gallery actions
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    GalleryFirst,
    GalleryLast,
}

/// Application mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Viewer,
    Gallery,
}

/// Map a key event to an action based on the current mode.
/// Returns None for unmapped keys.
pub fn map_key(event: &KeyEvent, mode: Mode) -> Option<Action> {
    // Handle key releases: only pan stop events matter
    if !event.pressed {
        return match mode {
            Mode::Viewer => map_viewer_key_release(event.keycode, event.keysym),
            Mode::Gallery => None,
        };
    }

    let sym = event.keysym;

    // Global keys (press only)
    match sym {
        keysyms::q => return Some(Action::Quit),
        keysyms::Escape => return Some(Action::EscapeOrQuit),
        keysyms::Return => return Some(Action::ToggleMode),
        _ => {}
    }

    match mode {
        Mode::Viewer => map_viewer_key(event.keycode, sym, event.ctrl),
        Mode::Gallery => map_gallery_key(sym),
    }
}

fn map_viewer_key(keycode: u32, sym: u32, ctrl: bool) -> Option<Action> {
    // When Ctrl is held, map hjkl/arrows to pan.
    // For h/j/k/l we use keycode because xkb transforms them into control characters.
    // For arrows we use keysym (stable with Ctrl held).
    if ctrl {
        let pan = match keycode {
            KEY_H => Some(PanDirection::Left),
            KEY_J => Some(PanDirection::Down),
            KEY_K => Some(PanDirection::Up),
            KEY_L => Some(PanDirection::Right),
            _ => match sym {
                keysyms::Left => Some(PanDirection::Left),
                keysyms::Right => Some(PanDirection::Right),
                keysyms::Up => Some(PanDirection::Up),
                keysyms::Down => Some(PanDirection::Down),
                _ => None,
            },
        };
        if let Some(dir) = pan {
            return Some(Action::PanStart(dir));
        }
        if keycode == KEY_W {
            return Some(Action::FitToWindow);
        }
        if keycode == KEY_0 {
            return Some(Action::ActualSize);
        }
    }

    match sym {
        keysyms::n => Some(Action::NextImage),
        keysyms::p => Some(Action::PrevImage),
        keysyms::g => Some(Action::FirstImage),
        keysyms::G => Some(Action::LastImage),
        keysyms::plus | keysyms::equal => Some(Action::ZoomIn),
        keysyms::minus => Some(Action::ZoomOut),
        keysyms::_0 => Some(Action::ZoomReset),
        keysyms::h | keysyms::Left => Some(Action::MoveLeft),
        keysyms::l | keysyms::Right => Some(Action::MoveRight),
        keysyms::k | keysyms::Up => Some(Action::MoveUp),
        keysyms::j | keysyms::Down => Some(Action::MoveDown),
        keysyms::e => Some(Action::ToggleExif),
        keysyms::f => Some(Action::Fullscreen),
        keysyms::r => Some(Action::RotateCW),
        keysyms::R => Some(Action::RotateCCW),
        keysyms::space => Some(Action::NextImage),
        keysyms::BackSpace => Some(Action::PrevImage),
        _ => None,
    }
}

/// Map key releases in viewer mode — only pan stop events.
/// Uses keycode for h/j/k/l (since keysym changes with Ctrl held)
/// and keysym for arrow keys (stable regardless of modifiers).
fn map_viewer_key_release(keycode: u32, sym: u32) -> Option<Action> {
    match keycode {
        KEY_H => Some(Action::PanStop(PanDirection::Left)),
        KEY_J => Some(Action::PanStop(PanDirection::Down)),
        KEY_K => Some(Action::PanStop(PanDirection::Up)),
        KEY_L => Some(Action::PanStop(PanDirection::Right)),
        _ => match sym {
            keysyms::Left => Some(Action::PanStop(PanDirection::Left)),
            keysyms::Right => Some(Action::PanStop(PanDirection::Right)),
            keysyms::Up => Some(Action::PanStop(PanDirection::Up)),
            keysyms::Down => Some(Action::PanStop(PanDirection::Down)),
            _ => None,
        },
    }
}

fn map_gallery_key(sym: u32) -> Option<Action> {
    match sym {
        keysyms::h | keysyms::Left => Some(Action::MoveLeft),
        keysyms::l | keysyms::Right => Some(Action::MoveRight),
        keysyms::k | keysyms::Up => Some(Action::MoveUp),
        keysyms::j | keysyms::Down => Some(Action::MoveDown),
        keysyms::g => Some(Action::GalleryFirst),
        keysyms::G => Some(Action::GalleryLast),
        _ => None,
    }
}
