//! Animals and bosses. Ids are sent to clients and must match
//! `web/src/data/animals.js`.

/// How an animal reacts to players.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behavior {
    /// Wanders, runs away when hit.
    Passive,
    /// Wanders, fights back when hit.
    Neutral,
    /// Hunts nearby players.
    Hostile,
    /// Swims in schools (boids) in the ocean.
    Fish,
}

/// Where an animal lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Habitat {
    /// Anywhere on land.
    Land,
    /// The ocean.
    Ocean,
}

/// An animal definition.
#[derive(Clone, Copy, Debug)]
pub struct AnimalDef {
    /// Id sent to clients.
    pub id:       u8,
    /// Display name.
    pub name:     &'static str,
    /// Max health.
    pub health:   f32,
    /// Top speed per tick.
    pub speed:    f32,
    /// Collider radius.
    pub radius:   f32,
    /// How it behaves.
    pub behavior: Behavior,
    /// Bite damage.
    pub damage:   f32,
    /// Food dropped on death.
    pub food:     u32,
    /// Gold dropped on death.
    pub gold:     u32,
    /// XP for the killer.
    pub xp:       u32,
    /// How many live at once (bosses spawn through world events instead).
    pub count:    u32,
    /// Where it lives.
    pub habitat:  Habitat,
    /// Whether it's a boss (announced, shown on the minimap).
    pub boss:     bool,
}

/// Shorthand for a land animal.
#[allow(clippy::too_many_arguments)]
const fn land(
    id: u8,
    name: &'static str,
    (health, speed, radius): (f32, f32, f32),
    behavior: Behavior,
    damage: f32,
    (food, gold, xp): (u32, u32, u32),
    count: u32,
    boss: bool,
) -> AnimalDef {
    AnimalDef {
        id,
        name,
        health,
        speed,
        radius,
        behavior,
        damage,
        food,
        gold,
        xp,
        count,
        habitat: Habitat::Land,
        boss,
    }
}

/// The fish id.
pub const FISH: u8 = 0;
/// The goldfish id.
pub const GOLDFISH: u8 = 9;

/// Every animal, indexed by id.
pub static ANIMALS: [AnimalDef; 14] = [
    AnimalDef {
        id:       FISH,
        name:     "Fish",
        health:   20.0,
        speed:    6.7,
        radius:   34.0,
        behavior: Behavior::Fish,
        damage:   0.0,
        food:     5,
        gold:     0,
        xp:       5,
        count:    4000,
        habitat:  Habitat::Ocean,
        boss:     false,
    },
    land(
        1,
        "Wolf",
        (300.0, 22.0, 60.0),
        Behavior::Hostile,
        8.0,
        (30, 0, 60),
        24,
        false,
    ),
    land(
        2,
        "Cow",
        (500.0, 14.0, 55.0),
        Behavior::Passive,
        0.0,
        (100, 0, 100),
        30,
        false,
    ),
    land(
        3,
        "Pig",
        (800.0, 18.0, 65.0),
        Behavior::Passive,
        0.0,
        (80, 0, 90),
        25,
        false,
    ),
    land(
        4,
        "Bull",
        (1800.0, 20.0, 84.0),
        Behavior::Neutral,
        20.0,
        (200, 0, 200),
        10,
        false,
    ),
    land(
        5,
        "Bully",
        (2800.0, 21.0, 84.0),
        Behavior::Hostile,
        20.0,
        (300, 0, 300),
        5,
        false,
    ),
    land(
        6,
        "Sheep",
        (120.0, 15.0, 59.0),
        Behavior::Passive,
        0.0,
        (40, 0, 40),
        30,
        false,
    ),
    land(
        7,
        "Chicken",
        (40.0, 16.0, 40.0),
        Behavior::Passive,
        0.0,
        (10, 0, 15),
        25,
        false,
    ),
    land(
        8,
        "Duck",
        (50.0, 16.0, 40.0),
        Behavior::Passive,
        0.0,
        (10, 0, 15),
        25,
        false,
    ),
    AnimalDef {
        id:       GOLDFISH,
        name:     "Goldfish",
        health:   40.0,
        speed:    8.0,
        radius:   34.0,
        behavior: Behavior::Fish,
        damage:   0.0,
        food:     5,
        gold:     50,
        xp:       20,
        count:    80,
        habitat:  Habitat::Ocean,
        boss:     false,
    },
    land(
        10,
        "Sid",
        (4000.0, 18.0, 90.0),
        Behavior::Hostile,
        30.0,
        (500, 1500, 1500),
        0,
        true,
    ),
    land(
        11,
        "Vince",
        (6000.0, 17.0, 90.0),
        Behavior::Hostile,
        40.0,
        (800, 2500, 2500),
        0,
        true,
    ),
    land(
        12,
        "Raider",
        (3500.0, 23.0, 84.0),
        Behavior::Hostile,
        25.0,
        (300, 1200, 1200),
        0,
        true,
    ),
    land(
        13,
        "Alpha Wolf",
        (2500.0, 25.0, 84.0),
        Behavior::Hostile,
        22.0,
        (200, 800, 900),
        0,
        true,
    ),
];

/// Looks up an animal by id.
pub fn animal(id: u8) -> Option<&'static AnimalDef> {
    ANIMALS.get(usize::from(id))
}

/// Ids of every boss.
pub fn bosses() -> impl Iterator<Item = u8> {
    ANIMALS.iter().filter(|a| a.boss).map(|a| a.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_positions() {
        for (i, a) in ANIMALS.iter().enumerate() {
            assert_eq!(usize::from(a.id), i, "{} has the wrong id", a.name);
        }
    }
}
