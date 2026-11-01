//! Player health updates.

use shared::{to_client::UpdateHealthData, PacketType};

use crate::structs::outbox::{Outbox, Target};

/// Queues a health update for everyone near the player.
pub fn send_health(out: &mut Outbox, id: u32, (x, y): (f32, f32), health: f32, max_health: f32) {
    out.send(
        Target::Near(x, y),
        PacketType::UpdateHealth,
        &UpdateHealthData {
            id,
            new_health: health,
            max_health,
        },
    );
}
