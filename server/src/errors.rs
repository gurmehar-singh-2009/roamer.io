use shared::structs::server::{Aim, HitEvent, InternalChatMessage, Move, Player};
use snafu::Snafu;

/// internal game messages used to pass information around
#[derive(Debug)]
pub enum InternalGameMessages {
    Disconnect,
    AddPlayer(Player),
    MovePlayer(Move),
    AimPlayer(Aim),
    PlayerHit(HitEvent),
    ChatMessage(InternalChatMessage),
    SetWeapon(shared::structs::server::ChooseWeapon),
}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum GameError {
    // --- Internal Errors ---
    #[snafu(display("Decryption Failer"))]
    DecryptionError { source: std::io::Error },

    #[snafu(display("System Panic"))]
    SystemPanic,

    #[snafu(display("Read Slice Error"))]
    ReadSliceError,

    #[snafu(display("Write Slice Error"))]
    WriteSliceError,

    #[snafu(display("Encryption Failer"))]
    EncryptionError { source: std::io::Error },

    // --- Client Errors ---
    #[snafu(display("Faulty Connection"))]
    FaultyConnection,

    #[snafu(display("Faulty Message"))]
    FaultyMessage,

    #[snafu(display("Invalid Public Kyber Key Size"))]
    InvalidKyberPublicKeySize,

    #[snafu(display("HKDF Expansion Failed"))]
    HKDFExpansionFailure,
}
