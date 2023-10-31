
use core::num::NonZeroU8;
use defmt::Format;
use heapless::Vec;
use lora_phy::mod_params::RadioError;
use lora_phy::mod_traits::RadioKind;
use lora_phy::LoRa;
use embassy_time::{Duration, Timer};
use embedded_hal_async::delay::DelayUs;
use snafu::Snafu;
use crate::device::config::LoraConfig;
use crate::message::{Message, Intent};
use crate::network::routing_table::RoutingTable;

pub mod config;

const INSTACK_SIZE: usize = 32;
const OUTSTACK_SIZE: usize = 32;
const MAX_INSTACK_PROCESS: usize = 5;
const MAX_OUTSTACK_TRANSMIT: usize = 5;

pub type Uid = NonZeroU8;
pub type InStack = Vec<Message, INSTACK_SIZE>;
pub type OutStack = Vec<Message, OUTSTACK_SIZE>;

pub struct LoraDevice<RK, DLY>
    where
        RK: RadioKind,
        DLY: DelayUs,
{
    uid: Uid,
    config: LoraConfig,
    radio: LoRa<RK, DLY>,
    state: DeviceState,
    instack: InStack,
    outstack: OutStack,
    routing_table: RoutingTable,
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
            state: DeviceState::Idle,
            config,
            instack: Vec::new(),
            outstack: Vec::new(),
            routing_table: RoutingTable::default(),
        }
    }

    pub fn receive_message(&mut self, message: Message) {
        self.instack.push(message).unwrap(); // Handle this unwrap appropriately
    }

    pub async fn process_instack(&mut self) -> Result<(), RadioError> {
        let to_process = core::cmp::min(self.instack.len(), MAX_INSTACK_PROCESS);
        for _ in 0..to_process {
            let message = self.instack.pop().unwrap(); // Handle this unwrap appropriately
            let new_message = self.process_message(message).await?;
            self.outstack.push(new_message).unwrap(); // Handle this unwrap appropriately
        }
        Ok(())
    }

    pub async fn process_outstack(&mut self) -> Result<(), RadioError> {
        let to_transmit = core::cmp::min(self.outstack.len(), MAX_OUTSTACK_TRANSMIT);
        for _ in 0..to_transmit {
            let message = self.outstack.pop().unwrap(); // Handle this unwrap appropriately
            self.send_message(message).await?;
        }
        Ok(())
    }

    pub async fn process_message(&mut self, message: Message) -> Result<Message, RadioError> {
        // Your existing logic for processing messages
        match message.intent {
            Intent::Ping => {
                let pong_message = Message::pong(self.uid, message.sender_uid);
                Ok(pong_message)
            },
            Intent::Data => {
                // Your logic for handling Data messages
                Ok(message) // Placeholder
            },
            Intent::Discover => {
                let depth = message.content[0];
                if depth > 0 {
                    self.discover_nodes(depth - 1).await?;
                }
                Ok(message) // Placeholder
            },
            _ => Ok(message) // Placeholder
        }
    }

    pub async fn send_message(&mut self, mut message: Message) -> Result<(), RadioError> {
        // Your existing send_message logic
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
        self.radio
            .tx(
                &self.config.modulation,
                &mut self.config.tx_pkt_params,
                &buffer,
                0xffffff,
            )
            .await?;
        self.state = DeviceState::Idle;
        Ok(())
    }

    pub async fn retransmit_message(&mut self, message: Message) -> Result<(), RadioError> {
        if message.receiver_uid.unwrap().get() != self.uid.get() {
            self.send_message(message).await?;
        }
        Ok(())
    }

    pub async fn discover_nodes(&mut self, depth: u8) -> Result<(), RadioError> {
        if depth == 0 {
            return Ok(());
        }
        let message = Message::discover(self.uid, depth - 1); // Decrement depth
        self.send_message(message).await
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