//! The window, keyboard, mouse and picture output, for Windows, Linux and
//! macOS alike. This file is the window as the game sees it; the window
//! itself is opened, and its events reported, by the backend: winit
//! (platform/winit.rs), or SDL3 when the program is built with the feature
//! "sdl" (platform/sdl.rs). wgpu hands the finished pictures to the screen
//! (Vulkan on Windows and Linux, Metal on the Mac, OpenGL where neither is
//! there).
//!
//! The game asks for its input the way it did with minifb: "is this key
//! down", "was it pressed since the last picture", "where is the mouse".
//! Here the window's events are collected and answered from that. The
//! window is not driven by an event loop that calls the game; the game's
//! own loop asks for the events now and then (`Backend::pump`).
//!
//! The window can be resized and maximized, and Alt+Enter switches to the
//! full screen and back. The picture keeps its proportions: it is fitted
//! into the window, with black bars where the window is wider or taller.
//! The game makes its pictures in the fitted size (`display_size`).
//!
//! Every picture goes to the screen whole, in step with its refresh
//! ("Fifo"), so there is no tearing line across it; handing over a picture
//! waits for the next refresh when the previous one is still waiting.

use std::time::Duration;

#[cfg_attr(feature = "sdl", path = "platform/sdl.rs")]
#[cfg_attr(not(feature = "sdl"), path = "platform/winit.rs")]
mod backend;

/// The keys the game knows. Letters, digits and signs follow the
/// keyboard layout (Z is the key labelled Z, also on a German keyboard),
/// the others are taken by their place.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Up, Down, Left, Right, PageUp, PageDown, Home, End, Insert, Delete,
    Escape, Enter, Space, Tab, Backspace,
    LeftShift, RightShift, LeftCtrl, RightCtrl, LeftAlt, RightAlt,
    Comma, Period, Minus, Equal, Apostrophe,
    NumPad0, NumPad1, NumPad2, NumPad3, NumPad4, NumPad5, NumPad6, NumPad7, NumPad8, NumPad9,
    NumPadMinus, NumPadPlus, NumPadEnter,
}

/// The buttons of a gamepad, named as on the usual pads (A at the bottom,
/// B right, X left, Y at the top).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Pad {
    A, B, X, Y,
    /// Shoulder buttons and triggers.
    LB, RB, LT, RT,
    Back, Start,
    /// The sticks pressed in.
    LStick, RStick,
    Up, Down, Left, Right,
}

impl Pad {
    pub const ALL: [Pad; 16] = [
        Pad::A, Pad::B, Pad::X, Pad::Y, Pad::LB, Pad::RB, Pad::LT, Pad::RT, Pad::Back, Pad::Start, Pad::LStick, Pad::RStick,
        Pad::Up, Pad::Down, Pad::Left, Pad::Right,
    ];

    fn of(b: gilrs::Button) -> Option<Pad> {
        use gilrs::Button as G;
        Some(match b {
            G::South => Pad::A,
            G::East => Pad::B,
            G::West => Pad::X,
            G::North => Pad::Y,
            G::LeftTrigger => Pad::LB,
            G::RightTrigger => Pad::RB,
            G::LeftTrigger2 => Pad::LT,
            G::RightTrigger2 => Pad::RT,
            G::Select => Pad::Back,
            G::Start => Pad::Start,
            G::LeftThumb => Pad::LStick,
            G::RightThumb => Pad::RStick,
            G::DPadUp => Pad::Up,
            G::DPadDown => Pad::Down,
            G::DPadLeft => Pad::Left,
            G::DPadRight => Pad::Right,
            _ => return None,
        })
    }
}

/// Whether held keys repeating count as new presses.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyRepeat {
    Yes,
    No,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// What `get_mouse_pos` does when the pointer is outside the window:
/// keep it at the edge (`Clamp`) or report nothing (`Discard`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MouseMode {
    Clamp,
    Discard,
}

/// What a mouse button did since the last look.
#[derive(Default, Clone, Copy)]
pub struct Edges {
    pub pressed: bool,
    pub released: bool,
}

