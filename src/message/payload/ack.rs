use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Format)]
pub enum AckType {
    Success {
        message_id: u32,
    },
    Failure {
        message_id: u32,
    },
}
