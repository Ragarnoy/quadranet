use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub struct DeviceConfig {
    pub device_class: DeviceClass,
    pub device_capabilities: DeviceCapabilities,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            device_class: DeviceClass::A,
            device_capabilities: DeviceCapabilities::Lora,
        }
    }
}

impl From<DeviceConfig> for u8 {
    fn from(value: DeviceConfig) -> Self {
        match (value.device_class, value.device_capabilities) {
            (DeviceClass::A, DeviceCapabilities::Lora) => 0,
            (DeviceClass::A, DeviceCapabilities::LoraBle) => 1,
            (DeviceClass::A, DeviceCapabilities::LoraWifi) => 2,
            (DeviceClass::B, DeviceCapabilities::Lora) => 3,
            (DeviceClass::B, DeviceCapabilities::LoraBle) => 4,
            (DeviceClass::B, DeviceCapabilities::LoraWifi) => 5,
            (DeviceClass::C, DeviceCapabilities::Lora) => 6,
            (DeviceClass::C, DeviceCapabilities::LoraBle) => 7,
            (DeviceClass::C, DeviceCapabilities::LoraWifi) => 8,
        }
    }
}

impl From<u8> for DeviceConfig {
    fn from(value: u8) -> Self {
        match value {
            1 => Self {
                device_class: DeviceClass::A,
                device_capabilities: DeviceCapabilities::LoraBle,
            },
            2 => Self {
                device_class: DeviceClass::A,
                device_capabilities: DeviceCapabilities::LoraWifi,
            },
            3 => Self {
                device_class: DeviceClass::B,
                device_capabilities: DeviceCapabilities::Lora,
            },
            4 => Self {
                device_class: DeviceClass::B,
                device_capabilities: DeviceCapabilities::LoraBle,
            },
            5 => Self {
                device_class: DeviceClass::B,
                device_capabilities: DeviceCapabilities::LoraWifi,
            },
            6 => Self {
                device_class: DeviceClass::C,
                device_capabilities: DeviceCapabilities::Lora,
            },
            7 => Self {
                device_class: DeviceClass::C,
                device_capabilities: DeviceCapabilities::LoraBle,
            },
            8 => Self {
                device_class: DeviceClass::C,
                device_capabilities: DeviceCapabilities::LoraWifi,
            },
            _ => Self {
                device_class: DeviceClass::A,
                device_capabilities: DeviceCapabilities::Lora,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum DeviceClass {
    A,
    B,
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Format, Deserialize, Serialize)]
pub enum DeviceCapabilities {
    Lora,
    LoraBle,
    LoraWifi,
}
