use core::cmp;
use crate::device::collections::MessageQueue;
use crate::device::config::device_config::DeviceConfig;
use crate::device::device_error::DeviceError;
use crate::message::payload::discovery::DiscoveryType;
use crate::message::payload::route::RouteType;
use crate::message::payload::Payload;
use crate::message::payload::Payload::Discovery;
use crate::message::Message;
use crate::route::routing_table::RoutingTable;
use crate::route::Route;
use config::lora_config::LoraConfig;
use core::num::NonZeroU8;
use defmt::{debug, error, info, warn};
use embassy_time::{Duration, Timer};
use embedded_hal_async::delay::DelayNs;
use heapless::Vec;
use lora_phy::mod_params::RadioError;
use lora_phy::mod_traits::RadioKind;
use lora_phy::LoRa;

pub mod collections;
pub mod config;
pub mod device_error;

pub static mut DEVICE_CONFIG: Option<DeviceConfig> = None;

static mut DEVICE_STATE: DeviceState = DeviceState::Idle;

const INQUEUE_SIZE: usize = 32;
const OUTQUEUE_SIZE: usize = 32;
const MAX_INQUEUE_PROCESS: usize = 5;
const MAX_OUTQUEUE_TRANSMIT: usize = 5;

pub type Uid = NonZeroU8;
pub type InQueue = Vec<Message, INQUEUE_SIZE>;
pub type OutQueue = Vec<Message, OUTQUEUE_SIZE>;

