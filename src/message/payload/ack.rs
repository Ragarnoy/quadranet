use crate::device::Uid;
use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Format)]
pub enum AckType {
    Success { message_id: u32 },
    AckDiscovered { hops: u8, last_hop: Uid },
    Failure { message_id: u32 },
}
