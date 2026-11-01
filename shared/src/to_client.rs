//! Messages the server sends to clients.

use crate::message;

message! {
    /// Everything about a player that changes often.
    pub struct PlayerTO {
        /// Player id.
        pub id: u32,
        /// X position.
        pub x: f32,
        /// Y position.
        pub y: f32,
        /// Aim in radians.
        pub aim: f32,
        /// Current health.
        pub health: f32,
        /// Max health.
        pub max_health: f32,
        /// Held weapon id.
        pub weapon: u8,
        /// Held weapon's variant (0 plain, 1 gold, 2 diamond, 3 ruby, 4 emerald).
        pub variant: u8,
        /// Hat id (0 = none).
        pub hat: u8,
        /// Accessory id (0 = none).
        pub accessory: u8,
        /// Skin color id.
        pub skin: u8,
        /// Clan id, if in one.
        pub clan: Option<u32>,
        /// Whether they're underground (in the caves).
        pub underground: bool,
    }
}

message! {
    /// A player came into the game.
    pub struct AddPlayerData {
        /// Whether this is the receiving player.
        pub is_mine: bool,
        /// Display name.
        pub name: String,
        /// State.
        pub data: PlayerTO,
    }
}

message! {
    /// Every player's current state.
    pub struct UpdatePlayerData {
        /// Players.
        pub players: Vec<PlayerTO>,
    }
}

message! {
    /// An animal in view.
    pub struct AnimalTO {
        /// Entity index.
        pub id: u32,
        /// X position.
        pub x: f32,
        /// Y position.
        pub y: f32,
        /// Facing in radians.
        pub dir: f32,
        /// Animal kind (see `web/src/data/animals.js`).
        pub kind: u8,
        /// Health from 0 to 1.
        pub health: f32,
    }
}

message! {
    /// Animals in the player's view.
    pub struct AnimalsData {
        /// Animals.
        pub animals: Vec<AnimalTO>,
    }
}

message! {
    /// Someone attacked (plays the swing or recoil animation).
    pub struct HitEventTO {
        /// Player id.
        pub entity_id: u32,
    }
}

message! {
    /// A map object.
    pub struct ObjectTO {
        /// Entity index.
        pub id: u32,
        /// X position.
        pub x: f32,
        /// Y position.
        pub y: f32,
        /// Rotation in radians.
        pub dir: f32,
        /// Collider radius.
        pub scale: f32,
        /// 0 tree, 1 stone, 2 bush, 3 gold, 10-14 crate tiers, 50+ item id.
        pub type_obj: u8,
        /// Current health.
        pub health: f32,
        /// Max health.
        pub max_health: f32,
        /// Owner player id (`u32::MAX` for none).
        pub owner: u32,
    }
}

message! {
    /// Objects that came into view.
    pub struct AddObjectData {
        /// Objects.
        pub objects: Vec<ObjectTO>,
    }
}

message! {
    /// A player's health changed.
    pub struct UpdateHealthData {
        /// Player id.
        pub id: u32,
        /// Current health.
        pub new_health: f32,
        /// Max health.
        pub max_health: f32,
    }
}

message! {
    /// A weapon's variant progress.
    pub struct WeaponProgressTO {
        /// Weapon id.
        pub weapon: u8,
        /// Variant reached.
        pub variant: u8,
        /// Weapon XP.
        pub xp: u32,
        /// XP for the next variant (0 at the last one).
        pub next: u32,
    }
}

message! {
    /// The player's weapons and items.
    pub struct LoadoutData {
        /// Primary weapon id.
        pub primary: u8,
        /// Secondary weapon id.
        pub secondary: Option<u8>,
        /// Item ids, in hotbar order.
        pub items: Vec<u8>,
        /// Held weapon id.
        pub selected: u8,
        /// Variant progress of the owned weapons.
        pub progress: Vec<WeaponProgressTO>,
    }
}

message! {
    /// A tree, stone, etc. was hit (wiggle animation).
    pub struct ObjectHitAnimData {
        /// Entity index.
        pub id: u32,
        /// Direction of the hit.
        pub dir: f32,
    }
}

