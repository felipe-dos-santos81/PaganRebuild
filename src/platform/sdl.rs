//! The window opened by SDL3: the backend of platform.rs when the program
//! is built with the feature "sdl" (as the Makefile does on a Mac). SDL
//! opens the window and reports the keyboard and the mouse; what the events
//! mean for the game is decided in platform.rs (`Input`). The pictures
//! still go to the screen through wgpu, the gamepads through gilrs.
//!
//! The mouse pointer is never held in the window: nothing here grabs it or
//! asks for relative mouse movement.

use super::{Input, Key};
use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::{Keycode, Mod, Scancode};
use sdl3::mouse::MouseButton;
use sdl3::video::FullscreenType;
// winit stays a dependency of every build, so that the build without SDL
// is the one it always was; here it is named only so that it is not
// reported as unused.
// ponytail: winit is still compiled into this build; make it an optional
// dependency if the build time ever matters.
use winit as _;

type KeyMap = crate::hash::HashMap<Key, Option<Key>>;

/// The key a key event is about: keys with a fixed place by their place
/// (scancode), letters, digits and signs by what the key says (keycode), so
/// that Z is the key labelled Z, also on a German keyboard.
fn key_of(scancode: Option<Scancode>, keycode: Option<Keycode>) -> Option<Key> {
    use Key::*;
    use Scancode as S;
    let by_place = scancode.and_then(|code| {
        Some(match code {
            S::F1 => F1, S::F2 => F2, S::F3 => F3, S::F4 => F4, S::F5 => F5, S::F6 => F6,
            S::F7 => F7, S::F8 => F8, S::F9 => F9, S::F10 => F10, S::F11 => F11, S::F12 => F12,
            S::Up => Up, S::Down => Down, S::Left => Left, S::Right => Right,
            S::PageUp => PageUp, S::PageDown => PageDown, S::Home => Home, S::End => End,
            S::Insert => Insert, S::Delete => Delete,
            S::Escape => Escape, S::Return => Enter, S::Space => Space, S::Tab => Tab, S::Backspace => Backspace,
            S::LShift => LeftShift, S::RShift => RightShift, S::LCtrl => LeftCtrl, S::RCtrl => RightCtrl,
            S::LAlt => LeftAlt, S::RAlt => RightAlt,
            S::Kp0 => NumPad0, S::Kp1 => NumPad1, S::Kp2 => NumPad2, S::Kp3 => NumPad3, S::Kp4 => NumPad4,
            S::Kp5 => NumPad5, S::Kp6 => NumPad6, S::Kp7 => NumPad7, S::Kp8 => NumPad8, S::Kp9 => NumPad9,
            S::KpMinus => NumPadMinus, S::KpPlus => NumPadPlus, S::KpEnter => NumPadEnter,
            _ => return None,
        })
    });
    if by_place.is_some() {
        return by_place;
    }
    // letters, digits, signs: what the key says without shift (SDL's
    // keycode of such a key is its character)
    let c = char::from_u32(keycode? as u32)?.to_ascii_lowercase();
    const LETTERS: [Key; 26] = [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z];
    const DIGITS: [Key; 10] = [Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9];
    match c {
        'a'..='z' => Some(LETTERS[(c as u8 - b'a') as usize]),
        '0'..='9' => Some(DIGITS[(c as u8 - b'0') as usize]),
        ',' => Some(Comma),
        '.' => Some(Period),
        '-' => Some(Minus),
        // the key right of 0/ß: "=" on English keyboards, "+" on German ones
        '=' | '+' => Some(Equal),
        '\'' => Some(Apostrophe),
        _ => None,
    }
}

/// How many screen pixels one of SDL's points has on a screen with this
/// pixel density (2 on a Retina display); a density SDL could not tell
/// counts as 1.
fn scale(density: f32) -> f64 {
    if density > 0.0 {
        density as f64
    } else {
        1.0
    }
}

/// Screen pixels to SDL's points: SDL counts window sizes and the mouse's
/// place in points, the game in screen pixels.
fn points(pixels: f64, density: f32) -> f64 {
    pixels / scale(density)
}

/// SDL's points to screen pixels.
fn pixels(points: f64, density: f32) -> f64 {
    points * scale(density)
}

