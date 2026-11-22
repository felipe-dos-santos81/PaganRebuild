//! The window opened by winit: the backend of platform.rs on Windows and
//! Linux (and on the Mac, when the program is built without SDL). winit
//! opens the window and reports the keyboard and the mouse; what the events
//! mean for the game is decided in platform.rs (`Input`).
//!
//! The window is not driven by an event loop that calls the game; the
//! game's own loop asks for the events now and then (`pump_app_events`).

use super::{Input, Key};
use std::sync::Arc;
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
use winit::window::WindowId;

/// The key a key event is about.
fn key_of(event: &winit::event::KeyEvent) -> Option<Key> {
    use Key::*;
    let PhysicalKey::Code(code) = event.physical_key else { return None };
    // the number pad without NumLock: its arrows are arrows
    if let winit::keyboard::Key::Named(named) = &event.logical_key {
        use winit::keyboard::NamedKey as N;
        let arrow = match named {
            N::ArrowUp => Some(Up),
            N::ArrowDown => Some(Down),
            N::ArrowLeft => Some(Left),
            N::ArrowRight => Some(Right),
            N::PageUp => Some(PageUp),
            N::PageDown => Some(PageDown),
            _ => None,
        };
        if arrow.is_some() {
            return arrow;
        }
    }
    // keys taken by their place (the number pad gives digits as text too)
    let by_place = match code {
        KeyCode::F1 => Some(F1), KeyCode::F2 => Some(F2), KeyCode::F3 => Some(F3), KeyCode::F4 => Some(F4),
        KeyCode::F5 => Some(F5), KeyCode::F6 => Some(F6), KeyCode::F7 => Some(F7), KeyCode::F8 => Some(F8),
        KeyCode::F9 => Some(F9), KeyCode::F10 => Some(F10), KeyCode::F11 => Some(F11), KeyCode::F12 => Some(F12),
        KeyCode::ArrowUp => Some(Up), KeyCode::ArrowDown => Some(Down),
        KeyCode::ArrowLeft => Some(Left), KeyCode::ArrowRight => Some(Right),
        KeyCode::PageUp => Some(PageUp), KeyCode::PageDown => Some(PageDown),
        KeyCode::Home => Some(Home), KeyCode::End => Some(End),
        KeyCode::Insert => Some(Insert), KeyCode::Delete => Some(Delete),
        KeyCode::Escape => Some(Escape), KeyCode::Enter => Some(Enter), KeyCode::Space => Some(Space),
        KeyCode::Tab => Some(Tab), KeyCode::Backspace => Some(Backspace),
        KeyCode::ShiftLeft => Some(LeftShift), KeyCode::ShiftRight => Some(RightShift),
        KeyCode::ControlLeft => Some(LeftCtrl), KeyCode::ControlRight => Some(RightCtrl),
        KeyCode::AltLeft => Some(LeftAlt), KeyCode::AltRight => Some(RightAlt),
        KeyCode::Numpad0 => Some(NumPad0), KeyCode::Numpad1 => Some(NumPad1), KeyCode::Numpad2 => Some(NumPad2),
        KeyCode::Numpad3 => Some(NumPad3), KeyCode::Numpad4 => Some(NumPad4), KeyCode::Numpad5 => Some(NumPad5),
        KeyCode::Numpad6 => Some(NumPad6), KeyCode::Numpad7 => Some(NumPad7), KeyCode::Numpad8 => Some(NumPad8),
        KeyCode::Numpad9 => Some(NumPad9),
        KeyCode::NumpadSubtract => Some(NumPadMinus), KeyCode::NumpadAdd => Some(NumPadPlus),
        KeyCode::NumpadEnter => Some(NumPadEnter),
        _ => None,
    };
    if by_place.is_some() {
        return by_place;
    }
    // letters, digits, signs: what the key says without shift
    let winit::keyboard::Key::Character(s) = event.key_without_modifiers() else { return None };
    let c = s.chars().next()?.to_ascii_lowercase();
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

/// How the window is to be opened, and the window once it is there (winit
/// makes it when its event loop starts).
struct Opening {
    title: String,
    size: PhysicalSize<u32>,
    fullscreen: bool,
    window: Option<Arc<winit::window::Window>>,
    error: Option<String>,
}

/// The part winit calls with the events, for the time of one `pump`.
struct Handler<'a> {
    opening: &'a mut Opening,
    input: &'a mut Input,
    key_map: &'a crate::hash::HashMap<Key, Option<Key>>,
}

