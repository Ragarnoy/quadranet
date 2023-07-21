use lora_phy::LoRa;

pub struct Device {
    uid: u16,
    radio: LoRa<RK, DLY>,
}