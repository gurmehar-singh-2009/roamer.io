//! Error types and the messages passed from connection tasks to the game loop.

use shared::to_server::{ArenaMessage, ChestMessage, ClanMessage, ShopMessage, TradeMessage};
use snafu::Snafu;

/// Messages from connection tasks to the game loop.
#[derive(Debug)]
pub enum InternalGameMessages {
    /// The player's connection closed.
    Disconnect,
    /// Spawn (or respawn) the player.
    Spawn {
        /// Display name.
        name:  String,
        /// Account token.
        token: String,
    },
    /// Attack button pressed or released.
    Attack(bool),
    /// A chat message to broadcast.
    ChatMessage(String),
    /// Switch to an owned weapon.
    SetWeapon(u8),
    /// Eat food or place a building.
    UseItem(u8),
    /// Pick an upgrade.
    ChooseUpgrade {
        /// Whether `id` is an item.
        is_item: bool,
        /// Weapon or item id.
        id:      u8,
    },
    /// Shop action.
    Shop(ShopMessage),
    /// Clan action.
    Clan(ClanMessage),
    /// Move resources into or out of the player's chest.
    Chest(ChestMessage),
    /// Go down or up a tunnel entrance.
    Cave,
    /// Give resources to a clan member.
    Trade(TradeMessage),
    /// Arena action.
    Arena(ArenaMessage),
}

/// Errors the server can run into.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum GameError {
    // --- Internal Errors ---
    /// Decoding or decrypting a message failed.
    #[snafu(display("Decryption Failer"))]
    DecryptionError {
        /// Underlying error.
        source: std::io::Error,
    },

    /// An unrecoverable setup step failed.
    #[snafu(display("System Panic"))]
    SystemPanic,

    /// Reading from a stream failed.
    #[snafu(display("Read Slice Error"))]
    ReadSliceError,

    /// Writing to a stream failed.
    #[snafu(display("Write Slice Error"))]
    WriteSliceError,

    /// Encoding or encrypting a message failed.
    #[snafu(display("Encryption Failer"))]
    EncryptionError {
        /// Underlying error.
        source: std::io::Error,
    },

    // --- Client Errors ---
    /// The client connection misbehaved.
    #[snafu(display("Faulty Connection"))]
    FaultyConnection,

    /// The client sent a malformed message.
    #[snafu(display("Faulty Message"))]
    FaultyMessage,

    /// The client's Kyber public key has the wrong size.
    #[snafu(display("Invalid Public Kyber Key Size"))]
    InvalidKyberPublicKeySize,

    /// Deriving the session key failed.
    #[snafu(display("HKDF Expansion Failed"))]
    HKDFExpansionFailure,
}
