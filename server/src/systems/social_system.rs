//! Clans, trading, the shop, quests, world events, the leaderboard and the
//! minimap.

use std::collections::{HashMap, HashSet};

use bevy_ecs::{component::Component, entity::Entity, resource::Resource, world::World};
use nanorand::Rng;
use shared::{
    to_client::{
        ClanMemberTO, ClanStateData, ClanTO, LeaderEntry, LeaderboardData, MinimapData, MinimapPoint, QuestData,
        WorldEventData,
    },
    to_server::{ClanMessage, ShopMessage, TradeMessage},
    PacketType,
};

use crate::{
    catalog::{
        animals::{animal, bosses},
        hats::{accessory, hat},
    },
    replication::upgrades::send_shop,
    structs::{
        components::{
            Account, AnimalEntity, Dirty, Gear, Name, ObjectKind, PlayerEntity, PlayerStats, Position, Progress,
            Resources, TickCount,
        },
        outbox::{Outbox, Target},
    },
    systems::{
        accounts::Accounts,
        anticheat_system::note_hat_switch,
        damage_system::player_entity,
        map_system::{spawn_animal, spawn_crate},
        player_system::{random_land_spot, Sessions},
        progression_system::grant_xp,
        rng, TICK_MS,
    },
};

// ---------------------------------------------------------------- clans --

/// Max clan name length.
const MAX_CLAN_NAME: usize = 8;

/// A clan.
#[derive(Clone, Debug, Default)]
pub struct Clan {
    /// Clan id.
    pub id:       u32,
    /// Name.
    pub name:     String,
    /// Leader's player id.
    pub leader:   u32,
    /// Officers (can accept and kick members).
    pub officers: HashSet<u32>,
    /// Members, leader included, in join order.
    pub members:  Vec<u32>,
    /// Players asking to join.
    pub requests: Vec<u32>,
}

/// Every clan.
#[derive(Resource, Default)]
pub struct Clans {
    /// Next clan id.
    next_id:   u32,
    /// Clans by id.
    pub clans: HashMap<u32, Clan>,
}

/// The clan a player is in.
fn clan_of(world: &World, id: u32) -> Option<u32> {
    world.resource::<Sessions>().0.get(&id).and_then(|s| s.clan)
}

/// Whether two players are on the same side: the same arena team when
/// fighting in the arena, else the same clan.
pub fn are_allies(world: &World, a: u32, b: u32) -> bool {
    if a == b {
        return false;
    }
    if let Some(same_team) = crate::systems::arena_system::arena_allies(world, a, b) {
        return same_team;
    }
    clan_of(world, a).is_some_and(|c| clan_of(world, b) == Some(c))
}

/// A player's name from their session.
fn session_name(world: &World, id: u32) -> String {
    world
        .resource::<Sessions>()
        .0
        .get(&id)
        .map_or_default(|s| s.name.clone())
}

/// Sets a player's clan in their session.
fn set_clan(world: &mut World, id: u32, clan: Option<u32>) {
    if let Some(s) = world.resource_mut::<Sessions>().0.get_mut(&id) {
        s.clan = clan;
    }
}

/// Sends clan state to one player, or to everyone if `to` is `None`.
pub fn send_clan_state(world: &mut World, to: Option<u32>) {
    let clans: Vec<ClanTO> = world
        .resource::<Clans>()
        .clans
        .values()
        .map(|c| ClanTO {
            id:      c.id,
            name:    c.name.clone(),
            members: c
                .members
                .iter()
                .map(|&m| ClanMemberTO {
                    id:   m,
                    name: session_name(world, m),
                    role: if m == c.leader {
                        2
                    } else {
                        u8::from(c.officers.contains(&m))
                    },
                })
                .collect(),
        })
        .collect();

    let recipients: Vec<u32> = to.map_or_else(
        || world.resource::<Sessions>().0.keys().copied().collect(),
        |id| vec![id],
    );
    for id in recipients {
        let mine = clan_of(world, id);
        let requests = mine
            .and_then(|c| world.resource::<Clans>().clans.get(&c).cloned())
            .filter(|c| c.leader == id || c.officers.contains(&id))
            .map_or_default(|c| {
                c.requests
                    .iter()
                    .map(|&r| ClanMemberTO {
                        id:   r,
                        name: session_name(world, r),
                        role: 0,
                    })
                    .collect()
            });
        world.resource_mut::<Outbox>().send_reliable(
            Target::Player(id),
            PacketType::ClanState,
            &ClanStateData {
                clans: clans.clone(),
                mine,
                requests,
            },
        );
    }
}

