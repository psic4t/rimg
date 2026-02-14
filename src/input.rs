use xkbcommon_dl::keysyms;

use crate::wayland::KeyEvent;

// Evdev keycodes (layout-independent)
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

    // Global actions
    CycleSort,
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
        keysyms::s => return Some(Action::CycleSort),
        _ => {}
    }

    match mode {
        Mode::Viewer => map_viewer_key(event.keycode, sym, event.ctrl, event.shift),
        Mode::Gallery => map_gallery_key(sym),
    }
}

fn map_viewer_key(keycode: u32, sym: u32, ctrl: bool, shift: bool) -> Option<Action> {
    if ctrl && keycode == KEY_0 {
        return Some(Action::ActualSize);
    }

    if shift && keycode == KEY_W {
        return Some(Action::FitToWindow);
    }

    // h/j/k/l and arrow keys pan directly (no Ctrl required).
    match sym {
        keysyms::h | keysyms::Left => Some(Action::PanStart(PanDirection::Left)),
        keysyms::l | keysyms::Right => Some(Action::PanStart(PanDirection::Right)),
        keysyms::k | keysyms::Up => Some(Action::PanStart(PanDirection::Up)),
        keysyms::j | keysyms::Down => Some(Action::PanStart(PanDirection::Down)),
        keysyms::n => Some(Action::NextImage),
        keysyms::p => Some(Action::PrevImage),
        keysyms::g => Some(Action::FirstImage),
        keysyms::G => Some(Action::LastImage),
        keysyms::plus | keysyms::equal => Some(Action::ZoomIn),
        keysyms::minus => Some(Action::ZoomOut),
        keysyms::_0 => Some(Action::ZoomReset),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wayland::KeyEvent;

    fn press(keysym: u32) -> KeyEvent {
        KeyEvent {
            keycode: 0,
            keysym,
            pressed: true,
            ctrl: false,
            shift: false,
        }
    }

    fn release(keysym: u32) -> KeyEvent {
        KeyEvent {
            keycode: 0,
            keysym,
            pressed: false,
            ctrl: false,
            shift: false,
        }
    }

    #[test]
    fn test_quit_viewer() {
        let action = map_key(&press(keysyms::q), Mode::Viewer);
        assert_eq!(action, Some(Action::Quit));
    }

    #[test]
    fn test_quit_gallery() {
        let action = map_key(&press(keysyms::q), Mode::Gallery);
        assert_eq!(action, Some(Action::Quit));
    }

    #[test]
    fn test_escape() {
        let action = map_key(&press(keysyms::Escape), Mode::Viewer);
        assert_eq!(action, Some(Action::EscapeOrQuit));
    }

    #[test]
    fn test_toggle_mode() {
        let action = map_key(&press(keysyms::Return), Mode::Viewer);
        assert_eq!(action, Some(Action::ToggleMode));
    }

    #[test]
    fn test_cycle_sort() {
        let action = map_key(&press(keysyms::s), Mode::Viewer);
        assert_eq!(action, Some(Action::CycleSort));
        let action = map_key(&press(keysyms::s), Mode::Gallery);
        assert_eq!(action, Some(Action::CycleSort));
    }

    #[test]
    fn test_viewer_next_image() {
        let action = map_key(&press(keysyms::n), Mode::Viewer);
        assert_eq!(action, Some(Action::NextImage));
    }

    #[test]
    fn test_viewer_pan() {
        let action = map_key(&press(keysyms::h), Mode::Viewer);
        assert_eq!(action, Some(Action::PanStart(PanDirection::Left)));
        let action = map_key(&press(keysyms::j), Mode::Viewer);
        assert_eq!(action, Some(Action::PanStart(PanDirection::Down)));
    }

    #[test]
    fn test_gallery_move_down() {
        let action = map_key(&press(keysyms::j), Mode::Gallery);
        assert_eq!(action, Some(Action::MoveDown));
    }

    #[test]
    fn test_gallery_move_left() {
        let action = map_key(&press(keysyms::h), Mode::Gallery);
        assert_eq!(action, Some(Action::MoveLeft));
    }

    #[test]
    fn test_gallery_first_last() {
        let action = map_key(&press(keysyms::g), Mode::Gallery);
        assert_eq!(action, Some(Action::GalleryFirst));
        let action = map_key(&press(keysyms::G), Mode::Gallery);
        assert_eq!(action, Some(Action::GalleryLast));
    }

    #[test]
    fn test_viewer_zoom() {
        let action = map_key(&press(keysyms::plus), Mode::Viewer);
        assert_eq!(action, Some(Action::ZoomIn));
        let action = map_key(&press(keysyms::minus), Mode::Viewer);
        assert_eq!(action, Some(Action::ZoomOut));
    }

    #[test]
    fn test_viewer_rotate() {
        let action = map_key(&press(keysyms::r), Mode::Viewer);
        assert_eq!(action, Some(Action::RotateCW));
        let action = map_key(&press(keysyms::R), Mode::Viewer);
        assert_eq!(action, Some(Action::RotateCCW));
    }

    #[test]
    fn test_unmapped_key() {
        let action = map_key(&press(keysyms::z), Mode::Viewer);
        assert_eq!(action, None);
    }

    #[test]
    fn test_release_ignored_gallery() {
        let action = map_key(&release(keysyms::j), Mode::Gallery);
        assert_eq!(action, None);
    }

    #[test]
    fn test_viewer_key_release_pan_stop() {
        let ev = KeyEvent {
            keycode: KEY_H,
            keysym: keysyms::h,
            pressed: false,
            ctrl: false,
            shift: false,
        };
        let action = map_key(&ev, Mode::Viewer);
        assert_eq!(action, Some(Action::PanStop(PanDirection::Left)));
    }
}
