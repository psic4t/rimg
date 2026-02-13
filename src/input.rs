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
        Mode::Viewer => map_viewer_key(sym),
        Mode::Gallery => map_gallery_key(sym),
    }
}

fn map_viewer_key(sym: u32) -> Option<Action> {
    match sym {
        keysyms::n => Some(Action::NextImage),
        keysyms::p => Some(Action::PrevImage),
        keysyms::g => Some(Action::FirstImage),
        keysyms::G => Some(Action::LastImage),
        keysyms::plus | keysyms::equal => Some(Action::ZoomIn),
        keysyms::minus => Some(Action::ZoomOut),
        keysyms::_0 => Some(Action::ZoomReset),
        keysyms::h => Some(Action::PanLeft),
        keysyms::l => Some(Action::PanRight),
        keysyms::k => Some(Action::PanUp),
        keysyms::j => Some(Action::PanDown),
        keysyms::space => Some(Action::NextImage),
        keysyms::BackSpace => Some(Action::PrevImage),
        _ => None,
    }
}

fn map_gallery_key(sym: u32) -> Option<Action> {
    match sym {
        keysyms::h => Some(Action::MoveLeft),
        keysyms::l => Some(Action::MoveRight),
        keysyms::k => Some(Action::MoveUp),
        keysyms::j => Some(Action::MoveDown),
        keysyms::g => Some(Action::GalleryFirst),
        keysyms::G => Some(Action::GalleryLast),
        _ => None,
    }
}
