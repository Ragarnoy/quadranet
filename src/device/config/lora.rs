use embedded_hal_async::delay::DelayNs;
use lora_phy::mod_params::{
    Bandwidth, CodingRate, ModulationParams, PacketParams, RadioError, SpreadingFactor,
};
use lora_phy::mod_traits::RadioKind;
use lora_phy::LoRa;

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
    /// Creates a new `LoRa` configuration.
    ///
    /// # Errors
    ///
    /// Returns a `RadioError` if any of the underlying configuration operations fail.
    pub fn new<RK, DLY>(lora: &mut LoRa<RK, DLY>) -> Result<Self, RadioError>
    where
        RK: RadioKind,
        DLY: DelayNs,
    {
        let modulation = modulation_params(lora)?;
        let tx_pkt_params = create_tx_packet(lora, &modulation)?;
        let rx_pkt_params = create_rx_packet(lora, &modulation)?;

        Ok(Self {
            tx_power: TX_POWER,
            modulation,
            rx_pkt_params,
            tx_pkt_params,
            boosted: false,
        })
    }
}

fn modulation_params<RK, DLY>(lora: &mut LoRa<RK, DLY>) -> Result<ModulationParams, RadioError>
where
    RK: RadioKind,
    DLY: DelayNs,
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
    DLY: DelayNs,
{
    lora.create_rx_packet_params(8, false, 255, true, false, mdltn_params)
}

fn create_tx_packet<RK, DLY>(
    lora: &mut LoRa<RK, DLY>,
    mdltn_params: &ModulationParams,
) -> Result<PacketParams, RadioError>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    lora.create_tx_packet_params(8, false, true, false, mdltn_params)
}