/// The graphics card's side: the window's surface, and the picture as a
/// texture drawn into the window.
struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// The picture, smoothed and stretched to the window (gpuscale.rs).
    scaler: crate::gpuscale::Scaler,
}

impl Gpu {
    fn new(backend: &backend::Backend) -> Result<Gpu, String> {
        // WGPU_BACKEND=vulkan|gl|metal picks one (for tests)
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::from_env_or_default());
        let surface = backend.surface(&instance)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or("no graphics adapter")?;
        let info = adapter.get_info();
        eprintln!("graphics: {} through {:?}", info.name, info.backend);
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("pagan"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| format!("no device: {e}"))?;
        let caps = surface.get_capabilities(&adapter);
        // the pixels are sRGB already: a format that would convert them again is avoided
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let size = backend.pixel_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.0,
            height: size.1,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: crate::debug::env().latency.unwrap_or(1),
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let scaler = crate::gpuscale::Scaler::new(&device, format);
        Ok(Gpu { surface, device, queue, config, scaler })
    }

    /// The window's size in screen pixels, as it is now.
    fn resize(&mut self, size: (u32, u32)) {
        if size.0 > 0 && size.1 > 0 && size != (self.config.width, self.config.height) {
            (self.config.width, self.config.height) = size;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Takes the picture (0x00RRGGBB per pixel, as the game draws), to be
    /// shown with `look`.
    fn upload(&mut self, pixels: &[u32], w: u32, h: u32, look: crate::gpuscale::Look) {
        self.scaler.set_picture(&self.device, &self.queue, pixels, w, h, look);
    }

    /// Shows the picture in the part `rect` (x, y, width, height) of the
    /// window, black around it; waits if the screen has not taken the one
    /// before yet. False if nothing could be shown.
    fn draw(&mut self, rect: (u32, u32, u32, u32)) -> bool {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return false;
            }
            Err(_) => return false,
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("picture") });
        let drawn = {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("picture"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let (x, y, w, h) = rect;
            let (x, y) = (x.min(self.config.width - 1), y.min(self.config.height - 1));
            let (w, h) = (w.clamp(1, self.config.width - x), h.clamp(1, self.config.height - y));
            self.scaler.draw(&self.device, &self.queue, &mut pass, (x, y, w, h))
        };
        if !drawn {
            return false;
        }
        self.queue.submit([enc.finish()]);
        frame.present();
        true
    }
}

/// What the window's events left behind since the last look.
#[derive(Default)]
struct Input {
    down: crate::hash::HashSet<Key>,
    /// Pressed since the last `update`, in order; true: by repeating.
    pressed: Vec<(Key, bool)>,
    /// Position in window pixels, and whether the pointer is over the window.
    mouse: (f64, f64),
    inside: bool,
    buttons: [bool; 3],
    /// Presses and releases of left and right (see `take_clicks`).
    edges: [Edges; 2],
    wheel: (f32, f32),
    /// What the keys typed since the last look (with the keyboard's layout,
    /// shift, dead keys ...).
    typed: String,
    closed: bool,
    /// An Alt key is held (as the system reports it).
    alt: bool,
    /// The keys held and what they were taken for (see `App::key_map`),
    /// so that letting go lets go of the same key.
    held_as: crate::hash::HashMap<Key, Key>,
    /// The window has the keyboard (it is the one in front).
    focused: bool,
    /// When the window last got the focus back: a click right after it is
    /// the one that brought the window to the front, not meant for the game.
    focused_at: Option<std::time::Instant>,
    /// Buttons whose press was such a click (their release is let go too).
    swallowed: [bool; 3],
}