/// Removes a player from their clan (disbanding it if they were last).
pub fn leave_clan(world: &mut World, id: u32) {
    let Some(clan_id) = clan_of(world, id) else {
        return;
    };
    set_clan(world, id, None);
    let mut clans = world.resource_mut::<Clans>();
    let Some(clan) = clans.clans.get_mut(&clan_id) else {
        return;
    };
    clan.members.retain(|&m| m != id);
    clan.officers.remove(&id);
    if clan.members.is_empty() {
        clans.clans.remove(&clan_id);
    } else if clan.leader == id {
        // Hand leadership to an officer, else the longest member.
        let next = clan.officers.iter().min().copied().unwrap_or(clan.members[0]);
        clan.leader = next;
        clan.officers.remove(&next);
    }
    // Requests from someone leaving the game go too.
    for c in clans.clans.values_mut() {
        c.requests.retain(|&r| r != id);
    }
    send_clan_state(world, None);
}

/// Handles a clan action from a player.
#[allow(
    clippy::too_many_lines,
    reason = "one step after another; splitting it hides the order"
)]
pub fn clan_action(world: &mut World, id: u32, msg: &ClanMessage) {
    let mine = clan_of(world, id);
    let (role_ok, is_leader) = mine
        .and_then(|c| world.resource::<Clans>().clans.get(&c).cloned())
        .map_or((false, false), |c| {
            (c.leader == id || c.officers.contains(&id), c.leader == id)
        });

    let notice = match msg.action {
        // Create.
        0 => {
            let name: String = msg.name.trim().chars().take(MAX_CLAN_NAME).collect();
            if mine.is_some() {
                Some("Leave your clan first")
            } else if name.is_empty() {
                Some("Clan names can't be empty")
            } else if world
                .resource::<Clans>()
                .clans
                .values()
                .any(|c| c.name.eq_ignore_ascii_case(&name))
            {
                Some("That clan name is taken")
            } else {
                let mut clans = world.resource_mut::<Clans>();
                clans.next_id += 1;
                let clan_id = clans.next_id;
                clans.clans.insert(
                    clan_id,
                    Clan {
                        id: clan_id,
                        name,
                        leader: id,
                        members: vec![id],
                        ..Clan::default()
                    },
                );
                set_clan(world, id, Some(clan_id));
                None
            }
        }
        // Request to join.
        1 => {
            if mine.is_some() {
                Some("Leave your clan first")
            } else if let Some(clan) = world.resource_mut::<Clans>().clans.get_mut(&msg.target) {
                if !clan.requests.contains(&id) {
                    clan.requests.push(id);
                }
                None
            } else {
                Some("That clan doesn't exist")
            }
        }
        // Leave.
        2 => {
            leave_clan(world, id);
            None
        }
        // Accept / decline.
        3 | 4 => {
            let target = msg.target;
            let accepted = msg.action == 3 && clan_of(world, target).is_none();
            if let (true, Some(clan_id)) = (role_ok, mine) {
                if let Some(clan) = world.resource_mut::<Clans>().clans.get_mut(&clan_id) {
                    if clan.requests.contains(&target) {
                        clan.requests.retain(|&r| r != target);
                        if accepted {
                            clan.members.push(target);
                        }
                    }
                }
                if accepted {
                    set_clan(world, target, Some(clan_id));
                    for c in world.resource_mut::<Clans>().clans.values_mut() {
                        c.requests.retain(|&r| r != target);
                    }
                }
                None
            } else {
                Some("Only leaders and officers can answer requests")
            }
        }
        // Kick.
        5 => {
            let target = msg.target;
            let target_is_officer = mine
                .and_then(|c| {
                    world
                        .resource::<Clans>()
                        .clans
                        .get(&c)
                        .map(|c| c.officers.contains(&target))
                })
                .unwrap_or(false);
            if role_ok && target != id && clan_of(world, target) == mine && (is_leader || !target_is_officer) {
                leave_clan(world, target);
                world
                    .resource_mut::<Outbox>()
                    .notice(target, "You were kicked from your clan", 1);
                None
            } else {
                Some("You can't kick that player")
            }
        }
        // Promote / demote.
        6 | 7 => {
            if let (true, Some(clan)) = (
                is_leader && clan_of(world, msg.target) == mine && msg.target != id,
                mine,
            ) {
                promote(world, clan, msg.target, msg.action == 6);
                None
            } else {
                Some("Only the leader can change roles")
            }
        }
        _ => None,
    };

    if let Some(text) = notice {
        world.resource_mut::<Outbox>().notice(id, text, 1);
    }
    send_clan_state(world, None);
}

