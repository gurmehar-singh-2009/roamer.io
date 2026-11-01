//! Messages queued by systems during a tick and sent by the game loop
//! afterwards.

use bevy_ecs::resource::Resource;
use shared::PacketType;

/// Who a message goes to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// One player.
    Player(u32),
    /// Everyone.
    All,
    /// Everyone except one player.
    AllExcept(u32),
    /// Everyone whose view contains this point.
    Near(f32, f32),
    /// Like `Near`, but only players on one layer (true = underground).
    NearLayer(f32, f32, bool),
    /// Everyone who has been sent this object (entity index).
    KnowsObject(u32),
    /// Like `KnowsObject`, and they forget it afterwards (object removed).
    ForgetObject(u32),
}

/// A queued message.
#[derive(Clone, Debug)]
pub struct Outgoing {
    /// Recipients.
    pub target:   Target,
    /// Encoded packet (opcode + body).
    pub bytes:    Vec<u8>,
    /// Send over a reliable stream instead of a datagram.
    pub reliable: bool,
}

/// Messages waiting to be sent.
#[derive(Resource, Default)]
pub struct Outbox(pub Vec<Outgoing>);

impl Outbox {
    /// Queues a message as a datagram.
    pub fn send<T: borsh::BorshSerialize>(&mut self, target: Target, opcode: PacketType, data: &T) {
        self.push(target, opcode, data, false);
    }

    /// Queues a message on a reliable stream (for things that must arrive).
    pub fn send_reliable<T: borsh::BorshSerialize>(&mut self, target: Target, opcode: PacketType, data: &T) {
        self.push(target, opcode, data, true);
    }

    /// Queues an already encoded message.
    pub fn send_raw(&mut self, target: Target, bytes: Vec<u8>, reliable: bool) {
        self.0.push(Outgoing {
            target,
            bytes,
            reliable,
        });
    }

    /// Encodes and queues a message.
    fn push<T: borsh::BorshSerialize>(&mut self, target: Target, opcode: PacketType, data: &T, reliable: bool) {
        let mut bytes = vec![opcode as u8];
        if borsh::to_writer(&mut bytes, data).is_ok() {
            self.0.push(Outgoing {
                target,
                bytes,
                reliable,
            });
        }
    }

    /// Queues a notice for one player.
    pub fn notice(&mut self, player: u32, text: impl Into<String>, kind: u8) {
        self.send(
            Target::Player(player),
            PacketType::Notice,
            &shared::to_client::NoticeData {
                text: text.into(),
                kind,
            },
        );
    }
}
