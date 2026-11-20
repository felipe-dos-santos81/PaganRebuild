//! What the player sees and handles on top of the world: the windows of
//! containers and books, dragging, the menus, the savegame book, the
//! title, messages and what the pointer is over. (The code for each lives
//! in its own file: gumps.rs, drag.rs, menu.rs, slots.rs, title.rs, hover.rs ...)

use crate::hash::HashMap;

/// The windows, menus, books and other things on the screen for the player (ui.rs).
#[derive(Default)]
pub struct Ui {
    /// Where the enemy health bars were drawn, down to the creature's
    /// feet: (actor, x0, y0, x1, y1). A click there means the creature.
    pub bar_areas: Vec<(u16, i32, i32, i32, i32)>,
    /// Where the minimap was drawn (x0, y0, x1, y1), to keep clicks off the world.
    pub minimap_area: Option<(i32, i32, i32, i32)>,
    /// The page each slot window shows (option "better inventory").
    pub gump_pages: HashMap<u16, usize>,
    pub gumps: Vec<crate::gumps::Gump>,
    pub reader: Option<crate::gumps::Reader>,
    pub drag: Option<crate::drag::Drag>,
    /// A script waits for the player to click on a target (its process).
    pub targeting: Option<u16>,
    /// The target being picked is for the peace spell (its process).
    pub peace_target: Option<u16>,
    /// A script waits for the player to enter an amount.
    pub slider: Option<crate::magic::SliderInput>,
    /// A short message for the player and when it was put up.
    pub notice: Option<(String, std::time::Instant)>,
    /// The volume menu, while it is open.
    pub menu: Option<crate::menu::Menu>,
    /// The savegame book, while it is open.
    pub slot_menu: Option<crate::slots::SlotMenu>,
    pub hover: Option<u16>,
    /// What looking at the item under the pointer would say (hover.rs).
    pub hover_name: Option<String>,
    /// Where the mouse pointer is in the view (none outside the window).
    pub pointer: Option<(i32, i32)>,
    /// The key is held that lights up everything worth pointing out.
    pub show_all: bool,
    /// The thing in reach picked with the keys for the next "use" (option
    /// "cycle through things", controls.rs); it lights up.
    pub picked: Option<u16>,
    /// Its name, shown over it: it may stand behind the Avatar or
    /// something else, where its light is not seen.
    pub picked_name: Option<String>,
    /// The last thing picked in the world (and where it lay round the
    /// Avatar, where he stood): picking again goes on from there while
    /// he stands still, after the pick was let go (used, a window).
    pub last_picked: Option<(u16, f32, (i32, i32))>,
    /// The thing picked in an open container's window (windowpick.rs),
    /// and its name.
    pub window_picked: Option<u16>,
    pub window_picked_name: Option<String>,
    /// The thing picked last in each container's window (by the
    /// container): switching back to a window goes on there.
    pub window_marks: crate::hash::HashMap<u16, u16>,
    /// The keys the Avatar carried at the last look (keys.rs: a key that
    /// is new among them goes onto his key ring). None: not looked yet
    /// (after loading, a change of map), the keys carried then stay.
    pub keys_carried: Option<crate::hash::HashSet<u16>>,
    /// The small menu "use or take" for a thing that can be both
    /// (usemenu.rs), while it is open.
    pub use_menu: Option<crate::usemenu::UseMenu>,
    /// The main menu, the name entry, a movie or the credits.
    pub title: Option<crate::title::Title>,
    /// Where the pointer was over the main menu the picture before.
    pub title_mouse: Option<(i32, i32)>,
    /// The large map with the list of places (option "guide", key O).
    pub worldmap: Option<crate::worldmap::WorldMap>,
}