message! {
    /// The player's resources.
    pub struct SetResourceData {
        /// Wood.
        pub wood: u32,
        /// Stone.
        pub stone: u32,
        /// Food.
        pub food: u32,
        /// Gold (also the score).
        pub gold: u32,
        /// Gun ammo.
        pub ammo: u32,
    }
}

message! {
    /// You died. Shown on the death screen.
    pub struct DeathData {
        /// Who killed you ("" if nobody did).
        pub killer: String,
        /// Score (gold).
        pub score: u32,
        /// Age reached.
        pub age: u32,
        /// Players killed.
        pub kills: u32,
        /// Seconds alive.
        pub time_alive: u32,
    }
}

message! {
    /// A chat message.
    pub struct ChatData {
        /// Sender's player id.
        pub id: u32,
        /// Text.
        pub message: String,
    }
}

message! {
    /// Upgrades the player can pick from (empty lists hide the menu).
    pub struct UpgradesData {
        /// Age the choice is for.
        pub age: u32,
        /// Weapon ids.
        pub weapons: Vec<u8>,
        /// Item ids.
        pub items: Vec<u8>,
        /// Upgrades still waiting after this one.
        pub pending: u32,
    }
}

message! {
    /// One leaderboard row.
    pub struct LeaderEntry {
        /// Player id.
        pub id: u32,
        /// Name.
        pub name: String,
        /// Score (gold).
        pub score: u32,
        /// Kills.
        pub kills: u32,
    }
}

message! {
    /// Top players, plus who holds each challenge icon.
    pub struct LeaderboardData {
        /// Top 10 by score.
        pub entries: Vec<LeaderEntry>,
        /// Highest score (crown).
        pub crown: Option<u32>,
        /// Most kills (skull).
        pub skull: Option<u32>,
        /// Oldest age (star).
        pub star: Option<u32>,
        /// Players online.
        pub online: u32,
    }
}

message! {
    /// A building's health changed.
    pub struct UpdateObjectHealthData {
        /// Entity index.
        pub id: u32,
        /// Current health.
        pub health: f32,
        /// Max health.
        pub max_health: f32,
    }
}

message! {
    /// An object was destroyed and should be removed.
    pub struct RemoveObjectData {
        /// Entity index.
        pub id: u32,
    }
}

message! {
    /// A projectile was fired. Clients move it themselves in a straight line
    /// until it has travelled `range`, or until a `RemoveProjectileData`
    /// arrives.
    pub struct AddProjectileData {
        /// Projectile id.
        pub id: u32,
        /// Start position.
        pub x: f32,
        /// Start position.
        pub y: f32,
        /// Direction in radians.
        pub dir: f32,
        /// Distance per server tick.
        pub speed: f32,
        /// Total distance.
        pub range: f32,
        /// Weapon id that fired it (255 for turrets).
        pub weapon: u8,
        /// Player who fired it (`u32::MAX` for turrets), so clients can start
        /// it at the shooter's on-screen position.
        pub owner: u32,
    }
}

message! {
    /// A projectile hit something.
    pub struct RemoveProjectileData {
        /// Projectile id.
        pub id: u32,
    }
}

message! {
    /// The player's age and progress to the next one.
    pub struct AgeData {
        /// Age (starts at 1).
        pub age: u32,
        /// XP into this age.
        pub xp: u32,
        /// XP needed for the next age.
        pub max_xp: u32,
    }
}

message! {
    /// A short message shown in the middle of the screen.
    pub struct NoticeData {
        /// Text.
        pub text: String,
        /// 0 info, 1 warning, 2 good news.
        pub kind: u8,
    }
}

message! {
    /// Something exploded (TNT).
    pub struct ExplosionData {
        /// Center.
        pub x: f32,
        /// Center.
        pub y: f32,
        /// Blast radius.
        pub radius: f32,
    }
}

message! {
    /// A clan member.
    pub struct ClanMemberTO {
        /// Player id.
        pub id: u32,
        /// Name.
        pub name: String,
        /// 0 member, 1 officer, 2 leader.
        pub role: u8,
    }
}

message! {
    /// A clan.
    pub struct ClanTO {
        /// Clan id.
        pub id: u32,
        /// Name.
        pub name: String,
        /// Members.
        pub members: Vec<ClanMemberTO>,
    }
}

