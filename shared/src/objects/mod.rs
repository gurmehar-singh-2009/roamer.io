
#[cfg_attr(feature = "server", derive(bevy_ecs::component::Component))]
pub enum GameObjects {
    UsableGameObjects(UsableGameObjects),
    PlacableGameObjects(PlacableGameObjects),
    StaticGameObjects(StaticGameObjects),
}

// healing
pub enum UsableGameObjects {
    Apple,
}

/// Buildings a player can place.
/// The discriminant is the `kind` byte sent in `PlaceBuildingMessage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PlacableGameObjects {
    Wall  = 0,
    Spike = 1,
}

impl TryFrom<u8> for PlacableGameObjects {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Wall),
            1 => Ok(Self::Spike),
            other => Err(other),
        }
    }
}

pub enum StaticGameObjects {
    Tree,
    Stone,
    Bush,
}