/// Makes a member an officer (or a member again).
fn promote(world: &mut World, clan: u32, target: u32, up: bool) {
    if let Some(c) = world.resource_mut::<Clans>().clans.get_mut(&clan) {
        if up {
            c.officers.insert(target);
        } else {
            c.officers.remove(&target);
        }
    }
}

/// Gives resources to a clan member.
pub fn trade(world: &mut World, from: u32, msg: &TradeMessage) {
    if !are_allies(world, from, msg.to) {
        world
            .resource_mut::<Outbox>()
            .notice(from, "You can only trade with clan members", 1);
        return;
    }
    let (Some(giver), Some(taker)) = (player_entity(world, from), player_entity(world, msg.to)) else {
        return;
    };
    let Some(have) = world.get::<Resources>(giver).copied() else {
        return;
    };
    if have.wood < msg.wood || have.stone < msg.stone || have.food < msg.food || have.gold < msg.gold {
        world
            .resource_mut::<Outbox>()
            .notice(from, "You don't have that much", 1);
        return;
    }
    if let Some(mut r) = world.get_mut::<Resources>(giver) {
        r.wood -= msg.wood;
        r.stone -= msg.stone;
        r.food -= msg.food;
        r.gold -= msg.gold;
    }
    if let Some(mut r) = world.get_mut::<Resources>(taker) {
        r.wood += msg.wood;
        r.stone += msg.stone;
        r.food += msg.food;
        r.gold += msg.gold;
    }
    {
        let mut dirty = world.resource_mut::<Dirty>();
        dirty.resources.insert(giver);
        dirty.resources.insert(taker);
    }
    let giver_name = session_name(world, from);
    let mut out = world.resource_mut::<Outbox>();
    out.notice(from, "Trade sent", 2);
    out.notice(
        msg.to,
        format!(
            "{giver_name} gave you {} wood, {} stone, {} food, {} gold",
            msg.wood, msg.stone, msg.food, msg.gold
        ),
        2,
    );
}

// ----------------------------------------------------------------- shop --

