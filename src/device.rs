use core::cmp;
use core::num::NonZeroU8;
use core::sync::atomic::{AtomicU8, Ordering};
use config::lora::LoraConfig;
use defmt::{debug, error, info, warn, Display2Format};
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_async::delay::DelayNs;
use heapless::{FnvIndexMap, Vec};
use lora_phy::mod_params::RadioError;
use lora_phy::mod_traits::RadioKind;
use lora_phy::{LoRa, RxMode};

use crate::device::collections::MessageQueue;
use crate::device::config::device::DeviceConfig;
use crate::device::device_error::DeviceError;
use crate::device::pending_ack::{MAX_ACK_ATTEMPTS, MAX_PENDING_ACKS, PendingAck};
use crate::message::payload::ack::AckType;
use crate::message::payload::data::DataType;
use crate::message::payload::route::RouteType;
use crate::message::payload::Payload;
use crate::message::Message;
use crate::route::routing_table::{RoutingTable, ROUTE_EXPIRY_SECONDS};
use crate::route::Route;

pub mod collections;
pub mod config;
pub mod device_error;
pub mod pending_ack;

static STATS_COUNTER: AtomicU8 = AtomicU8::new(0);

const INQUEUE_SIZE: usize = 32;
const OUTQUEUE_SIZE: usize = 32;
const MAX_INQUEUE_PROCESS: usize = 5;
const MAX_OUTQUEUE_TRANSMIT: usize = 5;

pub type Uid = NonZeroU8;
pub type InQueue = Vec<Message, INQUEUE_SIZE>;
pub type OutQueue = Vec<Message, OUTQUEUE_SIZE>;

const INITIAL_BACKOFF_MS: u64 = 500; // Start with 500ms backoff
const BACKOFF_FACTOR: u64 = 2; // Double the backoff each retry
const MAX_BACKOFF_MS: u64 = 10000; // Max backoff of 10 seconds

pub struct RxInfo {
    pub rssi: i16,
    pub snr: i16,
    pub timestamp: Instant,
}

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
    device_config: DeviceConfig, // Encapsulated config instead of global
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum DeviceState {
    Idle,
    Transmitting,
    Receiving,
}

