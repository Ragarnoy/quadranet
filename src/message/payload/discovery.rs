use defmt::Format;
use serde::{Deserialize, Serialize};
use crate::device::config::device_config::DeviceCapabilities;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Format)]
pub struct DiscoveryType {
    pub original_ttl: u8,
    pub sender_capabilities: DeviceCapabilities,
}