/// Buys or equips a hat, accessory or skin.
pub fn shop_action(world: &mut World, entity: Entity, msg: &ShopMessage) {
    let Some(id) = world.get::<PlayerEntity>(entity).map(|p| p.0) else {
        return;
    };
    let Some(mut gear) = world.get::<Gear>(entity).cloned() else {
        return;
    };

    let result: Result<(), &str> = match (msg.kind, msg.buy) {
        // Hats and accessories.
        (0 | 1, true) => {
            let def = if msg.kind == 0 { hat(msg.id) } else { accessory(msg.id) };
            let owned = if msg.kind == 0 {
                &mut gear.hats
            } else {
                &mut gear.accessories
            };
            match def {
                None => Err("Unknown item"),
                Some(_) if owned.contains(&msg.id) => Err("You already own that"),
                Some(def) => {
                    let gold = world.get::<Resources>(entity).map_or(0, |r| r.gold);
                    if gold < def.price {
                        Err("Not enough gold")
                    } else {
                        if let Some(mut r) = world.get_mut::<Resources>(entity) {
                            r.gold -= def.price;
                        }
                        world.resource_mut::<Dirty>().resources.insert(entity);
                        owned.push(msg.id);
                        world
                            .resource_mut::<Outbox>()
                            .notice(id, format!("Bought {}", def.name), 2);
                        Ok(())
                    }
                }
            }
        }
        (0, false) => {
            if msg.id == 0 || gear.hats.contains(&msg.id) {
                if note_hat_switch(world, id) {
                    return;
                }
                gear.hat = msg.id;
                Ok(())
            } else {
                Err("You don't own that hat")
            }
        }
        (1, false) => {
            if msg.id == 0 || gear.accessories.contains(&msg.id) {
                gear.accessory = msg.id;
                Ok(())
            } else {
                Err("You don't own that accessory")
            }
        }
        // Skins live on the account.
        (2, buy) => {
            let token = world.get::<Account>(entity).map_or_default(|a| a.0.clone());
            let mut accounts = world.resource_mut::<Accounts>();
            let ok = if buy {
                accounts.buy_skin(&token, msg.id)
            } else {
                accounts.select_skin(&token, msg.id)
            };
            if ok {
                gear.skin = msg.id;
                Ok(())
            } else if buy {
                Err("Not enough coins (earn them by playing)")
            } else {
                Err("You don't own that skin")
            }
        }
        _ => Err("Unknown shop action"),
    };

    match result {
        Ok(()) => {
            // Bought hats stay with the session across deaths.
            if let Some(s) = world.resource_mut::<Sessions>().0.get_mut(&id) {
                s.gear = gear.clone();
            }
            // Hats change reload time.
            let weapon = world
                .get::<crate::structs::components::Weapon>(entity)
                .map_or(0, |w| w.0);
            let reload = crate::systems::progression_system::reload_ms(weapon, gear.hat);
            if let Some(mut r) = world.get_mut::<crate::structs::components::ReloadState>(entity) {
                r.1 = reload;
            }
            world.entity_mut(entity).insert(gear);
            send_shop(world, entity);
        }
        Err(text) => world.resource_mut::<Outbox>().notice(id, text, 1),
    }
}

// --------------------------------------------------------------- quests --

/// What a quest asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestKind {
    /// Gather resources.
    Gather,
    /// Kill animals.
    KillAnimals,
    /// Kill players.
    KillPlayers,
    /// Open crates.
    OpenCrates,
    /// Place buildings.
    PlaceBuildings,
}

/// A player's current quest.
#[derive(Component, Clone, Copy, Debug)]
pub struct Quest {
    /// What to do.
    pub kind:     QuestKind,
    /// How much so far.
    pub progress: u32,
    /// How much is needed.
    pub goal:     u32,
    /// Gold reward.
    pub reward:   u32,
}

impl Quest {
    /// Description shown to the player.
    fn text(&self) -> String {
        match self.kind {
            QuestKind::Gather => format!("Gather {} resources", self.goal),
            QuestKind::KillAnimals => format!("Hunt {} animals", self.goal),
            QuestKind::KillPlayers => format!("Defeat {} players", self.goal),
            QuestKind::OpenCrates => format!("Open {} crates", self.goal),
            QuestKind::PlaceBuildings => format!("Place {} buildings", self.goal),
        }
    }
}

/// Gives a player a new random quest, scaled to their age.
pub fn give_quest(world: &mut World, entity: Entity) {
    let age = world.get::<Progress>(entity).map_or(1, |p| p.age);
    let mut rng = rng();
    let scale = 1 + age / 3;
    let (kind, goal) = match rng.generate_range(0_u8..5) {
        0 => (QuestKind::Gather, 100 * scale),
        1 => (QuestKind::KillAnimals, 2 + scale),
        2 => (QuestKind::KillPlayers, 1 + scale / 2),
        3 => (QuestKind::OpenCrates, 1 + scale / 2),
        _ => (QuestKind::PlaceBuildings, 5 + 2 * scale),
    };
    let reward = match kind {
        QuestKind::Gather => goal * 2,
        QuestKind::KillAnimals => goal * 40,
        QuestKind::KillPlayers => goal * 250,
        QuestKind::OpenCrates => goal * 120,
        QuestKind::PlaceBuildings => goal * 15,
    };
    let quest = Quest {
        kind,
        progress: 0,
        goal,
        reward,
    };
    world.entity_mut(entity).insert(quest);
    send_quest(world, entity);
}

/// Sends a player their quest.
fn send_quest(world: &mut World, entity: Entity) {
    let (Some(id), Some(q)) = (
        world.get::<PlayerEntity>(entity).map(|p| p.0),
        world.get::<Quest>(entity).copied(),
    ) else {
        return;
    };
    world.resource_mut::<Outbox>().send(
        Target::Player(id),
        PacketType::Quest,
        &QuestData {
            text:     q.text(),
            progress: q.progress,
            goal:     q.goal,
            reward:   q.reward,
        },
    );
}