impl Input {
    /// A key pressed or let go, as the keyboard names it (before the key
    /// map). True: Alt+Enter was pressed anew; the backend switches the full
    /// screen, and the game does not see this Enter.
    fn key(&mut self, raw: Key, pressed: bool, repeat: bool, key_map: &crate::hash::HashMap<Key, Option<Key>>) -> bool {
        let k = if pressed {
            match key_map.get(&raw).copied().unwrap_or(Some(raw)) {
                Some(k) => {
                    self.held_as.insert(raw, k);
                    k
                }
                None => return false,
            }
        } else {
            // let go as what it was pressed as, whatever the map says by now
            match self.held_as.remove(&raw) {
                Some(k) => k,
                None => return false,
            }
        };
        if crate::debug::env().keys {
            eprintln!("key {k:?} {} repeat {repeat} alt {}", if pressed { "Pressed" } else { "Released" }, self.alt);
        }
        // Alt+Enter: full screen or back (the game does not see this Enter)
        if (k == Key::Enter || k == Key::NumPadEnter) && self.alt {
            return pressed && !repeat;
        }
        if pressed {
            self.down.insert(k);
            self.pressed.push((k, repeat));
        } else {
            self.down.remove(&k);
        }
        false
    }

    /// What the keys typed (with the keyboard's layout, shift, dead keys
    /// ...), without control characters.
    fn text(&mut self, typed: &str) {
        self.typed.extend(typed.chars().filter(|c| !c.is_control()));
        if crate::debug::env().keys {
            eprintln!("typed {typed:?}");
        }
    }

    /// A mouse button (0 left, 1 right, 2 middle) pressed or let go. A
    /// click that brings the window back to the front is not passed on.
    fn mouse_button(&mut self, i: usize, pressed: bool) {
        if pressed {
            let fresh = self.focused_at.take().is_some_and(|t| t.elapsed() < Duration::from_millis(300));
            self.swallowed[i] = fresh;
            if fresh {
                return;
            }
        } else if std::mem::take(&mut self.swallowed[i]) {
            return;
        }
        self.buttons[i] = pressed;
        if i < 2 {
            if pressed {
                self.edges[i].pressed = true;
            } else {
                self.edges[i].released = true;
            }
        }
    }

    /// The pad's stick pushes the pointer by window pixels, in a window of
    /// this size. Gives the pointer's new place, or None when it stays
    /// where it is.
    fn push_pointer(&mut self, dx: f64, dy: f64, size: (u32, u32)) -> Option<(f64, f64)> {
        // with another program in front the pointer is that program's: a
        // stick still pushed must not pull it back into the game
        if !self.focused {
            return None;
        }
        let x = (self.mouse.0 + dx).clamp(0.0, size.0.saturating_sub(1) as f64);
        let y = (self.mouse.1 + dy).clamp(0.0, size.1.saturating_sub(1) as f64);
        self.mouse = (x, y);
        self.inside = true;
        Some(self.mouse)
    }

    /// The window got the keyboard (true) or lost it.
    fn focus(&mut self, gained: bool) {
        self.focused = gained;
        if gained {
            self.focused_at = Some(std::time::Instant::now());
        } else {
            // keys let go while another window had the keyboard never come back up
            self.down.clear();
            self.buttons = [false; 3];
            self.focused_at = None;
        }
    }
}

/// Waits for the desktop's next refresh (Windows). Vulkan under Windows
/// gives the buffers back at uneven moments (the desktop copies the picture
/// rather than flipping to it), so waiting there alone makes one pass take
/// 5 ms and the next 15; pictures handed over right after the desktop's
/// refresh go out evenly. False if there is nothing to wait for.
#[cfg(windows)]
fn desktop_refresh() -> bool {
    use std::sync::atomic::{AtomicU32, Ordering};
    // how often in a row the wait came back at once
    static INSTANT: AtomicU32 = AtomicU32::new(0);
    let start = std::time::Instant::now();
    // SAFETY: a plain call without arguments
    let ok = unsafe { windows_sys::Win32::Graphics::Dwm::DwmFlush() } == 0;
    // a wait that keeps coming back at once means there is no composition
    // (or the window is hidden): then the swap chain paces alone
    if ok && start.elapsed() < Duration::from_micros(300) {
        return INSTANT.fetch_add(1, Ordering::Relaxed) < 5;
    }
    INSTANT.store(0, Ordering::Relaxed);
    ok
}

#[cfg(not(windows))]
fn desktop_refresh() -> bool {
    false
}