pub struct LoraDevice<RK, DLY, IN, OUT>
where
    RK: RadioKind,
    DLY: DelayNs,
    IN: MessageQueue + 'static,
    OUT: MessageQueue + 'static,
{
    uid: Uid,
    lora_config: LoraConfig,
    radio: LoRa<RK, DLY>,
    state: DeviceState,
    inqueue: &'static mut IN,
    outqueue: &'static mut OUT,
    routing_table: RoutingTable,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum DeviceState {
    Idle,
    Transmitting,
    Receiving,
}

/// Represents a LoRa device in a P2P Mesh Network.
///
/// This struct encapsulates the functionality required for a LoRa device
/// to participate in the mesh network, including message handling,
/// routing, and network discovery.
///
/// # Generic Parameters
/// - `RK`: The type that defines the kind of radio being used.
/// - `DLY`: Delay trait for asynchronous operations.
/// - `IN`: Message queue for incoming messages.
/// - `OUT`: Message queue for outgoing messages.
///
/// # Fields
/// - `uid`: Unique identifier of the device.
/// - `lora_config`: Configuration settings for the LoRa radio.
/// - `radio`: The LoRa radio instance.
/// - `state`: Current state of the device (Idle, Transmitting, Receiving).
/// - `inqueue`: Queue for incoming messages.
/// - `outqueue`: Queue for outgoing messages.
/// - `routing_table`: Table for managing routes to other devices.
impl<RK, DLY, IN, OUT> LoraDevice<RK, DLY, IN, OUT>
where
    RK: RadioKind,
    DLY: DelayNs,
    IN: MessageQueue + 'static,
    OUT: MessageQueue + 'static,
{
    pub fn new(
        uid: Uid,
        radio: LoRa<RK, DLY>,
        lora_config: LoraConfig,
        device_config: DeviceConfig,
        inqueue: &'static mut IN,
        outqueue: &'static mut OUT,
    ) -> Self {
        unsafe {
            DEVICE_CONFIG = Some(device_config);
        }
        Self {
            uid,
            radio,
            state: DeviceState::Idle,
            lora_config,
            inqueue,
            outqueue,
            routing_table: RoutingTable::default(),
        }
    }

    pub fn uid(&self) -> Uid {
        self.uid
    }

    pub fn update_state(&self) {
        unsafe {
            DEVICE_STATE = self.state;
        }
    }

    pub async fn enqueue_message(&mut self, message: Message) {
        if let Some(receiver) = message.destination_id() {
            if receiver.get() == self.uid.get() {
                if let Err(e) = self.inqueue.enqueue(message) {
                    error!("Error enqueueing message: {:?}", e);
                }
            } else if !message.is_expired() {
                if let Err(e) = self.route_message(message).await {
                    error!("Error routing message: {:?}", e);
                }
            }
        } else {
            if !message.is_expired() {
                if let Err(e) = self.route_message(message.clone()).await {
                    error!("Error routing message: {:?}", e);
                }
            }
            if let Err(e) = self.inqueue.enqueue(message) {
                error!("Error enqueueing message: {:?}", e);
            }
        }
    }

    async fn route_message(&mut self, message: Message) -> Result<(), DeviceError> {
        let mut message = Message::new(
            self.uid,
            message.destination_id(),
            message.payload().clone(),
            message.ttl(),
        );
        if let Discovery(DiscoveryType::Response {
            hops: _hops,
            last_hop,
        }) = *message.payload()
        {
            let payload = Discovery(DiscoveryType::Response {
                hops: 0,
                last_hop: self.uid,
            });
            message = Message::new(self.uid, Some(last_hop), payload, message.ttl());
        }

        if let Some(route) = self
            .routing_table
            .lookup_route(message.destination_id().unwrap().get())
        {
            message = Message::new(
                self.uid,
                Some(route.next_hop),
                message.payload().clone(),
                message.ttl(),
            );
            message.decrement_ttl();
            self.tx_message(message).await?;
        } else {
            return Err(DeviceError::RouteNotFound);
        }
        Ok(())
    }

    pub async fn process_inqueue(&mut self) -> Result<(), RadioError> {
        let to_process = cmp::min(self.inqueue.len(), MAX_INQUEUE_PROCESS);
        for _ in 0..to_process {
            let message: Message = self.inqueue.dequeue().unwrap(); // Handle this unwrap appropriately
            self.process_message(message).await;
        }
        Ok(())
    }

    pub async fn process_outqueue(&mut self) -> Result<(), RadioError> {
        let to_transmit = cmp::min(self.outqueue.len(), MAX_OUTQUEUE_TRANSMIT);
        for _ in 0..to_transmit {
            let message: Message = self.outqueue.dequeue().unwrap(); // Handle this unwrap appropriately
            self.send_message(message).await?;
        }
        Ok(())
    }

    pub async fn process_message(&mut self, message: Message) {
        // Your existing logic for processing messages
        match message.payload() {
            Payload::Data(data) => {
                info!("Received data: {:?}", defmt::Debug2Format(data));
            }
            Payload::Command(command) => {
                info!("Received command: {:?}", defmt::Debug2Format(command));
            }
            Payload::Ack(ack) => {
                info!("Received ack: {:?}", defmt::Debug2Format(ack));
            }
            Payload::Route(route) => match route {
                RouteType::Request => {}
                RouteType::Response => {}
                RouteType::Error => {}
            },
            Discovery(discovery) => match discovery {
                DiscoveryType::Request { original_ttl } => {
                    let hops = original_ttl - message.ttl();
                    let res = self.outqueue.enqueue(Message::new(
                        self.uid,
                        Some(message.source_id()),
                        Discovery(DiscoveryType::Response {
                            hops,
                            last_hop: self.uid,
                        }),
                        *original_ttl,
                    ));

                    if let Err(e) = res {
                        error!("Error enqueueing discovery message: {:?}", e);
                    }
                }
                DiscoveryType::Response { hops, last_hop } => {
                    self.routing_table.update(
                        message.source_id().get(),
                        Route {
                            next_hop: *last_hop,
                            hop_count: *hops,
                        },
                    );
                }
            },
        }
    }

    async fn send_message(&mut self, message: Message) -> Result<(), RadioError> {
        // Your existing send_message logic
        let tx_message = Message::new(
            self.uid,
            message.destination_id(),
            message.payload().clone(),
            message.ttl(),
        );
        self.outqueue.enqueue(tx_message).unwrap(); // Handle this unwrap appropriately
        Ok(())
    }

    pub async fn discover_nodes(&mut self) {
        let res = self.outqueue.enqueue(Message::new(
            self.uid,
            None,
            Discovery(DiscoveryType::Request { original_ttl: 5 }),
            1,
        ));

        if let Err(e) = res {
            error!("Error enqueueing discovery message: {:?}", e);
        }
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

    async fn try_wait_message(&mut self, buf: &mut [u8]) {
        self.state = DeviceState::Receiving;
        self.radio
            .prepare_for_rx(
                &self.lora_config.modulation,
                &self.lora_config.rx_pkt_params,
                Some(1),
                None,
                false,
            )
            .await
            .expect("Failed to prepare for RX");

        Timer::after(Duration::from_millis(50)).await;
        match self.radio.rx(&self.lora_config.rx_pkt_params, buf).await {
            Ok((size, _status)) => {
                if let Ok(message) = Message::try_from(&mut buf[..size as usize]) {
                    info!("Received message: {:?}", message);
                    self.enqueue_message(message).await;
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
        self.state = DeviceState::Idle;
    }
}

pub async fn run_quadranet<RK, DLY, IN, OUT>(
    mut device: LoraDevice<RK, DLY, IN, OUT>,
    buf: &mut [u8],
) where
    RK: RadioKind,
    DLY: DelayNs,
    IN: MessageQueue + 'static,
    OUT: MessageQueue + 'static,
{
    debug!("Starting QuadraNet");
    device.discover_nodes().await;
    debug!("Starting QuadraNet");
    loop {
        // Wait for a message
        device.try_wait_message(buf).await;

        // Process InQueue
        if !device.inqueue.is_empty() {
            if let Err(e) = device.process_inqueue().await {
                error!("Error processing inqueue: {:?}", e);
            }
        }

        // Process OutQueue
        if !device.outqueue.is_empty() {
            if let Err(e) = device.process_outqueue().await {
                error!("Error processing outqueue: {:?}", e);
            }
        }

        // Add a delay or yield the task to prevent it from hogging the CPU
        Timer::after(Duration::from_millis(10)).await;
    }
}

pub fn device_state() -> DeviceState {
    unsafe { DEVICE_STATE }
}
