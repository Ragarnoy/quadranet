use defmt::Format;
use serde::{Deserialize, Serialize};
use crate::device::Uid;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Format)]
pub enum AckType {
    Success {
        message_id: u32,
    },
    AckDiscovered {
        hops: u8,
        last_hop: Uid,
    },
    Failure {
        message_id: u32,
    },
}