/// A key pressed or let go. True: Alt+Enter, the full screen is to be
/// switched.
fn key(input: &mut Input, key_map: &KeyMap, scancode: Option<Scancode>, keycode: Option<Keycode>, keymod: Mod, pressed: bool, repeat: bool) -> bool {
    input.alt = keymod.intersects(Mod::LALTMOD | Mod::RALTMOD);
    key_of(scancode, keycode).is_some_and(|raw| input.key(raw, pressed, repeat, key_map))
}

/// What SDL is told before it starts.
fn set_hints() {
    // A held key repeats. Left alone, macOS opens its menu of accented
    // letters over the game when A or S is held, and the Avatar walks with
    // held keys.
    sdl3::hint::set("SDL_MAC_PRESS_AND_HOLD", "0");
}

/// The window and its events.
pub struct Backend {
    /// What the window's events left behind since the last look.
    pub input: Input,
    /// Keys that do what another key does (the player's own key bindings);
    /// None: the key does nothing.
    pub key_map: KeyMap,
    window: sdl3::video::Window,
    events: sdl3::EventPump,
    sdl: sdl3::Sdl,
    /// The window was brought to the front after its first events (see `pump`).
    raised: bool,
}

impl Backend {
    /// Opens the window with a picture area of `size` screen pixels.
    pub fn new(title: &str, size: (u32, u32), fullscreen: bool) -> Result<Backend, String> {
        // only SDL's video part: the gamepads stay with gilrs, the sound with rodio
        set_hints();
        let sdl = sdl3::init().map_err(|e| format!("no SDL: {e}"))?;
        let video = sdl.video().map_err(|e| format!("no SDL video: {e}"))?;
        let mut builder = video.window(title, size.0, size.1);
        builder.resizable().high_pixel_density().metal_view();
        if fullscreen {
            builder.fullscreen();
        } else {
            // shown once it has its size (see below)
            builder.hidden();
        }
        let mut window = builder.build().map_err(|e| format!("no window: {e}"))?;
        // SDL took the size as points; the game asked for screen pixels
        let density = window.pixel_density();
        let in_points = |px: u32| (points(px as f64, density).round() as u32).max(1);
        if !fullscreen {
            window.set_size(in_points(size.0), in_points(size.1)).map_err(|e| format!("no window: {e}"))?;
        }
        let _ = window.set_minimum_size(in_points(320), in_points(200));
        window.show();
        // the typed text comes as its own events (with the keyboard's layout, dead keys ...)
        video.text_input().start(&window);
        let events = sdl.event_pump().map_err(|e| format!("no events: {e}"))?;
        Ok(Backend { input: Input::default(), key_map: Default::default(), window, events, sdl, raised: false })
    }

