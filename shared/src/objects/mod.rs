use bevy_ecs::component::Component;

#[cfg_attr(feature = "server", derive(Component))]
pub enum GameObjects {
    UsableGameObjects(UsableGameObjects),
    PlacableGameObjects(PlacableGameObjects),
    StaticGameObjects(StaticGameObjects),
}

// healing
pub enum UsableGameObjects {
    Apple,
}

pub enum PlacableGameObjects {
    Block,
}

pub enum StaticGameObjects {
    Tree,
    Stone,
    Bush,
}
