use snafu::ResultExt;

use crate::errors::{DecryptionSnafu, GameError};

/// Decodes a borsh message body.
pub fn decode<T: borsh::BorshDeserialize>(buf: &[u8]) -> Result<T, GameError> {
    ResultExt::context(borsh::from_slice::<T>(buf), DecryptionSnafu)
}