    /// The surface the pictures are drawn to.
    pub fn surface(&self, instance: &wgpu::Instance) -> Result<wgpu::Surface<'static>, String> {
        // SAFETY: the surface must not outlive the window it draws to.
        // `Window` in platform.rs keeps its `gpu` (with the surface) before
        // its `backend` (with the window), so the surface is dropped first.
        unsafe {
            let target = wgpu::SurfaceTargetUnsafe::from_window(&self.window).map_err(|e| format!("no surface: {e}"))?;
            instance.create_surface_unsafe(target).map_err(|e| format!("no surface: {e}"))
        }
    }

    /// Takes the events that came in into `input`.
    pub fn pump(&mut self) {
        // asked anew every time: it changes when the window moves to another screen
        let density = self.window.pixel_density();
        let input = &mut self.input;
        for event in self.events.poll_iter() {
            match event {
                // Cmd+Q, or the window's close button
                Event::Quit { .. } => input.closed = true,
                Event::Window { win_event, .. } => match win_event {
                    WindowEvent::CloseRequested => input.closed = true,
                    WindowEvent::FocusGained => input.focus(true),
                    WindowEvent::FocusLost => input.focus(false),
                    WindowEvent::MouseEnter => input.inside = true,
                    WindowEvent::MouseLeave => input.inside = false,
                    _ => {}
                },
                Event::KeyDown { keycode, scancode, keymod, repeat, .. } => {
                    if key(input, &self.key_map, scancode, keycode, keymod, true, repeat) {
                        let full = !matches!(self.window.fullscreen_state(), FullscreenType::Off);
                        let _ = self.window.set_fullscreen(!full);
                    }
                }
                Event::KeyUp { keycode, scancode, keymod, .. } => {
                    key(input, &self.key_map, scancode, keycode, keymod, false, false);
                }
                Event::TextInput { text, .. } => input.text(&text),
                Event::MouseMotion { x, y, .. } => {
                    input.mouse = (pixels(x as f64, density), pixels(y as f64, density));
                    input.inside = true;
                }
                Event::MouseButtonDown { mouse_btn, .. } | Event::MouseButtonUp { mouse_btn, .. } => {
                    let i = match mouse_btn {
                        MouseButton::Left => 0,
                        MouseButton::Right => 1,
                        MouseButton::Middle => 2,
                        _ => continue,
                    };
                    input.mouse_button(i, matches!(event, Event::MouseButtonDown { .. }));
                }
                // SDL's y is positive away from the player, as the game expects
                Event::MouseWheel { x, y, .. } => {
                    input.wheel.0 += x;
                    input.wheel.1 += y;
                }
                _ => {}
            }
        }
        // Started from a terminal, the window opens behind the program in
        // front. macOS lets it come forward only once SDL has taken its
        // first events, so it is asked for here and not when the window is made.
        if !std::mem::replace(&mut self.raised, true) {
            self.window.raise();
        }
    }

    /// The picture area in screen pixels.
    pub fn pixel_size(&self) -> (u32, u32) {
        let (w, h) = self.window.size_in_pixels();
        (w.max(1), h.max(1))
    }

    pub fn is_minimized(&self) -> bool {
        self.window.is_minimized()
    }

    /// The screen's refresh rate, if the system tells it.
    pub fn refresh_millihertz(&self) -> Option<u32> {
        let rate = self.window.get_display().ok()?.get_mode().ok()?.refresh_rate;
        (rate > 0.0).then(|| (rate * 1000.0).round() as u32)
    }

    pub fn set_title(&mut self, title: &str) {
        let _ = self.window.set_title(title);
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.sdl.mouse().show_cursor(visible);
    }

    /// Puts the pointer at this place, in screen pixels of the picture area
    /// (the gamepad's stick moving the pointer; it stays free to leave).
    pub fn warp_pointer(&mut self, x: f64, y: f64) {
        let density = self.window.pixel_density();
        self.sdl.mouse().warp_mouse_in_window(&self.window, points(x, density) as f32, points(y, density) as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_with_a_fixed_place_go_by_their_place() {
        assert_eq!(key_of(Some(Scancode::Return), None), Some(Key::Enter));
        assert_eq!(key_of(Some(Scancode::KpEnter), None), Some(Key::NumPadEnter));
        assert_eq!(key_of(Some(Scancode::Kp1), Some(Keycode::Kp1)), Some(Key::NumPad1));
        assert_eq!(key_of(Some(Scancode::KpMinus), None), Some(Key::NumPadMinus));
        assert_eq!(key_of(Some(Scancode::LAlt), None), Some(Key::LeftAlt));
        assert_eq!(key_of(Some(Scancode::PageUp), None), Some(Key::PageUp));
    }

    #[test]
    fn letters_follow_the_keyboard_layout() {
        // a German keyboard: the key in the place of Y says Z
        assert_eq!(key_of(Some(Scancode::Y), Some(Keycode::Z)), Some(Key::Z));
        assert_eq!(key_of(Some(Scancode::A), Some(Keycode::_0)), Some(Key::Key0));
        // the key right of 0: "=" on English keyboards, "+" on German ones
        assert_eq!(key_of(None, Some(Keycode::Equals)), Some(Key::Equal));
        assert_eq!(key_of(None, Some(Keycode::Plus)), Some(Key::Equal));
        assert_eq!(key_of(None, Some(Keycode::Apostrophe)), Some(Key::Apostrophe));
        assert_eq!(key_of(None, Some(Keycode::PlusMinus)), None);
        assert_eq!(key_of(None, None), None);
    }

    #[test]
    fn holding_a_key_repeats_it() {
        // not the menu of accented letters macOS would open over the game
        // for a held A or S (the Avatar walks with held keys)
        set_hints();
        assert_eq!(sdl3::hint::get("SDL_MAC_PRESS_AND_HOLD").as_deref(), Some("0"));
    }

    #[test]
    fn points_and_pixels() {
        assert_eq!(points(1280.0, 2.0), 640.0);
        assert_eq!(pixels(320.5, 2.0), 641.0);
        assert_eq!(points(1280.0, 1.0), 1280.0);
        assert_eq!(pixels(10.0, 0.0), 10.0); // a density SDL could not tell counts as 1
    }
}
