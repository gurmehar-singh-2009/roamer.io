use snafu::ResultExt;

use crate::errors::{DecryptionSnafu, EncryptionSnafu, GameError};

pub fn decode<T: borsh::BorshDeserialize>(buf: &[u8]) -> Result<T, GameError> {
    ResultExt::context(borsh::from_slice::<T>(buf), DecryptionSnafu)
}

pub fn encode<T: borsh::BorshSerialize>(opcode: u8, data: T) -> Result<Vec<u8>, GameError> {
    let mut v = vec![opcode];

    v.extend(ResultExt::context(borsh::to_vec(&data), EncryptionSnafu)?);

    Ok(v)
}