message! {
    /// Every clan, and the receiving player's clan state.
    pub struct ClanStateData {
        /// All clans.
        pub clans: Vec<ClanTO>,
        /// Your clan id.
        pub mine: Option<u32>,
        /// Join requests you can answer: (player id, name).
        pub requests: Vec<ClanMemberTO>,
    }
}

message! {
    /// What the player owns and wears.
    pub struct ShopData {
        /// Owned hat ids.
        pub hats: Vec<u8>,
        /// Owned accessory ids.
        pub accessories: Vec<u8>,
        /// Worn hat (0 = none).
        pub hat: u8,
        /// Worn accessory (0 = none).
        pub accessory: u8,
        /// Owned skin colors (account-wide).
        pub skins: Vec<u8>,
        /// Selected skin color.
        pub skin: u8,
        /// Account coins (earned each game, spent on skins).
        pub coins: u32,
    }
}

message! {
    /// The player's current quest.
    pub struct QuestData {
        /// Description.
        pub text: String,
        /// Progress so far.
        pub progress: u32,
        /// Amount needed.
        pub goal: u32,
        /// Gold reward.
        pub reward: u32,
    }
}

message! {
    /// The current world event ("" when none).
    pub struct WorldEventData {
        /// Event name.
        pub name: String,
        /// Description.
        pub description: String,
        /// Seconds left.
        pub seconds_left: u32,
    }
}

message! {
    /// The player's chest.
    pub struct ChestData {
        /// Whether the player is standing next to their chest.
        pub near: bool,
        /// Whether they have a chest at all.
        pub has_chest: bool,
        /// Stored wood.
        pub wood: u32,
        /// Stored stone.
        pub stone: u32,
        /// Stored food.
        pub food: u32,
        /// Stored gold.
        pub gold: u32,
    }
}

message! {
    /// A point on the minimap.
    pub struct MinimapPoint {
        /// X position.
        pub x: f32,
        /// Y position.
        pub y: f32,
        /// 0 clan member, 1 boss, 2 event crate.
        pub kind: u8,
    }
}

message! {
    /// Things to show on the minimap besides yourself.
    pub struct MinimapData {
        /// Points.
        pub points: Vec<MinimapPoint>,
    }
}

message! {
    /// You spawned (sent once per life).
    pub struct SpawnedData {
        /// Your player id.
        pub id: u32,
        /// Account token to save for next time.
        pub token: String,
        /// Lifetime stats: best score.
        pub best_score: u32,
        /// Lifetime stats: total kills.
        pub total_kills: u32,
        /// Lifetime stats: games played.
        pub games: u32,
        /// Coins earned in the last game.
        pub coins_earned: u32,
    }
}

message! {
    /// An arena room.
    pub struct ArenaRoomTO {
        /// Room id.
        pub id: u32,
        /// Host's name.
        pub host: String,
        /// Players per team.
        pub team_size: u8,
        /// Players in the room.
        pub players: Vec<ClanMemberTO>,
        /// Whether the fight has started.
        pub started: bool,
        /// Map preset.
        pub map: u8,
        /// Max health.
        pub max_health: f32,
        /// Building damage multiplier.
        pub object_damage: f32,
        /// Knockback multiplier.
        pub bounciness: f32,
    }
}

message! {
    /// Arena rooms and the receiving player's room.
    pub struct ArenaStateData {
        /// Open rooms.
        pub rooms: Vec<ArenaRoomTO>,
        /// Your room.
        pub mine: Option<u32>,
        /// Last result ("" when none).
        pub result: String,
    }
}

message! {
    /// A dug-out cave cell (see `server/src/systems/cave_system.rs`).
    pub struct CaveCellTO {
        /// Cell column.
        pub x: i16,
        /// Cell row.
        pub y: i16,
    }
}

message! {
    /// A partly dug cave point.
    pub struct CaveDugTO {
        /// Point column.
        pub x: i16,
        /// Point row.
        pub y: i16,
        /// How far it's dug, 1-255 (255 would be open).
        pub amount: u8,
    }
}

message! {
    /// Cave points that are dug out (or partly dug).
    pub struct CaveCellsData {
        /// Replace everything known so far (sent on spawn).
        pub reset: bool,
        /// Newly opened points.
        pub cells: Vec<CaveCellTO>,
        /// Points being dug, with their progress.
        pub dug: Vec<CaveDugTO>,
    }
}