/// Represents a `LoRa` device in a P2P Mesh Network.
///
/// This struct encapsulates the functionality required for a `LoRa` device
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
/// - `lora_config`: Configuration settings for the `LoRa` radio.
/// - `radio`: The `LoRa` radio instance.
/// - `state`: Current state of the device (Idle, Transmitting, Receiving).
/// - `inqueue`: Queue for incoming messages.
/// - `outqueue`: Queue for outgoing messages.
/// - `routing_table`: Table for managing routes to other devices.
/// - `device_config`: Configuration for the device behavior.
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
        Self {
            uid,
            radio,
            state: DeviceState::Idle,
            lora_config,
            inqueue,
            outqueue,
            pending_acks: FnvIndexMap::new(),
            routing_table: RoutingTable::default(),
            device_config,
        }
    }

    pub const fn uid(&self) -> Uid {
        self.uid
    }

    // Return current device state for external observers
    pub const fn device_state(&self) -> DeviceState {
        self.state
    }

    // Return current device configuration
    pub const fn device_config(&self) -> &DeviceConfig {
        &self.device_config
    }

    /// Enqueue message with optional RX info
    async fn enqueue_message_with_rx_info(&mut self, message: Message, rx_info: Option<&RxInfo>) {
        if let Some(receiver) = message.destination_id() {
            if receiver.get() == self.uid.get() {
                if let Err(e) = self.inqueue.enqueue(message) {
                    error!("Error enqueueing message: {:?}", e);
                }
            } else if !message.is_expired() {
                if let Err(e) = self.route_message(message, rx_info).await {
                    error!("Error routing message: {:?}", e);
                }
            }
        } else if !message.is_expired() {
            // Broadcast message
            self.outqueue.enqueue(message.clone()).unwrap_or_else(|e| {
                error!("Error enqueueing broadcast message to outqueue: {:?}", e);
            });

            if let Err(e) = self.inqueue.enqueue(message) {
                error!("Error enqueueing broadcast message to inqueue: {:?}", e);
            }
        }
    }

    /// Route message with optional signal quality information
    async fn route_message(&mut self, mut message: Message, rx_info: Option<&RxInfo>) -> Result<(), DeviceError> {
        // Process Ack messages as before
        if let Payload::Ack(AckType::AckDiscovered { hops: _hops, last_hop }) = *message.payload() {
            let payload = AckType::AckDiscovered {
                hops: 0,
                last_hop: self.uid,
            };
            message = Message::new_ack(
                self.uid,
                Some(last_hop),
                payload,
                message.ttl(),
                message.req_ack(),
            );
        }

        // Extract destination
        let destination_id = match message.destination_id() {
            Some(id) => id.get(),
            None => return Err(DeviceError::InvalidDestination),
        };

        // Update link quality if we have signal information
        if let Some(info) = rx_info {
            let source_id = message.source_id().get();
            self.routing_table.update_link_quality(source_id, info.rssi, info.snr);

            // Create or update a direct route to the sender
            let route = Route::with_quality(
                message.source_id(),
                1, // Direct hop
                calculate_quality(info.rssi, info.snr),
            );
            self.routing_table.update(source_id, route);
        }

        // Look up best route to destination
        if let Some(route) = self.routing_table.lookup_route(destination_id) {
            // Create forwarded message
            message = Message::new(
                self.uid,
                Some(route.next_hop),
                message.payload().clone(),
                message.ttl(),
                message.req_ack(),
            );
            message.decrement_ttl();

            // Attempt to transmit
            self.tx_message(message).await?;

            // Record successful routing for link quality tracking
            self.routing_table.record_successful_delivery(route.next_hop.get());
        } else {
            // No route found, initiate route discovery if needed
            if !self.is_route_discovery_in_progress(destination_id) {
                self.initiate_route_discovery(destination_id);
            }
            return Err(DeviceError::RouteNotFound);
        }

        Ok(())
    }


    /// Check if a route discovery is already in progress for a destination
    fn is_route_discovery_in_progress(&self, destination: u8) -> bool {
        for (_, ack) in &self.pending_acks {
            if let Payload::Discovery(_) = ack.payload() {
                if let Some(dest) = ack.destination_uid() {
                    if dest.get() == destination {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Initiate targeted route discovery for a specific destination
    fn initiate_route_discovery(&mut self, destination: u8) {
        match NonZeroU8::new(destination) {
            Some(dest_uid) => {
                debug!("Initiating route discovery for node @{}", destination);

                let message = Message::new_discovery(
                    self.uid,
                    Some(dest_uid),
                    3,  // TTL
                    true, // Request ACK
                    self.device_config,
                );

                if let Err(e) = self.outqueue.enqueue(message) {
                    error!("Error enqueueing targeted discovery message: {:?}", e);
                }
            },
            None => warn!("Cannot initiate discovery for invalid destination 0"),
        }
    }

    /// Process received messages with signal quality information
    pub async fn process_message_with_rx_info(&mut self, message: &Message, rx_info: RxInfo) {
        // First, update link quality
        let source_id = message.source_id().get();
        self.routing_table.update_link_quality(source_id, rx_info.rssi, rx_info.snr);

        // Process the message content
        self.process_message_internal(message, Some(&rx_info));

        // Enqueue the message if needed
        self.enqueue_message_with_rx_info(message.clone(), Some(&rx_info)).await;
    }

    /// Internal message processing with optional signal info
    fn process_message_internal(&mut self, message: &Message, rx_info: Option<&RxInfo>) {
        match message.payload() {
            Payload::Data(data) => {
                match data {
                    DataType::Text(text) => {
                        debug!("Received text message: {}", text);
                    }
                    DataType::Binary(_) => {
                        debug!("Received data: {:?}", defmt::Debug2Format(data));
                    }
                }
                if message.req_ack() {
                    self.ack_success(message);
                }
            }
            Payload::Command(command) => {
                debug!("Received command: {:?}", defmt::Debug2Format(command));
                if message.req_ack() {
                    self.ack_success(message);
                }
            }
            Payload::Ack(ack) => self.handle_ack_message(message, *ack, rx_info),
            Payload::Route(route) => match route {
                RouteType::Response | RouteType::Error | RouteType::Request => {}
            },
            Payload::Discovery(discovery) => {
                // For discovery messages, compute actual hop count
                let hops = discovery.original_ttl - message.ttl();

                // If we have signal info, create a route back to source
                if let Some(info) = rx_info {
                    let quality = calculate_quality(info.rssi, info.snr);
                    let route = Route::with_quality(message.source_id(), 1, quality);
                    self.routing_table.update(message.source_id().get(), route);
                }

                // Send acknowledgment for discovery
                let res = self.outqueue.enqueue(Message::new_ack(
                    self.uid,
                    Some(message.source_id()),
                    AckType::AckDiscovered {
                        hops,
                        last_hop: self.uid,
                    },
                    message.ttl(),
                    false,
                ));

                if let Err(e) = res {
                    error!("Error enqueueing discovery response message: {:?}", e);
                }
            }
        }
    }

    pub fn process_inqueue(&mut self) -> Result<(), RadioError> {
        let to_process = cmp::min(self.inqueue.len(), MAX_INQUEUE_PROCESS);
        for _ in 0..to_process {
            if let Ok(message) = self.inqueue.dequeue() {
                // Use process_message_internal instead of the redundant process_message
                self.process_message_internal(&message, None);
            }
        }
        Ok(())
    }

    pub async fn process_outqueue(&mut self) -> Result<(), RadioError> {
        let to_transmit = cmp::min(self.outqueue.len(), MAX_OUTQUEUE_TRANSMIT);
        for _ in 0..to_transmit {
            if let Ok(message) = self.outqueue.dequeue() {
                self.send_message(message).await?;
            }
        }
        Ok(())
    }

    /// Handle acknowledgment messages with signal quality info
    fn handle_ack_message(&mut self, message: &Message, ack: AckType, rx_info: Option<&RxInfo>) {
        match ack {
            AckType::Success { message_id } => {
                if let Some(pending_ack) = self.pending_acks.get_mut(&message_id) {
                    info!("Success ACK received for Message {}", message_id);
                    pending_ack.is_acknowledged = true;

                    // Record successful delivery
                    self.routing_table.record_successful_delivery(message.source_id().get());
                }
            },
            AckType::AckDiscovered { hops, last_hop } => {
                // Create or update route using the hop count
                let route = Route::new(last_hop, hops);

                // If we have signal quality info, use it to set route quality
                let mut enhanced_route = route;
                if let Some(info) = rx_info {
                    enhanced_route.quality = calculate_quality(info.rssi, info.snr);
                }

                // Update the routing table
                self.routing_table.update(message.source_id().get(), enhanced_route);

                // Mark our discovery as acknowledged if we originated it
                if message.destination_id() == Some(self.uid) {
                    if let Some(pending_ack) = self.pending_acks.get_mut(&message.message_id()) {
                        info!("Discovery ACK Complete for Message {}", message.message_id());
                        pending_ack.is_acknowledged = true;
                    } else {
                        warn!("Received unexpected AckDiscovered for message ID: {}", message.message_id());
                    }
                }
            },
            AckType::Failure { message_id } => {
                if let Some(pending_ack) = self.pending_acks.get_mut(&message_id) {
                    warn!("Failure ACK received for Message {}", message_id);
                    pending_ack.is_acknowledged = true;

                    // Record failure for the route
                    if let Some(dest) = pending_ack.destination_uid() {
                        if let Some(route) = self.routing_table.lookup_route(dest.get()) {
                            self.routing_table.record_failed_delivery(route.next_hop.get());
                        }
                    }
                }
            },
        }
    }

    fn ack_success(&mut self, message: &Message) {
        let res = self.outqueue.enqueue(Message::new_ack(
            self.uid,
            Some(message.source_id()),
            AckType::Success {
                message_id: message.message_id(),
            },
            message.ttl(),
            false,
        ));

        if let Err(e) = res {
            error!("Error enqueueing ack message: {:?}", e);
        }
    }

    async fn send_message(&mut self, message: Message) -> Result<(), RadioError> {
        // Centralized pending ack creation
        if message.req_ack() {
            self.add_pending_ack(&message);
        }

        self.tx_message(message).await?;
        Ok(())
    }

    // New method for handling pending ack creation
    fn add_pending_ack(&mut self, message: &Message) {
        let pending_ack = PendingAck::new(
            message.payload().clone(),
            message.destination_id(),
            message.ttl(),
        );

        let message_id = message.message_id();
        if !self.pending_acks.contains_key(&message_id) {
            if self.pending_acks.insert(message_id, pending_ack).is_err() {
                error!("Error inserting pending ack for message: {}", message_id);
            } else {
                debug!("Added pending ack for message: {}", message_id);
            }
        }
    }

    pub fn discover_nodes(&mut self) {
        // Use device config for discovery messages
        let res = self.outqueue.enqueue(Message::new_discovery(
            self.uid,
            None,
            3,
            true,
            self.device_config, // Pass device config instead of relying on global
        ));

        if let Err(e) = res {
            error!("Error enqueueing discovery message: {:?}", e);
        }
    }

    async fn tx_message(&mut self, message: Message) -> Result<(), RadioError> {
        let buffer: [u8; 70] = message.into();
        let params = &mut self.lora_config.tx_pkt_params;

        self.radio
            .prepare_for_tx(
                &self.lora_config.modulation,
                params,
                self.lora_config.tx_power,
                &buffer,
            )
            .await?;

        self.state = DeviceState::Transmitting;
        Timer::after(Duration::from_millis(100)).await;
        debug!("Sending message: {:?}", buffer);
        self.radio.tx().await?;
        self.state = DeviceState::Idle;
        Ok(())
    }

    /// Enhanced `try_wait_message` with RSSI/SNR capture
    async fn try_wait_message(&mut self, buf: &mut [u8]) {
        self.state = DeviceState::Receiving;
        let result = self.radio
            .prepare_for_rx(
                RxMode::Single(10000),
                &self.lora_config.modulation,
                &self.lora_config.rx_pkt_params,
            )
            .await;

        if let Err(e) = result {
            error!("Failed to prepare for RX: {:?}", e);
            self.state = DeviceState::Idle;
            return;
        }

        Timer::after(Duration::from_millis(50)).await;
        match self.radio.rx(&self.lora_config.rx_pkt_params, buf).await {
            Ok((size, status)) => {
                // Get RSSI and SNR from status
                let rx_info = RxInfo {
                    rssi: status.rssi,
                    snr: status.snr,
                    timestamp: Instant::now(),
                };

                debug!("Received packet: RSSI {}dBm, SNR {}dB", rx_info.rssi, rx_info.snr);

                match Message::try_from(&mut buf[..size as usize]) {
                    Ok(message) => {
                        // Process with signal quality info
                        self.process_message_with_rx_info(&message, rx_info).await;
                    }
                    Err(e) => {
                        warn!("Received invalid message:{}", Display2Format(&e));
                    }
                }
            }
            Err(RadioError::ReceiveTimeout) => {
                // Do nothing on timeout
            }
            Err(e) => {
                error!("Error receiving message: {:?}", e);
            }
        }
        self.state = DeviceState::Idle;
    }
    
    pub fn check_pending_acks_and_routes(&mut self) {
        // First handle pending acks
        // Collect messages that need retrying to avoid borrow checker issues
        let mut to_retry = Vec::<(u32, u8), 16>::new();
        let mut to_acknowledge = Vec::<u32, 16>::new();

        {
            let now = Instant::now();
            // First pass: identify messages that need retry or acknowledgment
            for (id, ack) in &mut self.pending_acks {
                // Calculate backoff using exponential strategy
                let backoff_ms = calculate_backoff(ack.attempts);
                let retry_threshold = Duration::from_millis(backoff_ms);

                if now.duration_since(ack.timestamp) > retry_threshold {
                    if ack.attempts < MAX_ACK_ATTEMPTS {
                        // Mark for retry (store ID and current attempts)
                        to_retry.push((*id, ack.attempts)).unwrap_or_else(|_| {
                            warn!("Retry list full, skipping message: {}", id);
                        });

                        // Update fields in place
                        ack.timestamp = Instant::now();
                        ack.attempts += 1;
                    } else {
                        warn!("Max attempts reached for message: {}", id);
                        // Mark for acknowledgment (which will remove it)
                        to_acknowledge.push(*id).unwrap_or_else(|_| {
                            warn!("Acknowledge list full, skipping message: {}", id);
                        });

                        // Record routing failure if this was to a specific destination
                        if let Some(dest_uid) = ack.destination_uid() {
                            if let Some(route) = self.routing_table.lookup_route(dest_uid.get()) {
                                self.routing_table.record_failed_delivery(route.next_hop.get());
                            }
                        }
                    }
                }
            }
        }

        // Second pass: process retries
        for (message_id, attempts) in to_retry {
            self.retry_message(message_id);
            let backoff_ms = calculate_backoff(attempts + 1);
            debug!("Attempt {} for message {}, next retry in {}ms", 
            attempts + 1, message_id, backoff_ms);
        }

        // Mark messages as acknowledged
        for id in to_acknowledge {
            if let Some(ack) = self.pending_acks.get_mut(&id) {
                ack.is_acknowledged = true;
            }
        }

        // Remove acknowledged messages
        self.pending_acks.retain(|_id, ack| !ack.is_acknowledged);

        // Perform route maintenance
        self.routing_table.cleanup();

        // Log routing table stats periodically (every ~10 calls)
        let counter = STATS_COUNTER.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        if counter % 10 == 0 {
            let stats = self.routing_table.stats();
            info!("Routing: {} destinations, {} active, {} expired, avg. hops {}, avg. quality {}",
            stats.total_entries, stats.active_routes, stats.expired_routes,
            stats.avg_hop_count, stats.avg_quality);
        }
    }

    /// Refresh routes that are nearing expiration
    pub async fn refresh_routes(&mut self) {
        // Check our routing table for routes that need refreshing
        // This would be called periodically from a maintenance task

        let mut to_refresh = Vec::<u8, 16>::new();

        // Scan the routing table for routes that need refreshing
        for dest_id in 1..=255u8 {
            if dest_id != self.uid.get() &&
                NonZeroU8::new(dest_id).is_some() &&
                self.routing_table.needs_refresh(dest_id) {
                to_refresh.push(dest_id).unwrap_or_else(|_| {
                    warn!("Refresh list full, skipping node @{}", dest_id);
                });
            }
        }

        // Initiate discovery for routes that need refreshing
        for dest_id in to_refresh {
            // If we have a route but it's nearing expiration, refresh it
            self.initiate_route_discovery(dest_id);

            // Add a small delay between discoveries to avoid congestion
            Timer::after(Duration::from_millis(100)).await;
        }
    }

    // Separate method for message retry logic
    fn retry_message(&mut self, message_id: u32) {
        // Get information from pending ack
        let destination_uid;
        let payload;
        let ttl;

        {
            // Scope the borrow to get the information we need
            if let Some(ack) = self.pending_acks.get(&message_id) {
                destination_uid = ack.destination_uid();
                payload = ack.payload().clone();
                ttl = ack.ttl();
            } else {
                error!("No pending ack found for message: {}", message_id);
                return;
            }
        }

        // Create and send the retry message
        let mut message = Message::new(self.uid, destination_uid, payload, ttl, true);
        message.set_message_id(message_id);

        if let Err(e) = self.outqueue.enqueue(message) {
            error!("Error enqueueing retry message: {:?}", e);
        }
    }
}

pub async fn run_quadranet<RK, DLY, IN, OUT>(
    device: LoraDevice<RK, DLY, IN, OUT>,
    buf: &mut [u8],
) -> Result<(), DeviceError>
where
    RK: RadioKind,
    DLY: DelayNs,
    IN: MessageQueue + 'static,
    OUT: MessageQueue + 'static,
{
    let mut device = device;

    // Discover the network initially
    device.discover_nodes();

    // Setup timers for periodic maintenance
    let mut last_route_refresh = Instant::now();
    let route_refresh_interval = Duration::from_secs(ROUTE_EXPIRY_SECONDS / 2);

    loop {
        // Wait for a message
        device.try_wait_message(buf).await;

        // Process InQueue when it's not full
        if device.inqueue.len() < INQUEUE_SIZE - 1 {
            if let Err(e) = device.process_inqueue() {
                error!("Error processing inqueue: {:?}", e);
            }
        }

        // Process OutQueue
        if !device.outqueue.is_empty() {
            if let Err(e) = device.process_outqueue().await {
                error!("Error processing outqueue: {:?}", e);
            }
        }

        // Check for pending acks and perform route maintenance
        device.check_pending_acks_and_routes();

        // Periodically refresh routes
        if Instant::now().duration_since(last_route_refresh) > route_refresh_interval {
            device.refresh_routes().await;
            last_route_refresh = Instant::now();
        }

        // Add a delay to prevent CPU hogging
        Timer::after(Duration::from_millis(10)).await;
    }
}

/// Calculate exponential backoff
fn calculate_backoff(attempt: u8) -> u64 {
    if attempt == 0 {
        return INITIAL_BACKOFF_MS;
    }

    // Calculate exponential backoff: initial_backoff * backoff_factor^attempt
    let backoff = INITIAL_BACKOFF_MS * BACKOFF_FACTOR.pow(u32::from(attempt));

    // Cap at maximum backoff
    backoff.min(MAX_BACKOFF_MS)
}


/// Calculate signal quality score (0-255)
fn calculate_quality(rssi: i16, snr: i16) -> u8 {
    // Normalize RSSI: -120dBm -> 0, -30dBm -> 100
    let rssi_norm = (f32::from(rssi + 120) / 90.0 * 100.0).clamp(0.0, 100.0) as u16;

    // Normalize SNR: -20dB -> 0, +10dB -> 100
    let snr_norm = (f32::from(snr + 20) / 30.0 * 100.0).clamp(0.0, 100.0) as u16;

    // Calculate combined score weighted toward SNR
    let quality = (rssi_norm * 4 + snr_norm * 6) / 10;

    // Scale to 0-255
    ((quality * 255) / 100) as u8
}

// Helper function to get device state from outside
pub const fn device_state<RK, DLY, IN, OUT>(device: &LoraDevice<RK, DLY, IN, OUT>) -> DeviceState
where
    RK: RadioKind,
    DLY: DelayNs,
    IN: MessageQueue + 'static,
    OUT: MessageQueue + 'static,
{
    device.device_state()
}