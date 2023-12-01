use crate::device::Uid;
use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Format)]
pub enum DiscoveryType {
    Request { original_ttl: u8 },
    Response { hops: u8, last_hop: Uid },
}
