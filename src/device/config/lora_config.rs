use lora_phy::mod_params::{Bandwidth, CodingRate, ModulationParams, PacketParams, RadioError, SpreadingFactor};
use lora_phy::LoRa;
use lora_phy::mod_traits::RadioKind;
use embedded_hal_async::delay::DelayUs;

pub const LORA_FREQUENCY_IN_HZ: u32 = 433_220_000;
const TX_POWER: i32 = 20;

pub struct LoraConfig {
    pub tx_power: i32,
    pub modulation: ModulationParams,
    pub rx_pkt_params: PacketParams,
    pub tx_pkt_params: PacketParams,
    pub boosted: bool,
}

impl LoraConfig {
    pub fn new<RK, DLY>(lora: &mut LoRa<RK, DLY>) -> Self
    where
        RK: RadioKind,
        DLY: DelayUs,
    {
        let modulation = modulation_params(lora).expect("Failed to create modulation params");

        let tx_pkt_params =
            create_tx_packet(lora, &modulation).expect("Failed to create TX packet params");

        let rx_pkt_params =
            create_rx_packet(lora, &modulation).expect("Failed to create RX packet params");

        Self {
            tx_power: TX_POWER,
            modulation,
            rx_pkt_params,
            tx_pkt_params,
            boosted: false,
        }
    }
}


fn modulation_params<RK, DLY>(lora: &mut LoRa<RK, DLY>) -> Result<ModulationParams, RadioError>
where
    RK: RadioKind,
    DLY: DelayUs,
{
    lora.create_modulation_params(
        SpreadingFactor::_10,
        Bandwidth::_125KHz,
        CodingRate::_4_8,
        LORA_FREQUENCY_IN_HZ,
    )
}

fn create_rx_packet<RK, DLY>(
    lora: &mut LoRa<RK, DLY>,
    mdltn_params: &ModulationParams,
) -> Result<PacketParams, RadioError>
where
    RK: RadioKind,
    DLY: DelayUs,
{
    lora.create_rx_packet_params(8, false, 255, true, false, mdltn_params)
}

fn create_tx_packet<RK, DLY>(
    lora: &mut LoRa<RK, DLY>,
    mdltn_params: &ModulationParams,
) -> Result<PacketParams, RadioError>
where
    RK: RadioKind,
    DLY: DelayUs,
{
    lora.create_tx_packet_params(8, false, true, false, mdltn_params)
}
