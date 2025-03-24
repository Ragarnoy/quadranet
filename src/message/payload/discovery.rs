use crate::device::config::device::DeviceCapabilities;
use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Format)]
pub struct DiscoveryType {
    pub original_ttl: u8,
    pub sender_capabilities: DeviceCapabilities,
}