impl Handler<'_> {
    /// A key pressed or let go: typed text, the key as the game sees it
    /// (after the key map), Alt+Enter for the full screen.
    fn keyboard_input(&mut self, event: &winit::event::KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        if pressed {
            if let Some(t) = &event.text {
                self.input.text(t);
            }
        }
        let Some(raw) = key_of(event) else { return };
        if self.input.key(raw, pressed, event.repeat, self.key_map) {
            if let Some(w) = &self.opening.window {
                let full = w.fullscreen().is_some();
                w.set_fullscreen((!full).then_some(winit::window::Fullscreen::Borderless(None)));
            }
        }
    }
}

impl ApplicationHandler for Handler<'_> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        let o = &mut *self.opening;
        if o.window.is_some() {
            return;
        }
        let attrs = winit::window::Window::default_attributes()
            .with_title(o.title.clone())
            .with_inner_size(o.size)
            .with_min_inner_size(PhysicalSize::new(320, 200))
            .with_resizable(true)
            .with_fullscreen(o.fullscreen.then_some(winit::window::Fullscreen::Borderless(None)));
        match el.create_window(attrs) {
            Ok(w) => o.window = Some(Arc::new(w)),
            Err(e) => o.error = Some(format!("no window: {e}")),
        }
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let input = &mut *self.input;
        match event {
            WindowEvent::CloseRequested => input.closed = true,
            WindowEvent::Focused(gained) => input.focus(gained),
            WindowEvent::KeyboardInput { event, .. } => self.keyboard_input(&event),
            WindowEvent::ModifiersChanged(m) => input.alt = m.state().alt_key(),
            WindowEvent::CursorMoved { position, .. } => {
                input.mouse = (position.x, position.y);
                input.inside = true;
            }
            WindowEvent::CursorEntered { .. } => input.inside = true,
            WindowEvent::CursorLeft { .. } => input.inside = false,
            WindowEvent::MouseInput { state, button, .. } => {
                let i = match button {
                    winit::event::MouseButton::Left => 0,
                    winit::event::MouseButton::Right => 1,
                    winit::event::MouseButton::Middle => 2,
                    _ => return,
                };
                input.mouse_button(i, state == ElementState::Pressed);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32 / 40.0, p.y as f32 / 40.0),
                };
                input.wheel.0 += x;
                input.wheel.1 += y;
            }
            _ => {}
        }
    }
}

/// The window and its events.
pub struct Backend {
    /// What the window's events left behind since the last look.
    pub input: Input,
    /// Keys that do what another key does (the player's own key bindings);
    /// None: the key does nothing.
    pub key_map: crate::hash::HashMap<Key, Option<Key>>,
    events: EventLoop<()>,
    opening: Opening,
    window: Arc<winit::window::Window>,
}

impl Backend {
    /// Opens the window with a picture area of `size` screen pixels.
    pub fn new(title: &str, size: (u32, u32), fullscreen: bool) -> Result<Backend, String> {
        let mut events = EventLoop::new().map_err(|e| format!("no event loop: {e}"))?;
        let mut opening = Opening { title: title.to_string(), size: PhysicalSize::new(size.0, size.1), fullscreen, window: None, error: None };
        let mut input = Input::default();
        let key_map = Default::default();
        // the window is made when the event loop starts
        for _ in 0..100 {
            let mut handler = Handler { opening: &mut opening, input: &mut input, key_map: &key_map };
            let _ = events.pump_app_events(Some(Duration::ZERO), &mut handler);
            if opening.window.is_some() || opening.error.is_some() {
                break;
            }
        }
        if let Some(e) = opening.error.take() {
            return Err(e);
        }
        let window = opening.window.clone().ok_or("the window did not open")?;
        Ok(Backend { input, key_map, events, opening, window })
    }

    /// The surface the pictures are drawn to.
    pub fn surface(&self, instance: &wgpu::Instance) -> Result<wgpu::Surface<'static>, String> {
        instance.create_surface(self.window.clone()).map_err(|e| format!("no surface: {e}"))
    }

    /// Takes the events that came in into `input`.
    pub fn pump(&mut self) {
        let mut handler = Handler { opening: &mut self.opening, input: &mut self.input, key_map: &self.key_map };
        if let PumpStatus::Exit(_) = self.events.pump_app_events(Some(Duration::ZERO), &mut handler) {
            self.input.closed = true;
        }
    }

    /// The picture area in screen pixels.
    pub fn pixel_size(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        (size.width.max(1), size.height.max(1))
    }

    pub fn is_minimized(&self) -> bool {
        self.window.is_minimized().unwrap_or(false)
    }

    /// The screen's refresh rate, if the system tells it.
    pub fn refresh_millihertz(&self) -> Option<u32> {
        self.window.current_monitor()?.refresh_rate_millihertz()
    }

    pub fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.window.set_cursor_visible(visible);
    }

    /// Puts the pointer at this place, in screen pixels of the picture area.
    pub fn warp_pointer(&mut self, x: f64, y: f64) {
        let _ = self.window.set_cursor_position(PhysicalPosition::new(x, y));
    }
}