/// The game's window.
pub struct Window {
    /// The pictures' way to the screen. It stays before `backend`: its
    /// surface must be gone before the window it draws to.
    gpu: Gpu,
    backend: backend::Backend,
    /// The size the game opened the window with; the mouse is reported in
    /// these units (as minifb did), whatever size the window has on screen.
    logical: (usize, usize),
    /// Keys pressed by the last `update` (`pressed` collects the next).
    pressed: Vec<(Key, bool)>,
    /// The text typed by then.
    typed: String,
    wheel: Option<(f32, f32)>,
    /// Wait for the desktop's refresh before handing over (see `desktop_refresh`).
    desktop_pace: bool,
    /// The picture fills the whole window, whatever its proportions (the
    /// game then shows more of the world to the sides or above and below).
    fill: bool,
    /// Gamepads (None if they cannot be used on this system).
    pads: Option<gilrs::Gilrs>,
    /// The pads' buttons pressed (true) and let go (false) since the last
    /// picture, and the ones collected for the next.
    pad_events: Vec<(Pad, bool)>,
    pad_next: Vec<(Pad, bool)>,
    /// Where the sticks are pushed (left, right): x right, y up, -1..1.
    stick: (f32, f32),
    right_stick: (f32, f32),
    /// The pad's buttons held.
    pad_held: crate::hash::HashSet<Pad>,
    /// Keys and mouse buttons held down for the pad (see `set_virtual`).
    virtual_down: crate::hash::HashSet<Key>,
    virtual_buttons: [bool; 2],
}

impl Window {
    /// Opens a window of `w` x `h` screen pixels, or of the size `screen`
    /// asks for (`--window`, for videos): the game still counts in `w` x `h`
    /// then, and the picture is fitted into the window as after resizing it.
    pub fn new(title: &str, w: usize, h: usize, screen: Option<(u32, u32)>) -> Result<Window, String> {
        // PAGAN_FULLSCREEN=1: in the full screen from the start (for tests)
        let backend = backend::Backend::new(title, screen.unwrap_or((w as u32, h as u32)), crate::debug::env().fullscreen)?;
        let gpu = Gpu::new(&backend)?;
        let desktop_pace = cfg!(windows) && !crate::debug::env().no_dwm;
        Ok(Window { gpu, backend, logical: (w, h), pressed: Vec::new(), typed: String::new(), wheel: None, desktop_pace, fill: false,
            pads: gilrs::Gilrs::new().ok(), pad_events: Vec::new(), pad_next: Vec::new(), stick: (0.0, 0.0), right_stick: (0.0, 0.0), pad_held: Default::default(), virtual_down: Default::default(), virtual_buttons: [false; 2] })
    }

    fn pump(&mut self) {
        self.backend.pump();
        if let Some(pads) = &mut self.pads {
            while let Some(ev) = pads.next_event() {
                match ev.event {
                    gilrs::EventType::ButtonPressed(b, _) => {
                        if let Some(p) = Pad::of(b) {
                            self.pad_next.push((p, true));
                            self.pad_held.insert(p);
                        }
                    }
                    gilrs::EventType::ButtonReleased(b, _) => {
                        if let Some(p) = Pad::of(b) {
                            self.pad_next.push((p, false));
                            self.pad_held.remove(&p);
                        }
                    }
                    gilrs::EventType::AxisChanged(gilrs::Axis::LeftStickX, v, _) => self.stick.0 = v,
                    gilrs::EventType::AxisChanged(gilrs::Axis::LeftStickY, v, _) => self.stick.1 = v,
                    gilrs::EventType::AxisChanged(gilrs::Axis::RightStickX, v, _) => self.right_stick.0 = v,
                    gilrs::EventType::AxisChanged(gilrs::Axis::RightStickY, v, _) => self.right_stick.1 = v,
                    gilrs::EventType::Disconnected => {
                        self.stick = (0.0, 0.0);
                        self.right_stick = (0.0, 0.0);
                        self.pad_held.clear();
                    }
                    _ => {}
                }
            }
        }
    }

