//! Static game data: weapons, items, hats, animals, and the age progression
//! rules that tie weapons and items together.

pub mod animals;
pub mod hats;
pub mod items;
pub mod weapons;

use items::{item_def, ITEMS};
use weapons::{weapon, Slot, WEAPONS};

/// The last age that offers an upgrade. Ages keep going after it for score.
pub const MAX_UPGRADE_AGE: u32 = 10;

/// XP needed to go from `age` to the next age.
pub fn xp_for_age(age: u32) -> u32 {
    let exponent = i32::try_from(age.saturating_sub(1)).unwrap_or(i32::MAX);
    // Fits easily: age 100 needs about 1.8e13, far below f64's exact range,
    // and the result is clamped to u32.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let xp = (100.0 * 1.3_f64.powi(exponent)).round().min(f64::from(u32::MAX)) as u32;
    xp
}

/// What a player owns, for working out upgrades.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owned {
    /// Primary weapon id.
    pub primary:   u8,
    /// Secondary weapon id.
    pub secondary: Option<u8>,
    /// Item ids.
    pub items:     Vec<u8>,
}

impl Owned {
    /// A fresh player: fists and the age 1 items.
    pub fn starting() -> Self {
        Self {
            primary:   weapons::FISTS,
            secondary: None,
            items:     items::starting_items(),
        }
    }

    /// Whether the player owns this weapon.
    pub fn has_weapon(&self, id: u8) -> bool {
        self.primary == id || self.secondary == Some(id)
    }

    /// Whether the player owns this item.
    pub fn has_item(&self, id: u8) -> bool {
        self.items.contains(&id)
    }

    /// Weapon ids owned, primary first.
    pub fn weapons(&self) -> Vec<u8> {
        std::iter::once(self.primary).chain(self.secondary).collect()
    }
}

/// Weapons and items offered at `age` to a player owning `owned`.
pub fn upgrade_options(age: u32, owned: &Owned) -> (Vec<u8>, Vec<u8>) {
    let weapon_ids = WEAPONS
        .iter()
        .filter(|w| w.age == age && !owned.has_weapon(w.id))
        .filter(|w| {
            if w.pre.is_empty() {
                // New secondaries only fill an empty slot.
                w.slot == Slot::Secondary && owned.secondary.is_none()
            } else {
                w.pre.iter().any(|&p| owned.has_weapon(p))
            }
        })
        .map(|w| w.id)
        .collect();

    let item_ids = ITEMS
        .iter()
        .filter(|i| i.age == age && !owned.has_item(i.id))
        .filter(|i| {
            let in_group = owned_in_group(owned, i.group);
            if i.pre.is_some() {
                // Upgrades replace any older item of the same group, so
                // skipping a tier doesn't lock you out of later ones.
                in_group.is_some_and(|o| o.age < i.age)
            } else {
                in_group.is_none()
            }
        })
        .map(|i| i.id)
        .collect();

    (weapon_ids, item_ids)
}

/// The owned item in a group, if any.
fn owned_in_group(owned: &Owned, group: items::Group) -> Option<&'static items::ItemDef> {
    owned
        .items
        .iter()
        .filter_map(|&id| item_def(id))
        .find(|o| o.group == group)
}

/// Applies an upgrade pick. Returns false if it isn't one of the options for
/// `age`.
pub fn apply_upgrade(age: u32, owned: &mut Owned, is_item: bool, id: u8) -> bool {
    let (weapon_ids, item_ids) = upgrade_options(age, owned);

    if is_item {
        if !item_ids.contains(&id) {
            return false;
        }
        let group = item_def(id).map(|i| i.group);
        match owned.items.iter().position(|&o| item_def(o).map(|i| i.group) == group) {
            Some(slot) => owned.items[slot] = id,
            None => owned.items.push(id),
        }
        return true;
    }

    if !weapon_ids.contains(&id) {
        return false;
    }
    match weapon(id).map(|w| w.slot) {
        Some(Slot::Primary) => owned.primary = id,
        Some(Slot::Secondary) => owned.secondary = Some(id),
        None => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_curve_grows() {
        assert_eq!(xp_for_age(1), 100);
        assert_eq!(xp_for_age(2), 130);
        assert!(xp_for_age(10) > xp_for_age(9));
        assert!(xp_for_age(500) > 0);
    }

    /// Walks every possible upgrade path (always picking option `choice`
    /// modulo the number offered) and checks each age offers something.
    #[test]
    fn every_age_offers_something_on_every_path() {
        for choice in 0..12 {
            let mut owned = Owned::starting();
            for age in 2..=MAX_UPGRADE_AGE {
                let (weapon_ids, item_ids) = upgrade_options(age, &owned);
                let options: Vec<(bool, u8)> = weapon_ids
                    .iter()
                    .map(|&w| (false, w))
                    .chain(item_ids.iter().map(|&i| (true, i)))
                    .collect();
                assert!(!options.is_empty(), "nothing at age {age} on path {choice}");
                let (is_item, id) = options[choice % options.len()];
                assert!(apply_upgrade(age, &mut owned, is_item, id));
            }
        }
    }

    #[test]
    fn guns_come_at_age_five_and_upgrade_in_place() {
        let mut owned = Owned::starting();
        let (weapon_ids, _) = upgrade_options(5, &owned);
        assert!(weapon_ids.contains(&17), "pistol at age 5");
        assert!(apply_upgrade(5, &mut owned, false, 17));
        assert_eq!(owned.secondary, Some(17));

        // Hand cannon and machine pistol both upgrade from the pistol.
        let (weapon_ids, _) = upgrade_options(8, &owned);
        assert!(weapon_ids.contains(&19) && weapon_ids.contains(&20));
        assert!(!weapon_ids.contains(&22), "assault rifle needs the SMG");
        assert!(apply_upgrade(8, &mut owned, false, 19));
        assert_eq!(owned.secondary, Some(19));
    }

    #[test]
    fn items_replace_their_predecessor() {
        let mut owned = Owned::starting();
        assert!(apply_upgrade(3, &mut owned, true, 4), "stone wall");
        assert!(owned.has_item(4) && !owned.has_item(3));
        assert!(!apply_upgrade(3, &mut owned, true, 99));
        assert!(!apply_upgrade(4, &mut owned, true, 4), "already owned");
    }
}
