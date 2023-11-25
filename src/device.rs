use config::lora_config::LoraConfig;
use crate::device::collections::MessageStack;
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
use crate::device::config::device_config::DeviceConfig;
use crate::message::payload::Payload;

pub mod config;
pub mod device_error;
pub mod collections;

pub static mut DEVICE_CONFIG: Option<DeviceConfig> = None;

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
            DEVICE_CONFIG = Some(device_config);
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

    pub fn receive_message(&mut self, message: Message)
    {
        let route = Route {
            next_hop: message.source_id(), // The UID of the node that sent the message
                                          // ... other possible fields like cost, hop_count, etc.
        };

        if let Some(receiver) = message.destination_id() {
            if receiver.get() == self.uid.get() {
                self.instack.push(message).unwrap(); // Handle this unwrap appropriately
            }
        } else {
            self.instack.push(message).unwrap(); // Handle this unwrap appropriately
        }
    }

    pub async fn process_instack(&mut self) -> Result<(), RadioError>
    {
        let to_process = core::cmp::min(self.instack.len(), MAX_INSTACK_PROCESS);
        for _ in 0..to_process {
            let message: Message = self.instack.pop().unwrap(); // Handle this unwrap appropriately
            self.process_message(message).await;
        }
        Ok(())
    }

    pub async fn process_outstack(&mut self) -> Result<(), RadioError>
    {
        let to_transmit = core::cmp::min(self.outstack.len(), MAX_OUTSTACK_TRANSMIT);
        for _ in 0..to_transmit {
            let message : Message = self.outstack.pop().unwrap(); // Handle this unwrap appropriately
            self.send_message(message).await?;
        }
        Ok(())
    }

    pub async fn process_message(&mut self, message: Message)
    {
        // Your existing logic for processing messages
        match message.payload() {
            Payload::Data(data) => {
                info!("Received data: {:?}", data);
            }
            Payload::Command(command) => {
                info!("Received command: {:?}", command);
            }
            Payload::Ack(ack) => {
                info!("Received ack: {:?}", ack);
            }
        }
    }

    pub async fn send_message(&mut self, message: Message) -> Result<(), RadioError>
    {
        self.state = DeviceState::Transmitting;
        // Your existing send_message logic
        let tx_message = Message::new(
            self.uid,
            message.destination_id(),
            message.payload().clone(),
            message.ttl(),
        );
        self.outstack.push(tx_message).unwrap(); // Handle this unwrap appropriately
        self.state = DeviceState::Idle;
        Ok(())
    }

    pub async fn discover_nodes(&mut self, depth: u8) -> Result<(), RadioError> {
        if depth == 0 {
            return Ok(());
        }
        todo!()
        // self.send_message(message).await
    }

    async fn tx_message(&mut self, message: Message) -> Result<(), RadioError> {
        // Your existing send_message logic
        self.radio
            .prepare_for_tx(
                &self.lora_config.modulation,
                self.lora_config.tx_power,
                self.lora_config.boosted,
            )
            .await?;

        self.state = DeviceState::Transmitting;
        let buffer: [u8; 70] = message.into();
        Timer::after(Duration::from_millis(200)).await;
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
}

pub async fn run_device<RK, DLY, IS, OS, C>(mut device: LoraDevice<RK, DLY, IS, OS>, buf: &mut [u8])
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
