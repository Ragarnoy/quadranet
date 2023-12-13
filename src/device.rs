use core::cmp;
use core::num::NonZeroU8;

use config::lora_config::LoraConfig;
use defmt::{error, info, trace, warn};
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_async::delay::DelayNs;
use heapless::{FnvIndexMap, Vec};
use lora_phy::mod_params::RadioError;
use lora_phy::mod_traits::RadioKind;
use lora_phy::LoRa;

use crate::device::collections::MessageQueue;
use crate::device::config::device_config::DeviceConfig;
use crate::device::device_error::DeviceError;
use crate::device::pending_ack::*;
use crate::message::payload::ack::AckType;
use crate::message::payload::route::RouteType;
use crate::message::payload::Payload::{self, Ack, Discovery};
use crate::message::Message;
use crate::route::routing_table::RoutingTable;
use crate::route::Route;

pub mod collections;
pub mod config;
pub mod device_error;
pub mod pending_ack;

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
    pending_acks: FnvIndexMap<u32, PendingAck, MAX_PENDING_ACKS>,
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
            pending_acks: FnvIndexMap::new(),
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
        } else if !message.is_expired() {
            self.outqueue.enqueue(message.clone()).unwrap();
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
            message.req_ack(),
        );
        if let Ack(AckType::AckDiscovered {
            hops: _hops,
            last_hop,
        }) = *message.payload()
        {
            let payload = Ack(AckType::AckDiscovered {
                hops: 0,
                last_hop: self.uid,
            });
            message = Message::new(
                self.uid,
                Some(last_hop),
                payload,
                message.ttl(),
                message.req_ack(),
            );
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
                message.req_ack(),
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
            let message: Message = self.outqueue.dequeue().expect("Outqueue is empty");
            self.send_message(message).await?;
        }
        Ok(())
    }

    pub async fn process_message(&mut self, message: Message) {
        // Your existing logic for processing messages
        match message.payload() {
            Payload::Data(data) => {
                info!("Received data: {:?}", defmt::Debug2Format(data));
                if message.req_ack() {
                    let res = self.outqueue.enqueue(Message::new(
                        self.uid,
                        Some(message.source_id()),
                        Ack(AckType::Success {
                            message_id: message.message_id(),
                        }),
                        message.ttl(),
                        false,
                    ));

                    if let Err(e) = res {
                        error!("Error enqueueing ack message: {:?}", e);
                    }
                }
            }
            Payload::Command(command) => {
                info!("Received command: {:?}", defmt::Debug2Format(command));
                if message.req_ack() {
                    let res = self.outqueue.enqueue(Message::new(
                        self.uid,
                        Some(message.source_id()),
                        Ack(AckType::Success {
                            message_id: message.message_id(),
                        }),
                        message.ttl(),
                        false,
                    ));

                    if let Err(e) = res {
                        error!("Error enqueueing ack message: {:?}", e);
                    }
                }
            }
            Ack(ack) => match ack {
                AckType::Success { .. } => {}
                AckType::AckDiscovered { hops, last_hop } => {
                    self.pending_acks
                        .get_mut(&message.message_id())
                        .unwrap()
                        .is_acknowledged = true;
                    self.routing_table.update(
                        message.source_id().get(),
                        Route {
                            next_hop: *last_hop,
                            hop_count: *hops,
                        },
                    );
                }
                AckType::Failure { .. } => {}
            },
            Payload::Route(route) => match route {
                RouteType::Request => {}
                RouteType::Response => {}
                RouteType::Error => {}
            },
            Discovery { original_ttl } => {
                let hops = original_ttl - message.ttl();
                let res = self.outqueue.enqueue(Message::new(
                    self.uid,
                    Some(message.source_id()),
                    Ack(AckType::AckDiscovered {
                        hops,
                        last_hop: self.uid,
                    }),
                    *original_ttl,
                    false,
                ));

                if let Err(e) = res {
                    error!("Error enqueueing discovery response message: {:?}", e);
                }
            }
        }
    }

    async fn send_message(&mut self, message: Message) -> Result<(), RadioError> {
        // Your existing send_message logic

        if message.req_ack() {
            let pending_ack = PendingAck::new(
                message.payload().clone(),
                message.destination_id(),
                message.ttl(),
            );
            if self.pending_acks.contains_key(&message.message_id()) {
            } else {
                self.pending_acks
                    .insert(message.message_id(), pending_ack)
                    .unwrap_or_else(|_| {
                        error!("Error inserting pending ack");
                        None
                    });

            }
        }

        self.tx_message(message).await?;
        Ok(())
    }

    pub async fn discover_nodes(&mut self) {
        let res = self.outqueue.enqueue(Message::new(
            self.uid,
            None,
            Discovery { original_ttl: 5 },
            1,
            true,
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
        trace!("Sending message: {:?}", buffer);
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

    pub async fn check_pending_acks(&mut self) {
        let now = Instant::now();
        for (id, ack) in self.pending_acks.iter_mut() {
            if now.duration_since(ack.timestamp) > Duration::from_secs(ACK_WAIT_TIME) {
                if ack.attempts < MAX_ACK_ATTEMPTS {
                    let mut message = Message::new(
                        self.uid,
                        ack.destination_uid(),
                        ack.payload().clone(),
                        ack.ttl(),
                        true,
                    );
                    message.set_message_id(*id);
                    self.outqueue.enqueue(message).unwrap_or_else(|e| {
                        error!("Error enqueueing message: {:?}", e);
                    });
                    ack.timestamp = Instant::now();
                    ack.attempts += 1;
                    info!("Attempt {} for message: {}", ack.attempts, id);
                } else {
                    warn!("Max attempts reached for message: {}", id);
                    ack.is_acknowledged = true;
                }
            }
        }
        self.pending_acks.retain(|_id, ack| !ack.is_acknowledged);
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
    device.discover_nodes().await;
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

        // Check for pending acks
        device.check_pending_acks().await;

        // Add a delay or yield the task to prevent it from hogging the CPU
        Timer::after(Duration::from_millis(10)).await;
    }
}

pub fn device_state() -> DeviceState {
    unsafe { DEVICE_STATE }
}
