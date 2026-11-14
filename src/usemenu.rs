//! A small menu for a thing that can be both used and taken (option
//! "cycle through things", made for the pad): "use" on such a thing asks
//! which of the two is meant, beside the thing. Up and down choose, "use"
//! (Enter, the pad's A) confirms, Esc (B) cancels; a click on a line takes
//! it. The game stands still meanwhile.

use crate::game::{Game, AVATAR};
use crate::view::Surface;

/// The menu, while it is open.
pub struct UseMenu {
    /// The thing it is about.
    pub item: u16,
    /// The line chosen: 0 use, 1 take.
    pub sel: usize,
}

const BACK: u32 = 0x1C140C;
const EDGE: u32 = 0xFFE070;
/// Room around the lines and for the stud before the one chosen.
const PAD: i32 = 4;
const STUD: i32 = 10;

impl Game {
    /// Does "use" on this thing ask first? It can be used and also taken,
    /// and the Avatar does not carry it yet. Not a container: that is
    /// opened (a barrel is rarely carried off; the pad's attack button
    /// still takes it).
    pub(crate) fn asks_use_or_take(&mut self, id: u16) -> bool {
        let container = self
            .map()
            .get(id)
            .and_then(|it| self.data.infos.get(it.shape as usize))
            .is_some_and(|si| si.family == crate::shape::FAMILY_CONTAINER);
        self.options.pick_things && !container && self.portable(id) && self.root_container(id) != AVATAR && self.usable(id)
    }

    pub fn open_use_menu(&mut self, id: u16) {
        self.ui.use_menu = Some(UseMenu { item: id, sel: 0 });
    }

    pub fn close_use_menu(&mut self) {
        self.ui.use_menu = None;
    }

    fn use_menu_lines(&self) -> [&'static str; 2] {
        self.tr([["Benutzen", "Nehmen"], ["Utiliser", "Prendre"], ["Use", "Take"]])
    }

    /// The thing's name over the two lines (instead of over the thing,
    /// where the menu would cover it), if it has one.
    fn use_menu_title(&self) -> Option<String> {
        let id = self.ui.use_menu.as_ref()?.item;
        if self.ui.window_picked == Some(id) {
            self.ui.window_picked_name.clone()
        } else {
            self.ui.picked_name.clone().filter(|_| self.ui.picked == Some(id))
        }
    }

    /// Up (-1) or down (1) to the other line.
    pub fn use_menu_key(&mut self, dy: i32) {
        if let Some(m) = &mut self.ui.use_menu {
            m.sel = (m.sel as i32 + dy).rem_euclid(2) as usize;
        }
    }

    /// Does what the line chosen says. A thing in the world is let go
    /// afterwards; one in a window stays picked (it may be used again).
    pub fn use_menu_confirm(&mut self) {
        let Some(m) = self.ui.use_menu.take() else { return };
        let id = m.item;
        let in_window = self.ui.window_picked == Some(id);
        // a walk the player sent him on ends here
        self.cancel_approach();
        let in_world = self.map().get(id).is_some_and(|it| it.in_world());
        let done = if m.sel == 0 {
            // (in the world it may lie further than "use" reaches - on a
            // table the hand reaches: he steps up to it first, as after a
            // click)
            if in_world {
                self.use_or_approach(id);
            } else {
                self.interact(id, true);
            }
            true
        } else {
            self.take_into_backpack(id) == Some(true)
        };
        if done && !in_window {
            self.ui.picked = None;
        }
    }

    /// Where the menu is: beside the thing (in the world or in its window),
    /// kept inside the picture. Left, top, right, bottom, the height of a
    /// line and the top of the first of the two lines (below the title).
    fn use_menu_rect(&mut self, w: i32, h: i32) -> Option<(i32, i32, i32, i32, i32, i32)> {
        let id = self.ui.use_menu.as_ref()?.item;
        let it = self.map().get(id).copied()?;
        let (ax, ay) = if it.in_world() {
            let height = self.data.infos.get(it.shape as usize).map_or(0, |si| si.z * 8);
            let (sx, sy) = self.to_screen(it.x, it.y, it.z);
            (sx + 12, sy - height / 2)
        } else {
            let (bx, by) = self.window_bark_place(it)?;
            (bx + 12, by + 4)
        };
        let title = self.use_menu_title();
        let font = self.data.font.as_ref()?;
        let lines = self.use_menu_lines();
        let lh = font.height + 2;
        let title_w = title.as_deref().map_or(0, |t| font.width(t));
        let width = (lines.iter().map(|l| font.width(l)).max().unwrap_or(0) + STUD).max(title_w) + 2 * PAD;
        let title_h = if title.is_some() { lh + 2 } else { 0 };
        let height = title_h + 2 * lh + 2 * PAD;
        let x0 = ax.clamp(0, (w - width).max(0));
        let y0 = (ay - height / 2).clamp(0, (h - height).max(0));
        Some((x0, y0, x0 + width, y0 + height, lh, y0 + PAD + title_h))
    }

    pub fn draw_use_menu(&mut self, surf: &mut Surface) {
        let Some(sel) = self.ui.use_menu.as_ref().map(|m| m.sel) else { return };
        let Some((x0, y0, x1, y1, lh, first)) = self.use_menu_rect(surf.width, surf.height) else { return };
        let title = self.use_menu_title();
        surf.fill_rect(x0, y0, x1, y1, BACK);
        for x in x0..x1 {
            surf.put(x, y0, EDGE);
            surf.put(x, y1 - 1, EDGE);
        }
        for y in y0..y1 {
            surf.put(x0, y, EDGE);
            surf.put(x1 - 1, y, EDGE);
        }
        let lines = self.use_menu_lines();
        let Some(font) = &self.data.font else { return };
        if let Some(t) = &title {
            // the name, and a line under it
            font.draw(surf, t, x0 + PAD, y0 + PAD, &self.data.pal);
            for x in x0 + PAD..x1 - PAD {
                surf.put(x, first - 2, EDGE);
            }
        }
        for (i, line) in lines.iter().enumerate() {
            let y = first + i as i32 * lh;
            font.draw(surf, line, x0 + PAD + STUD, y, &self.data.pal);
            if i == sel {
                crate::menu::draw_stud(surf, x0 + PAD + 3, y + font.height / 2, true, true);
            }
        }
    }

    /// A click with the menu open: on a line it does that, anywhere else
    /// it closes the menu.
    pub fn use_menu_click(&mut self, x: i32, y: i32, w: i32, h: i32) {
        match self.use_menu_rect(w, h) {
            Some((x0, y0, x1, y1, lh, first)) if x >= x0 && x < x1 && y >= y0 && y < y1 => {
                // (a click on the title does nothing)
                if y < first {
                    return;
                }
                let line = ((y - first) / lh).clamp(0, 1) as usize;
                if let Some(m) = &mut self.ui.use_menu {
                    m.sel = line;
                }
                self.use_menu_confirm();
            }
            _ => self.close_use_menu(),
        }
    }
}