    /// The gamepad buttons pressed (true) and let go (false) since the
    /// picture before.
    pub fn pad_events(&self) -> &[(Pad, bool)] {
        &self.pad_events
    }

    /// The left stick: x right, y up, each -1..1.
    pub fn stick(&self) -> (f32, f32) {
        self.stick
    }

    /// The right stick, the same way.
    pub fn right_stick(&self) -> (f32, f32) {
        self.right_stick
    }

    /// The pad's buttons held, and the ones pressed since the picture
    /// before (a quick press may be over already).
    pub fn pad_down(&self, p: Pad) -> bool {
        self.pad_held.contains(&p) || self.pad_events.contains(&(p, true))
    }

    /// Moves the pointer by window pixels (the pad's right stick), while
    /// the window is the one in front.
    pub fn move_pointer(&mut self, dx: f64, dy: f64) {
        let size = self.backend.pixel_size();
        if let Some((x, y)) = self.backend.input.push_pointer(dx, dy, size) {
            self.backend.warp_pointer(x, y);
        }
    }

    /// The window's size in its pixels.
    pub fn pixel_size(&self) -> (u32, u32) {
        self.backend.pixel_size()
    }

    /// Is this key held by a button of the pad (not by the keyboard)?
    pub fn pad_holds(&self, k: Key) -> bool {
        self.virtual_down.contains(&k)
    }

    /// The keys and mouse buttons (left, right) the pad holds down now.
    /// Keys that come anew count as pressed in this picture, the ones
    /// missing are let go. `repeat`: held keys pressed again (by repeating).
    pub fn set_virtual(&mut self, keys: &crate::hash::HashSet<Key>, buttons: [bool; 2], repeat: &[Key]) {
        let input = &mut self.backend.input;
        for &k in keys {
            if self.virtual_down.insert(k) {
                input.down.insert(k);
                self.pressed.push((k, false));
            }
        }
        let gone: Vec<Key> = self.virtual_down.difference(keys).copied().collect();
        for k in gone {
            self.virtual_down.remove(&k);
            // unless the keyboard holds it as well
            if !input.held_as.values().any(|&h| h == k) {
                input.down.remove(&k);
            }
        }
        for &k in repeat {
            if self.virtual_down.contains(&k) {
                self.pressed.push((k, true));
            }
        }
        for i in 0..2 {
            if buttons[i] != self.virtual_buttons[i] {
                self.virtual_buttons[i] = buttons[i];
                input.buttons[i] = buttons[i];
                if buttons[i] {
                    input.edges[i].pressed = true;
                } else {
                    input.edges[i].released = true;
                }
            }
        }
    }

    /// Keys that do what other keys do (the player's own bindings).
    pub fn set_key_map(&mut self, map: crate::hash::HashMap<Key, Option<Key>>) {
        self.backend.key_map = map;
    }


    pub fn is_open(&self) -> bool {
        !self.backend.input.closed
    }

    /// The screen's refresh time, if the system tells it.
    pub fn refresh_period(&self) -> Option<Duration> {
        let mhz = self.backend.refresh_millihertz()?;
        (mhz > 0).then(|| Duration::from_secs_f64(1000.0 / mhz as f64))
    }

    pub fn set_title(&mut self, title: &str) {
        self.backend.set_title(title);
    }

    /// Where the picture goes in the window: x, y, width, height in screen
    /// pixels, as large as fits with the proportions of the size the window
    /// was opened with.
    fn fit(&self) -> (u32, u32, u32, u32) {
        let size = self.backend.pixel_size();
        let (sw, sh) = (size.0 as u64, size.1 as u64);
        if self.fill {
            return (0, 0, sw as u32, sh as u32);
        }
        let (lw, lh) = (self.logical.0.max(1) as u64, self.logical.1.max(1) as u64);
        // the full width, unless the height is too small for it
        let (w, h) = if sw * lh <= sh * lw { (sw, (sw * lh / lw).max(1)) } else { ((sh * lw / lh).max(1), sh) };
        (((sw - w) / 2) as u32, ((sh - h) / 2) as u32, w as u32, h as u32)
    }

