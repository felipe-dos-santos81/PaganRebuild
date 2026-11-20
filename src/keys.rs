//! Option "better inventory": a key the Avatar takes goes onto his key
//! ring by itself, as if he had dropped it onto the ring.
//!
//! Only plain door keys (shape 82 with the number of their lock, 1 to
//! 255): the key blank (number 0), the keys with a larger number and
//! things of other shapes that open something (the Key of the Caretaker)
//! stay where they are; scripts may want them by themselves.

use crate::controls::KEYRING;
use crate::game::{Game, AVATAR};

/// The shape of the keys.
pub const KEY: u32 = 82;

/// How often it is looked (ticks).
const LOOK_TICKS: u32 = 4;

impl Game {
    /// Once in a while: keys new in the Avatar's things go onto the first
    /// key ring he carries. The keys he had when looked first (after
    /// loading or a change of map) stay where they are.
    pub fn keys_to_ring(&mut self) {
        if !self.tick.is_multiple_of(LOOK_TICKS) || !self.player_in_control() {
            return;
        }
        if !self.options.better_inventory {
            self.ui.keys_carried = None;
            return;
        }
        let carried: Vec<(u16, u16)> = self
            .map()
            .items
            .iter()
            .filter(|it| !it.gone && it.shape == KEY && (1..=255).contains(&it.quality) && it.parent != 0)
            .filter(|it| self.root_container(it.id) == AVATAR)
            .map(|it| (it.id, it.parent))
            .collect();
        let ring = self.key_ring();
        if let (Some(known), Some(ring)) = (self.ui.keys_carried.take(), ring) {
            for &(id, parent) in &carried {
                let on_ring = self.map().get(parent).is_some_and(|p| p.shape == KEYRING);
                if known.contains(&id) || on_ring {
                    continue;
                }
                if self.put_in_container(id, ring, 8, 8) {
                    let text = self.tr(["Schl\u{81}ssel am Schl\u{81}sselring", "Cl\u{82} sur le trousseau", "Key put on the key ring"]);
                    self.notify(text);
                }
            }
        }
        self.ui.keys_carried = Some(carried.into_iter().map(|c| c.0).collect());
    }

    /// The key ring the Avatar carries (also in a bag in the backpack).
    pub fn key_ring(&self) -> Option<u16> {
        self.map().items.iter().find(|it| !it.gone && it.shape == KEYRING && it.parent != 0 && self.root_container(it.id) == AVATAR).map(|it| it.id)
    }
}
