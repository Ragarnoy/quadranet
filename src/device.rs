pub mod config;
pub mod outstack;

use core::num::NonZeroU8;
use defmt::Format;
use crate::message::Message;
use config::LoraConfig;
use embassy_time::{Duration, Timer};
use embedded_hal_async::delay::DelayUs;
use lora_phy::mod_params::{PacketStatus, RadioError};
use lora_phy::mod_traits::RadioKind;
use lora_phy::LoRa;
use snafu::Snafu;

pub type Uid = NonZeroU8;

pub struct LoraDevice<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayUs,
{
    uid: Uid,
    radio: LoRa<RK, DLY>,
    config: LoraConfig,
    state: DeviceState,
}

pub enum DeviceState {
    Idle,
    Transmitting,
    Receiving,
}

impl<RK, DLY> LoraDevice<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayUs,
{
    pub fn new(uid: Uid, radio: LoRa<RK, DLY>, config: LoraConfig) -> Self {
        Self {
            uid,
            radio,
            config,
            state: DeviceState::Idle,
        }
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub async fn send_message(&mut self, mut message: Message) -> Result<(), RadioError> {
        self.radio
            .prepare_for_tx(
                &self.config.modulation,
                self.config.tx_power,
                self.config.boosted,
            )
            .await?;
        self.state = DeviceState::Transmitting;

        Timer::after(Duration::from_millis(300)).await;

        message.sender_uid = self.uid;
        let buffer: [u8; 70] = message.into();
        match self
            .radio
            .tx(
                &self.config.modulation,
                &mut self.config.tx_pkt_params,
                &buffer,
                0xffffff,
            )
            .await
        {
            Ok(()) => {
                self.state = DeviceState::Idle;
            }
            Err(err) => {
                return Err(err);
            }
        };
        Ok(())
    }

    pub async fn receive_message(
        &mut self,
        buf: &mut [u8],
    ) -> Result<(u8, PacketStatus), DeviceError> {
        self.radio
            .prepare_for_rx(
                &self.config.modulation,
                &self.config.rx_pkt_params,
                None,
                None,
                false,
            )
            .await?;
        self.state = DeviceState::Receiving;

        match self.radio.rx(&self.config.rx_pkt_params, buf).await {
            Ok((rx_length, packet_status)) => {
                self.state = DeviceState::Idle;
                Ok((rx_length, packet_status))
            }
            Err(err) => {
                Err(err.into())
            },
        }
    }
}

#[derive(Debug, Snafu, Format)]
pub enum DeviceError {
    #[snafu(display("Radio error: {:?}", error))]
    RadioError { error: RadioError },
}

impl From<RadioError> for DeviceError {
    fn from(error: RadioError) -> Self {
        Self::RadioError { error }
    }
}