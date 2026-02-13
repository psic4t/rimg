use xkbcommon_dl::keysyms;

use crate::wayland::KeyEvent;

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
    PanLeft,
    PanRight,
    PanUp,
    PanDown,
    Fullscreen,
    RotateCW,
    RotateCCW,

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
    if !event.pressed {
        return None;
    }

    let sym = event.keysym;

    // Global keys
    match sym {
        keysyms::q => return Some(Action::Quit),
        keysyms::Escape => return Some(Action::EscapeOrQuit),
        keysyms::Return => return Some(Action::ToggleMode),
        _ => {}
    }

    match mode {
        Mode::Viewer => map_viewer_key(sym, event.ctrl),
        Mode::Gallery => map_gallery_key(sym),
    }
}

fn map_viewer_key(sym: u32, ctrl: bool) -> Option<Action> {
    match sym {
        keysyms::n => Some(Action::NextImage),
        keysyms::p => Some(Action::PrevImage),
        keysyms::g => Some(Action::FirstImage),
        keysyms::G => Some(Action::LastImage),
        keysyms::plus | keysyms::equal => Some(Action::ZoomIn),
        keysyms::minus => Some(Action::ZoomOut),
        keysyms::_0 => Some(Action::ZoomReset),
        keysyms::h | keysyms::Left => {
            if ctrl {
                Some(Action::PanLeft)
            } else {
                Some(Action::MoveLeft)
            }
        }
        keysyms::l | keysyms::Right => {
            if ctrl {
                Some(Action::PanRight)
            } else {
                Some(Action::MoveRight)
            }
        }
        keysyms::k | keysyms::Up => {
            if ctrl {
                Some(Action::PanUp)
            } else {
                Some(Action::MoveUp)
            }
        }
        keysyms::j | keysyms::Down => {
            if ctrl {
                Some(Action::PanDown)
            } else {
                Some(Action::MoveDown)
            }
        }
        keysyms::f => Some(Action::Fullscreen),
        keysyms::r => Some(Action::RotateCW),
        keysyms::R => Some(Action::RotateCCW),
        keysyms::space => Some(Action::NextImage),
        keysyms::BackSpace => Some(Action::PrevImage),
        _ => None,
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
