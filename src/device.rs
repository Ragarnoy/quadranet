use config::lora_config::LoraConfig;
use crate::device::stacks::MessageStack;
use crate::message::intent::Intent;
use crate::message::Message;
use crate::route::routing_table::RoutingTable;
use crate::route::Route;
use core::num::NonZeroU8;
use defmt::{error, info, warn};
use embassy_time::{Duration, Timer};
use embedded_hal_async::delay::DelayUs;
use heapless::Vec;
use lora_phy::mod_params::RadioError;
use lora_phy::mod_traits::RadioKind;
use lora_phy::LoRa;
use crate::device::config::device_config::{DeviceCapabilities, DeviceClass, DeviceConfig};

pub mod config;
pub mod device_error;
pub mod stacks;

pub static mut DEVICE_CONFIG: DeviceConfig = DeviceConfig { device_class: DeviceClass::A, device_capabilities: DeviceCapabilities::Lora };

const INSTACK_SIZE: usize = 32;
const OUTSTACK_SIZE: usize = 32;
const MAX_INSTACK_PROCESS: usize = 5;
const MAX_OUTSTACK_TRANSMIT: usize = 5;

pub type Uid = NonZeroU8;
pub type InStack = Vec<Message, INSTACK_SIZE>;
pub type OutStack = Vec<Message, OUTSTACK_SIZE>;

pub struct LoraDevice<RK, DLY, IS, OS>
where
    RK: RadioKind,
    DLY: DelayUs,
    IS: MessageStack + 'static,
    OS: MessageStack + 'static,
{
    uid: Uid,
    lora_config: LoraConfig,
    radio: LoRa<RK, DLY>,
    state: DeviceState,
    instack: &'static mut IS,
    outstack: &'static mut OS,
    routing_table: RoutingTable,
}

pub enum DeviceState {
    Idle,
    Transmitting,
    Receiving,
}

impl<RK, DLY, IS, OS> LoraDevice<RK, DLY, IS, OS>
where
    RK: RadioKind,
    DLY: DelayUs,
    IS: MessageStack + 'static,
    OS: MessageStack + 'static,
{
    pub fn new(
        uid: Uid,
        radio: LoRa<RK, DLY>,
        lora_config: LoraConfig,
        device_config: DeviceConfig,
        instack: &'static mut IS,
        outstack: &'static mut OS,
    ) -> Self {
        unsafe {
            DEVICE_CONFIG = device_config;
        }
        Self {
            uid,
            radio,
            state: DeviceState::Idle,
            lora_config,
            instack,
            outstack,
            routing_table: RoutingTable::default(),
        }
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn receive_message(&mut self, message: Message) {
        let route = Route {
            next_hop: message.sender_uid, // The UID of the node that sent the message
                                          // ... other possible fields like cost, hop_count, etc.
        };
        self.routing_table.update(message.sender_uid.get(), route);

        if let Some(receiver) = message.receiver_uid {
            if receiver.get() == self.uid.get() {
                self.instack.push(message).unwrap(); // Handle this unwrap appropriately
            } else if let Some(hop) = message.next_hop {
                if hop.get() == self.uid.get() {
                    self.outstack.push(message).unwrap(); // Handle this unwrap appropriately
                }
            }
        } else {
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
                info!("Pong!");
                Some(pong_message)
            }
            Intent::Data => {
                info!("Received data: {:?}", message.content);
                Some(Message::ack(self.uid, message.sender_uid, message))
            }
            Intent::Discover => {
                let depth = message.content[0];
                if depth > 0 {
                    Some(Message::discover(self.uid, depth - 1))
                } else {
                    None
                }
            }
            Intent::Information => {
                info!("Received information: {:?}", message.content);
                Some(Message::ack(self.uid, message.sender_uid, message))
            }
            _ => None,
        }
    }

    pub async fn send_message(&mut self, mut message: Message) -> Result<(), RadioError> {
        // Your existing send_message logic
        self.radio
            .prepare_for_tx(
                &self.lora_config.modulation,
                self.lora_config.tx_power,
                self.lora_config.boosted,
            )
            .await?;

        if message.next_hop.is_none() && message.receiver_uid.is_some() {
            if let Some(route) = self
                .routing_table
                .lookup_route(message.receiver_uid.unwrap().get())
            {
                message.next_hop = Some(route.next_hop);
            } else {
                // Handle the case where the route is not found
                warn!("Route not found");
            }
        }

        self.state = DeviceState::Transmitting;
        Timer::after(Duration::from_millis(200)).await;
        message.sender_uid = self.uid;
        let buffer: [u8; 70] = message.into();
        info!("Sending message: {:?}", buffer);
        self.radio
            .tx(
                &self.lora_config.modulation,
                &mut self.lora_config.tx_pkt_params,
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
}

pub async fn run_device<RK, DLY, IS, OS>(mut device: LoraDevice<RK, DLY, IS, OS>, buf: &mut [u8])
where
    RK: RadioKind,
    DLY: DelayUs,
    IS: MessageStack + 'static,
    OS: MessageStack + 'static,
{
    loop {
        device.radio.prepare_for_rx(&device.lora_config.modulation, &device.lora_config.rx_pkt_params,
                                    Some(1), None,
                                    false).await.expect("Failed to prepare for RX");

        Timer::after(Duration::from_millis(50)).await;
        match device.radio.rx(&device.lora_config.rx_pkt_params, buf).await {
            Ok((size, _status)) => {
                if let Ok(message) = Message::try_from(&buf[..size as usize]) {
                    info!("Received message: {:?}", message);
                    device.receive_message(message);
                } else {
                    warn!("Received invalid message");
                }
            }
            Err(RadioError::ReceiveTimeout) => {
               // Do nothing
            }
            Err(e) => {
                error!("Error receiving message: {:?}", e);
            }
        }

        // Process InStack
        if !device.instack.is_empty() {
            if let Err(e) = device.process_instack().await {
                error!("Error processing instack: {:?}", e);
            }
        }

        // Process OutStack
        if !device.outstack.is_empty() {
            if let Err(e) = device.process_outstack().await {
                error!("Error processing outstack: {:?}", e);
            }
        }

        // Add a delay or yield the task to prevent it from hogging the CPU
        // For example, using embassy_time's Timer:
        Timer::after(Duration::from_millis(10)).await;
    }
}
