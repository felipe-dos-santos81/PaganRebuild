//! Option "cycle through things" in open containers (modern controls, made
//! for the pad): while a container's window is open, two buttons go
//! through the things in the open windows one by one, the backpack's
//! pages turning along; the thing picked is framed and says its name.
//! "Use" uses it, and one button moves it between the backpack and the
//! other container open (out of the backpack, or into it). The Avatar's
//! own window with what he wears is left out: it shows no container.

use crate::game::{Game, AVATAR};
use crate::view::Surface;

/// The frame around the thing picked: the gold of the menu's studs.
const FRAME: u32 = 0xFFE070;

impl Game {
    /// Is a container's window open (not just the Avatar's own window)?
    pub fn item_windows_open(&self) -> bool {
        self.ui.gumps.iter().any(|g| g.owner != AVATAR)
    }

    /// The things in the open windows in the order they are gone through:
    /// window by window as they lie on the screen, from the left (not by
    /// which is in front: the window of the thing picked comes to the
    /// front); in the backpack's slot window slot by slot (a pile once),
    /// elsewhere row by row from the top left.
    fn window_things(&mut self) -> Vec<u16> {
        let mut windows: Vec<(i32, i32, u16)> = self.ui.gumps.iter().filter(|g| g.owner != AVATAR).map(|g| (g.x, g.y, g.owner)).collect();
        windows.sort();
        let owners: Vec<u16> = windows.into_iter().map(|w| w.2).collect();
        let mut all = Vec::new();
        for owner in owners {
            if self.grid_window(owner) {
                all.extend(self.grid_layout(owner).into_iter().map(|(_, id)| id));
                continue;
            }
            let mut things: Vec<(i32, i32, u16)> = self
                .map()
                .items
                .iter()
                .filter(|it| !it.gone && it.parent == owner && it.id != 0)
                .filter(|it| !self.data.infos.get(it.shape as usize).is_some_and(|si| si.editor))
                // (inside a container the y field holds the place in the window)
                .map(|it| ((it.y >> 8) & 0xFF, it.y & 0xFF, it.id))
                .collect();
            things.sort();
            all.extend(things.into_iter().map(|t| t.2));
        }
        all
    }

    /// Picks the next thing in the open windows (`step` 1) or the one
    /// before (-1); the first press the first (or last) one. The backpack
    /// turns to the page it is on. None if the windows are empty.
    pub fn pick_in_window(&mut self, step: i32) -> Option<u16> {
        if !self.options.pick_things {
            self.ui.window_picked = None;
            return None;
        }
        let things = self.window_things();
        if things.is_empty() {
            self.ui.window_picked = None;
            return None;
        }
        let n = things.len() as i32;
        let at = match self.ui.window_picked.and_then(|id| things.iter().position(|&t| t == id)) {
            Some(i) => (i as i32 + step).rem_euclid(n),
            None if step < 0 => n - 1,
            None => 0,
        };
        let id = things[at as usize];
        if self.ui.window_picked != Some(id) {
            self.ui.window_picked_name = self.look_name(id).or_else(|| self.unnamed_thing(id));
        }
        self.ui.window_picked = Some(id);
        self.show_page_of(id);
        // its window comes to the front, as when it is clicked
        if let Some(parent) = self.map().get(id).map(|it| it.parent) {
            if let Some(gi) = self.ui.gumps.iter().position(|g| g.owner == parent) {
                let g = self.ui.gumps.remove(gi);
                self.ui.gumps.push(g);
            }
        }
        Some(id)
    }

    /// The thing picked in a window, while it is still in an open one.
    pub fn window_picked(&mut self) -> Option<u16> {
        let id = self.ui.window_picked?;
        if !self.options.pick_things || !self.window_things().contains(&id) {
            self.ui.window_picked = None;
        }
        self.ui.window_picked
    }

    /// The backpack's slot window turns to the page the thing is on.
    fn show_page_of(&mut self, id: u16) {
        let Some(owner) = self.map().get(id).map(|it| it.parent) else { return };
        if !self.grid_window(owner) {
            return;
        }
        if let Some((slot, _)) = self.grid_layout(owner).into_iter().find(|&(_, t)| t == id) {
            self.ui.gump_pages.insert(owner, slot / crate::inventory::PER_PAGE);
        }
    }

    /// Moves the thing picked between the backpack and the other container
    /// open: out of the backpack into the container opened last, from
    /// anywhere else into the backpack, with the checks of dragging it there
    /// (weight, room; taking what belongs to others counts as stealing).
    /// The pick stays on it. False if nothing moved (the player is told why
    /// where there is a reason).
    pub fn carry_picked(&mut self) -> bool {
        let Some(id) = self.window_picked() else { return false };
        let Some(pack) = self.backpack() else { return false };
        let parent = self.map().get(id).map_or(0, |it| it.parent);
        if parent != pack {
            return self.take_into_backpack(id) == Some(true);
        }
        // the container opened last that the Avatar does not carry himself
        let Some(target) = self.ui.gumps.iter().rev().map(|g| g.owner).find(|&o| o != AVATAR && o != pack && self.root_container(o) != AVATAR) else {
            return false;
        };
        let g = self.ui.gumps.iter().find(|g| g.owner == target).map(|g| g.shape).unwrap_or(0);
        let area = self.data.gump_areas.get(g.saturating_sub(1) as usize).copied().unwrap_or([0, 0, 64, 64]);
        let (w, h) = ((area[2] - area[0]).max(1), (area[3] - area[1]).max(1));
        // somewhere in its window, as when the original puts something in
        let (gx, gy) = (8.min(w / 2) + (self.rng.next() % (w - 8).max(1) as u32) as i32, 8.min(h / 2) + (self.rng.next() % (h - 8).max(1) as u32) as i32);
        self.put_in_container(id, target, gx, gy)
    }

    /// A frame around the thing picked, if it is in window `gi` (on the
    /// page shown).
    pub fn draw_window_pick(&mut self, surf: &mut Surface, gi: usize) {
        let Some(id) = self.ui.window_picked else { return };
        let Some(it) = self.map().get(id).copied() else { return };
        let owner = self.ui.gumps[gi].owner;
        if it.parent != owner {
            return;
        }
        let rect = if self.grid_window(owner) {
            self.grid_slot_rect(gi, id)
        } else {
            let Some((ox, oy)) = self.gump_item_origin(gi) else { return };
            let Some(f) = self.data.shapes.get(it.shape).and_then(|s| s.frames.get(it.frame as usize).map(|f| (f.xoff, f.yoff, f.width, f.height))) else { return };
            let (x, y) = (ox + (it.y & 0xFF) - f.0, oy + ((it.y >> 8) & 0xFF) - f.1);
            Some((x - 2, y - 2, x + f.2 + 1, y + f.3 + 1))
        };
        let Some((x0, y0, x1, y1)) = rect else { return };
        for x in x0..=x1 {
            surf.put(x, y0, FRAME);
            surf.put(x, y1, FRAME);
        }
        for y in y0..=y1 {
            surf.put(x0, y, FRAME);
            surf.put(x1, y, FRAME);
        }
    }
}
