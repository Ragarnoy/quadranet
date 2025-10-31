use crate::device::config::device::DeviceCapabilities;
#[cfg(feature = "defmt")]
use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub struct DiscoveryType {
    pub original_ttl: u8,
    pub sender_capabilities: DeviceCapabilities,
}
