//! Messages the client sends to the server.

use crate::message;

message! {
    /// Join the game (or respawn after dying).
    pub struct SpawnMessage {
        /// Display name.
        pub name: String,
        /// Account token from a previous session, empty for a new account.
        pub token: String,
    }
}

message! {
    /// Movement input.
    pub struct MoveMessage {
        /// Direction in radians, `None` to stop.
        pub dir: Option<f32>,
    }
}

message! {
    /// Aim input.
    pub struct AimMessage {
        /// Direction in radians.
        pub dir: Option<f32>,
    }
}

message! {
    /// Start or stop attacking (mouse down / up).
    pub struct AttackMessage {
        /// Whether the attack button is held.
        pub attacking: bool,
    }
}

message! {
    /// A chat message.
    pub struct ChatMessage {
        /// Text.
        pub message: String,
    }
}

message! {
    /// Switch to an owned weapon.
    pub struct ChooseWeapon {
        /// Weapon id.
        pub weapon: u8,
    }
}

message! {
    /// Eat food or place a building in front of the player.
    pub struct UseItemMessage {
        /// Item id.
        pub item: u8,
    }
}

message! {
    /// Pick one of the upgrades offered for the current age.
    pub struct ChooseUpgradeMessage {
        /// Whether `id` is an item (otherwise a weapon).
        pub is_item: bool,
        /// Weapon or item id.
        pub id: u8,
    }
}

message! {
    /// Buy or equip a hat, accessory or skin.
    pub struct ShopMessage {
        /// Buy (otherwise equip; equipping id 0 takes a hat or accessory off).
        pub buy: bool,
        /// 0 hat, 1 accessory, 2 skin color.
        pub kind: u8,
        /// Hat, accessory or skin id.
        pub id: u8,
    }
}

message! {
    /// A clan action.
    pub struct ClanMessage {
        /// 0 create, 1 request to join, 2 leave, 3 accept request, 4 decline
        /// request, 5 kick, 6 promote to officer, 7 demote to member.
        pub action: u8,
        /// Clan name (create).
        pub name: String,
        /// Clan id (join) or player id (accept, decline, kick, promote,
        /// demote).
        pub target: u32,
    }
}

message! {
    /// Move resources between the player and their chest. Amounts larger
    /// than what's there move everything.
    pub struct ChestMessage {
        /// Into the chest (otherwise out of it).
        pub deposit: bool,
        /// Wood to move.
        pub wood: u32,
        /// Stone to move.
        pub stone: u32,
        /// Food to move.
        pub food: u32,
        /// Gold to move.
        pub gold: u32,
    }
}

message! {
    /// Go down (or up) the tunnel entrance the player is standing on.
    pub struct CaveMessage {}
}

message! {
    /// Give resources to a clan member.
    pub struct TradeMessage {
        /// Player id.
        pub to: u32,
        /// Wood to give.
        pub wood: u32,
        /// Stone to give.
        pub stone: u32,
        /// Food to give.
        pub food: u32,
        /// Gold to give.
        pub gold: u32,
    }
}

message! {
    /// Arena (team fight) actions.
    pub struct ArenaMessage {
        /// 0 create room, 1 join room, 2 leave, 3 start, 4 set loadout.
        pub action: u8,
        /// Room id (join).
        pub room: u32,
        /// Players per team (create).
        pub team_size: u8,
        /// Map preset (create): 0 open, 1 pillars, 2 maze.
        pub map: u8,
        /// Max health (create).
        pub max_health: f32,
        /// Damage multiplier for buildings (create).
        pub object_damage: f32,
        /// Knockback multiplier (create).
        pub bounciness: f32,
        /// Primary weapon (set loadout).
        pub primary: u8,
        /// Secondary weapon, 255 for none (set loadout).
        pub secondary: u8,
    }
}