    /// The size the pictures are shown in, in screen pixels (it changes
    /// with the window).
    /// The picture fills the whole window (true) or keeps the proportions
    /// the window was opened with, with black bars beside it (false).
    pub fn set_fill(&mut self, on: bool) {
        self.fill = on;
    }

    /// The size the game draws for, in the units the window was opened
    /// with: that size, or, filling the window, as high as that and as
    /// wide as the window's proportions make it.
    pub fn logical_size(&self) -> (usize, usize) {
        if !self.fill {
            return self.logical;
        }
        let (sw, sh) = self.backend.pixel_size();
        ((self.logical.1 * sw as usize / sh as usize).max(1), self.logical.1)
    }

    pub fn display_size(&self) -> (usize, usize) {
        let (_, _, w, h) = self.fit();
        (w as usize, h as usize)
    }

    pub fn set_cursor_visibility(&mut self, visible: bool) {
        self.backend.set_cursor_visible(visible);
    }

    pub fn is_key_down(&self, key: Key) -> bool {
        self.backend.input.down.contains(&key)
    }

    /// Pressed since the picture before (with `Yes` also by holding it).
    pub fn is_key_pressed(&self, key: Key, repeat: KeyRepeat) -> bool {
        self.pressed.iter().any(|&(k, rep)| k == key && (repeat == KeyRepeat::Yes || !rep))
    }

    /// The text typed since the picture before.
    pub fn typed(&self) -> &str {
        &self.typed
    }

    pub fn get_keys_pressed(&self, repeat: KeyRepeat) -> Vec<Key> {
        self.pressed.iter().filter(|&&(_, rep)| repeat == KeyRepeat::Yes || !rep).map(|&(k, _)| k).collect()
    }

