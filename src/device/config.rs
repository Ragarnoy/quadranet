use lora_phy::mod_params::{ModulationParams, PacketParams};

pub struct LoraConfig {
    pub frequency: u32,
    pub tx_power: i32,
    pub modulation: ModulationParams,
    pub rx_pkt_params: PacketParams,
    pub tx_pkt_params: PacketParams,
    pub boosted: bool,
}

impl LoraConfig {
    pub fn new(frequency: u32, tx_power: i32, modulation: ModulationParams, rx_pkt_params: PacketParams, tx_pkt_params: PacketParams) -> Self {
        Self {
            frequency,
            tx_power,
            modulation,
            rx_pkt_params,
            tx_pkt_params,
            boosted: false,
        }
    }
}
