
use core::num::NonZeroU8;
use defmt::{error, info, warn};
use heapless::Vec;
use lora_phy::mod_params::RadioError;
use lora_phy::mod_traits::RadioKind;
use lora_phy::LoRa;
use embassy_time::{Duration, Timer};
use embedded_hal_async::delay::DelayUs;
use crate::device::config::LoraConfig;
use crate::device::device_error::DeviceError;
use crate::message::intent::Intent;
use crate::message::Message;
use crate::route::Route;
use crate::route::routing_table::RoutingTable;

pub mod config;
pub mod device_error;

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

    pub fn uid(&self) -> u8 {
        self.uid.get()
    }

    pub fn receive_message(&mut self, message: Message) {
        let route = Route {
            next_hop: message.sender_uid,  // The UID of the node that sent the message
            // ... other possible fields like cost, hop_count, etc.
        };
        self.routing_table.update(message.sender_uid.get().into(), route);

        if message.receiver_uid.unwrap().get() != self.uid.get() {
            self.outstack.push(message).unwrap(); // Handle this unwrap appropriately
        }
        else {
            self.instack.push(message).unwrap(); // Handle this unwrap appropriately
        }
    }

    pub async fn process_instack(&mut self) -> Result<(), RadioError> {
        let to_process = core::cmp::min(self.instack.len(), MAX_INSTACK_PROCESS);
        for _ in 0..to_process {
            let message = self.instack.pop().unwrap(); // Handle this unwrap appropriately
            if let Some(new_message) = self.process_message(message).await {
                self.outstack.push(new_message).unwrap(); // Handle this unwrap appropriately
            }
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

    pub async fn process_message(&mut self, message: Message) -> Option<Message> {
        // Your existing logic for processing messages
        match message.intent {
            Intent::Ping => {
                let pong_message = Message::pong(self.uid, message.sender_uid);
                Some(pong_message)
            },
            Intent::Data => {
                info!("Received data: {:?}", message);
                None
            },
            Intent::Discover => {
                let depth = message.content[0];
                if depth > 0 {
                    Some(Message::discover(self.uid, depth - 1))
                }
                else {
                    None
                }
            },
            _ => None
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
        if let Some(route) = self.routing_table.lookup_route(message.receiver_uid.unwrap().get()) {
            message.next_hop = Some(route.next_hop);
        } else {
            // Handle the case where the route is not found
            warn!("Route not found");
        }
        self.state = DeviceState::Transmitting;
        Timer::after(Duration::from_millis(200)).await;
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

    pub async fn discover_nodes(&mut self, depth: u8) -> Result<(), RadioError> {
        if depth == 0 {
            return Ok(());
        }
        let message = Message::discover(self.uid, depth - 1); // Decrement depth
        self.send_message(message).await
    }

    pub async fn listen_for_messages(&mut self, buf: &mut [u8]) -> Result<(), DeviceError> {
        loop {
            let (rx_length, _packet_status) = self.radio
                .rx(&self.config.rx_pkt_params, buf)
                .await?;

            let received_message = Message::try_from(&buf[0..rx_length as usize]).map_err(|source| DeviceError::MessageError { source })?;

            self.receive_message(received_message);
        }
    }
}

pub async fn run_device<RK, DLY>(device: &mut LoraDevice<RK, DLY>, buf: &mut [u8])
    where
        RK: RadioKind,
        DLY: DelayUs,
{
    loop {
        // Listen for incoming messages
        if let Ok((rx_length, _packet_status)) = device.radio.rx(&device.config.rx_pkt_params, buf).await {
            let received_message = Message::try_from(&buf[0..rx_length as usize]).unwrap(); // Handle unwrap appropriately
            info!("Received message: {:?}", received_message);
            device.receive_message(received_message);
        }

        // Process InStack
        if let Err(e) = device.process_instack().await {
            error!("Error processing instack: {:?}", e);
        }

        // Process OutStack
        if let Err(e) = device.process_outstack().await {
            error!("Error processing outstack: {:?}", e);
        }

        // Add a delay or yield the task to prevent it from hogging the CPU
        // For example, using embassy_time's Timer:
        Timer::after(Duration::from_millis(10)).await;
    }
}
