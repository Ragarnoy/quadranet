#[cfg(feature = "defmt")]
use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub enum CommandType {
    SetConfig,
}