/// Records progress on a player's quest, rewarding them when it's done.
pub fn quest_progress(world: &mut World, entity: Entity, kind: QuestKind, amount: u32) {
    let Some(mut quest) = world.get_mut::<Quest>(entity) else {
        return;
    };
    if quest.kind != kind {
        return;
    }
    quest.progress = (quest.progress + amount).min(quest.goal);
    let q = *quest;
    if q.progress < q.goal {
        send_quest(world, entity);
        return;
    }
    if let Some(mut r) = world.get_mut::<Resources>(entity) {
        r.gold += q.reward;
    }
    world.resource_mut::<Dirty>().resources.insert(entity);
    grant_xp(world, entity, q.reward / 2);
    if let Some(id) = world.get::<PlayerEntity>(entity).map(|p| p.0) {
        world
            .resource_mut::<Outbox>()
            .notice(id, format!("Quest complete! +{} gold", q.reward), 2);
    }
    give_quest(world, entity);
}

// --------------------------------------------------------------- events --

/// A world event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// Gathering gives double.
    GoldRush,
    /// A boss appears.
    BossHunt,
    /// High-tier crates drop.
    SupplyDrop,
    /// Lots of animals appear.
    Stampede,
}

impl EventKind {
    /// Name and description.
    const fn text(self) -> (&'static str, &'static str) {
        match self {
            Self::GoldRush => ("Gold Rush", "Gathering gives double resources"),
            Self::BossHunt => ("Boss Hunt", "A boss is roaming the land. Defeat it for loot!"),
            Self::SupplyDrop => ("Supply Drop", "Rare crates have dropped around the map"),
            Self::Stampede => ("Stampede", "Herds of animals are roaming the land"),
        }
    }

    /// Seconds it lasts.
    const fn seconds(self) -> u32 {
        match self {
            Self::GoldRush => 90,
            Self::BossHunt => 300,
            Self::SupplyDrop | Self::Stampede => 120,
        }
    }
}

/// The world event state.
#[derive(Resource)]
pub struct WorldEvents {
    /// The running event and the tick it ends on.
    pub current:    Option<(EventKind, u64)>,
    /// Tick the next event starts on.
    pub next_start: u64,
}

impl Default for WorldEvents {
    fn default() -> Self {
        Self {
            current:    None,
            // First event after about two minutes.
            next_start: u64::from(120_000 / TICK_MS),
        }
    }
}

/// Resource multiplier from events.
pub fn gather_multiplier(world: &World) -> u32 {
    match world.resource::<WorldEvents>().current {
        Some((EventKind::GoldRush, _)) => 2,
        _ => 1,
    }
}

/// Starts an event now.
pub fn start_event(world: &mut World, kind: EventKind) {
    let tick = world.resource::<TickCount>().0;
    let ends = tick + u64::from(kind.seconds() * 1000 / TICK_MS);
    world.resource_mut::<WorldEvents>().current = Some((kind, ends));

    match kind {
        EventKind::BossHunt => {
            let ids: Vec<u8> = bosses().collect();
            let boss = ids[rng().generate_range(0..ids.len())];
            let spot = random_land_spot(world, 150.0);
            spawn_animal(world, boss, Some(spot));
        }
        EventKind::SupplyDrop => {
            for i in 0..6 {
                spawn_crate(world, Some(if i < 2 { 4 } else { 3 }));
            }
        }
        EventKind::Stampede => {
            for _ in 0..30 {
                let id = rng().generate_range(2_u8..=8);
                spawn_animal(world, id, None);
            }
        }
        EventKind::GoldRush => {}
    }
    let (name, desc) = kind.text();
    world.resource_mut::<Outbox>().send(
        Target::All,
        PacketType::Notice,
        &shared::to_client::NoticeData {
            text: format!("{name}! {desc}"),
            kind: 2,
        },
    );
}