    pub fn get_mouse_down(&self, button: MouseButton) -> bool {
        self.backend.input.buttons[match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
        }]
    }

    /// The mouse position in the units the window was opened with.
    pub fn get_mouse_pos(&self, mode: MouseMode) -> Option<(f32, f32)> {
        let (fx, fy, fw, fh) = self.fit();
        let (lw, lh) = self.logical_size();
        let (lw, lh) = (lw as f64, lh as f64);
        let x = (self.backend.input.mouse.0 - fx as f64) * lw / fw.max(1) as f64;
        let y = (self.backend.input.mouse.1 - fy as f64) * lh / fh.max(1) as f64;
        match mode {
            MouseMode::Clamp => Some((x.clamp(0.0, lw - 1.0) as f32, y.clamp(0.0, lh - 1.0) as f32)),
            MouseMode::Discard => {
                (self.backend.input.inside && x >= 0.0 && y >= 0.0 && x < lw && y < lh).then_some((x as f32, y as f32))
            }
        }
    }

    /// The wheel's turn since the picture before (up is positive).
    pub fn get_scroll_wheel(&self) -> Option<(f32, f32)> {
        self.wheel
    }

    /// Presses and releases of the left and the right button since the
    /// last call. A short click (a touchpad tap) can go down and up between
    /// two pictures; it is still seen here.
    pub fn take_clicks(&mut self) -> (Edges, Edges) {
        let e = std::mem::take(&mut self.backend.input.edges);
        (e[0], e[1])
    }

    /// Takes the events that came in, and shows the last picture again
    /// (so that the pace stays that of the screen).
    pub fn update(&mut self) {
        let rect = self.fit();
        if self.desktop_pace {
            desktop_refresh();
        }
        self.gpu.resize(self.backend.pixel_size());
        if !self.gpu.draw(rect) {
            std::thread::sleep(Duration::from_millis(8));
        }
        self.events_in();
    }

    /// Shows a picture of `w` x `h` pixels, stretched over the whole
    /// window, with `look` (sharp, smoothed or as on a tube monitor); `times`: for how many screen refreshes (1 or more).
    pub fn update_with_buffer(&mut self, pixels: &[u32], w: usize, h: usize, look: crate::gpuscale::Look, times: u32) -> Result<(), String> {
        if pixels.len() < w * h {
            return Err(format!("picture of {w}x{h} with only {} pixels", pixels.len()));
        }
        let rect = self.fit();
        if self.backend.is_minimized() {
            // nothing to show it in: keep the pace anyway
            std::thread::sleep(Duration::from_millis(16));
        } else {
            self.gpu.resize(self.backend.pixel_size());
            for i in 0..times.max(1) {
                if self.desktop_pace {
                    desktop_refresh();
                }
                if i == 0 {
                    self.gpu.upload(pixels, w as u32, h as u32, look);
                }
                if !self.gpu.draw(rect) {
                    std::thread::sleep(Duration::from_millis(8));
                }
            }
        }
        self.events_in();
        Ok(())
    }

    fn events_in(&mut self) {
        self.pump();
        self.pressed = std::mem::take(&mut self.backend.input.pressed);
        self.typed = std::mem::take(&mut self.backend.input.typed);
        self.pad_events = std::mem::take(&mut self.pad_next);
        let wheel = std::mem::take(&mut self.backend.input.wheel);
        self.wheel = (wheel != (0.0, 0.0)).then_some(wheel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type Map = crate::hash::HashMap<Key, Option<Key>>;

    #[test]
    fn alt_enter_is_not_a_key_for_the_game() {
        let (mut input, map) = (Input::default(), Map::default());
        input.alt = true;
        assert!(input.key(Key::Enter, true, false, &map));
        assert!(!input.key(Key::Enter, true, true, &map)); // held: no second switch
        assert!(!input.key(Key::Enter, false, false, &map));
        assert!(input.down.is_empty() && input.pressed.is_empty());
    }

    #[test]
    fn a_mapped_key_is_let_go_as_what_it_was_pressed_as() {
        let mut input = Input::default();
        let mut map = Map::default();
        map.insert(Key::A, Some(Key::B));
        assert!(!input.key(Key::A, true, false, &map));
        assert!(input.down.contains(&Key::B) && input.pressed == vec![(Key::B, false)]);
        map.clear(); // the bindings change while the key is held
        input.key(Key::A, false, false, &map);
        assert!(input.down.is_empty());
    }

    #[test]
    fn a_key_mapped_to_nothing_does_nothing() {
        let mut input = Input::default();
        let mut map = Map::default();
        map.insert(Key::A, None);
        input.key(Key::A, true, false, &map);
        assert!(input.down.is_empty() && input.pressed.is_empty());
    }

    #[test]
    fn typed_text_has_no_control_characters() {
        let mut input = Input::default();
        input.text("a\u{8}\u{e4}\r");
        assert_eq!(input.typed, "a\u{e4}");
    }

    #[test]
    fn the_click_that_brings_the_window_back_is_not_for_the_game() {
        let mut input = Input::default();
        input.focus(true);
        input.mouse_button(0, true);
        input.mouse_button(0, false);
        assert!(!input.buttons[0] && !input.edges[0].pressed && !input.edges[0].released);
        input.mouse_button(0, true); // the next one counts
        assert!(input.buttons[0] && input.edges[0].pressed);
    }

    #[test]
    fn the_stick_moves_the_pointer_only_in_the_window_in_front() {
        let mut input = Input { mouse: (10.0, 10.0), ..Default::default() };
        // another program is in front: the pointer is left to it
        assert_eq!(input.push_pointer(5.0, -50.0, (100, 80)), None);
        assert_eq!(input.mouse, (10.0, 10.0));
        input.focus(true);
        assert_eq!(input.push_pointer(5.0, -50.0, (100, 80)), Some((15.0, 0.0)));
        assert_eq!(input.push_pointer(500.0, 500.0, (100, 80)), Some((99.0, 79.0)));
        assert!(input.inside);
        input.focus(false);
        assert_eq!(input.push_pointer(1.0, 1.0, (100, 80)), None);
    }

    #[test]
    fn losing_focus_lets_go_of_everything() {
        let (mut input, map) = (Input::default(), Map::default());
        input.key(Key::W, true, false, &map);
        input.mouse_button(1, true);
        input.focus(false);
        assert!(input.down.is_empty() && input.buttons == [false; 3]);
    }
}