/// Starts and ends world events.
fn tick_events(world: &mut World, tick: u64) {
    let state = world.resource::<WorldEvents>();
    if let Some((_, ends)) = state.current {
        if tick >= ends {
            let mut events = world.resource_mut::<WorldEvents>();
            events.current = None;
            // Next one in four to six minutes.
            events.next_start = tick + u64::from(rng().generate_range(240_000..360_000) / TICK_MS);
        }
    } else if tick >= state.next_start {
        let kind = match rng().generate_range(0_u8..4) {
            0 => EventKind::GoldRush,
            1 => EventKind::BossHunt,
            2 => EventKind::SupplyDrop,
            _ => EventKind::Stampede,
        };
        start_event(world, kind);
    }

    let tick_now = tick;
    let data = world
        .resource::<WorldEvents>()
        .current
        .map_or_else(WorldEventData::default, |(kind, ends)| {
            let (name, description) = kind.text();
            WorldEventData {
                name:         name.into(),
                description:  description.into(),
                seconds_left: u32::try_from(ends.saturating_sub(tick_now) * u64::from(TICK_MS) / 1000).unwrap_or(0),
            }
        });
    world
        .resource_mut::<Outbox>()
        .send(Target::All, PacketType::WorldEvent, &data);
}

// ------------------------------------------------- leaderboard & minimap --

/// Sends the leaderboard to everyone.
fn send_leaderboard(world: &mut World) {
    let mut rows: Vec<(LeaderEntry, u32)> = world
        .query::<(&PlayerEntity, &Name, &Resources, &PlayerStats, &Progress)>()
        .iter(world)
        .map(|(p, n, r, s, prog)| {
            (
                LeaderEntry {
                    id:    p.0,
                    name:  n.0.clone(),
                    score: r.gold,
                    kills: s.kills,
                },
                prog.age,
            )
        })
        .collect();
    let online = u32::try_from(rows.len()).unwrap_or(u32::MAX);
    let skull = rows
        .iter()
        .filter(|r| r.0.kills > 0)
        .max_by_key(|r| r.0.kills)
        .map(|r| r.0.id);
    let star = rows.iter().filter(|r| r.1 > 1).max_by_key(|r| r.1).map(|r| r.0.id);
    rows.sort_by_key(|r| std::cmp::Reverse(r.0.score));
    let crown = rows.first().filter(|r| r.0.score > 0).map(|r| r.0.id);
    rows.truncate(10);
    world.resource_mut::<Outbox>().send(
        Target::All,
        PacketType::Leaderboard,
        &LeaderboardData {
            entries: rows.into_iter().map(|r| r.0).collect(),
            crown,
            skull,
            star,
            online,
        },
    );
}

/// Sends each player their minimap points: clan members and bosses.
fn send_minimap(world: &mut World) {
    let players: Vec<(u32, Position)> = world
        .query::<(&PlayerEntity, &Position)>()
        .iter(world)
        .map(|(p, pos)| (p.0, *pos))
        .collect();
    let mut common: Vec<MinimapPoint> = world
        .query::<(&AnimalEntity, &Position)>()
        .iter(world)
        .filter(|(a, _)| animal(a.0).is_some_and(|d| d.boss))
        .map(|(_, p)| MinimapPoint {
            x:    p.0,
            y:    p.1,
            kind: 1,
        })
        .collect();
    common.extend(
        world
            .query::<(&ObjectKind, &Position)>()
            .iter(world)
            .filter(|(k, _)| matches!(k, ObjectKind::Crate(4)))
            .map(|(_, p)| MinimapPoint {
                x:    p.0,
                y:    p.1,
                kind: 2,
            }),
    );

    for &(id, _) in &players {
        let mut points = common.clone();
        points.extend(
            players
                .iter()
                .filter(|(other, _)| are_allies(world, id, *other))
                .map(|(_, p)| MinimapPoint {
                    x:    p.0,
                    y:    p.1,
                    kind: 0,
                }),
        );
        world
            .resource_mut::<Outbox>()
            .send(Target::Player(id), PacketType::Minimap, &MinimapData { points });
    }
}

/// Once a second: events, leaderboard and minimap.
pub fn social_tick_system(world: &mut World) {
    let tick = world.resource::<TickCount>().0;
    if !tick.is_multiple_of(u64::from(1000 / TICK_MS)) {
        return;
    }
    tick_events(world, tick);
    send_leaderboard(world);
    send_minimap(world);
}
