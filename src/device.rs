use config::lora::LoraConfig;
use core::cmp;
use core::num::NonZeroU8;
use core::sync::atomic::{AtomicU8, Ordering};
use defmt::{error, info, warn};
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_async::delay::DelayNs;
use heapless::index_map::FnvIndexMap;
use heapless::Vec;
use lora_phy::mod_params::RadioError;
use lora_phy::mod_traits::RadioKind;
use lora_phy::{LoRa, RxMode};

use crate::device::collections::MessageQueue;
use crate::device::config::device::DeviceConfig;
use crate::device::device_error::DeviceError;
use crate::device::pending_ack::{PendingAck, MAX_ACK_ATTEMPTS, MAX_PENDING_ACKS};
use crate::message::payload::ack::AckType;
use crate::message::payload::Payload;
use crate::message::Message;
use crate::route::routing_table::RoutingTable;
use crate::route::Route;

pub mod collections;
pub mod config;
pub mod device_error;
pub mod pending_ack;

static STATS_COUNTER: AtomicU8 = AtomicU8::new(0);

// Reduce queue sizes to save memory
const INQUEUE_SIZE: usize = 8; // Reduced from 16
const OUTQUEUE_SIZE: usize = 8; // Reduced from 16
const MAX_INQUEUE_PROCESS: usize = 4;
const MAX_OUTQUEUE_TRANSMIT: usize = 4;

pub type Uid = NonZeroU8;
pub type InQueue = Vec<Message, INQUEUE_SIZE>;
pub type OutQueue = Vec<Message, OUTQUEUE_SIZE>;

const INITIAL_BACKOFF_MS: u64 = 500;
const BACKOFF_FACTOR: u64 = 2;
const MAX_BACKOFF_MS: u64 = 5000;

// More compact RX info structure
#[derive(Copy, Clone)]
pub struct RxInfo {
    pub rssi: i16,
    pub snr: i16,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum DeviceState {
    Idle,
    Transmitting,
    Receiving,
}

// More memory-efficient device with merged functionality
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
    device_config: DeviceConfig,
}

impl<RK, DLY, IN, OUT> LoraDevice<RK, DLY, IN, OUT>
where
    RK: RadioKind,
    DLY: DelayNs,
    IN: MessageQueue + 'static,
    OUT: MessageQueue + 'static,
{
    pub const fn new(
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
            routing_table: RoutingTable::new_compact(),
            device_config,
        }
    }

    pub const fn uid(&self) -> Uid {
        self.uid
    }

    pub const fn device_state(&self) -> DeviceState {
        self.state
    }

    pub const fn device_config(&self) -> &DeviceConfig {
        &self.device_config
    }

    // Simplified message handling - pass pre-existing RxInfo by reference
    async fn handle_message(&mut self, message: Message, rx_info: Option<&RxInfo>) {
        // Update link quality if rx info provided
        if let Some(info) = rx_info {
            let source_id = message.source_id().get();
            self.routing_table
                .update_link_quality(source_id, info.rssi, info.snr);

            // Create direct route to sender
            self.routing_table.update(
                source_id,
                Route::with_quality(
                    message.source_id(),
                    1,
                    calculate_quality(info.rssi, info.snr),
                ),
            );
        }

        // Process message based on destination
        if let Some(receiver) = message.destination_id() {
            if receiver.get() == self.uid.get() {
                // Message for us
                self.process_received_message(&message, rx_info);

                // Queue for application processing
                if let Err(e) = self.inqueue.enqueue(message.clone()) {
                    error!("Inqueue error: {:?}", e);
                }
            } else if !message.is_expired() {
                // Forward message
                self.route_message(message).await;
            }
        } else if !message.is_expired() {
            // Broadcast message - process and relay
            self.process_received_message(&message, rx_info);

            // Queue locally
            if let Err(e) = self.inqueue.enqueue(message.clone()) {
                error!("Inqueue broadcast error: {:?}", e);
            }

            // Relay to others (with TTL decrement)
            let mut relay = message;
            relay.decrement_ttl();
            if !relay.is_expired() {
                if let Err(e) = self.outqueue.enqueue(relay) {
                    error!("Outqueue broadcast error: {:?}", e);
                }
            }
        }
    }

    // Core message processing
    fn process_received_message(&mut self, message: &Message, rx_info: Option<&RxInfo>) {
        match message.payload() {
            Payload::Command(_) | Payload::Data(_) => {
                if message.req_ack() {
                    self.send_ack(message);
                }
            }
            Payload::Ack(ack) => {
                match ack {
                    AckType::Success { message_id } => {
                        if let Some(pending) = self.pending_acks.get_mut(message_id) {
                            pending.acknowledge();
                            self.routing_table
                                .record_successful_delivery(message.source_id().get());
                        }
                    }
                    AckType::AckDiscovered { hops, last_hop } => {
                        // Update route with discovery information
                        let mut route = Route::new(*last_hop, *hops);

                        if let Some(info) = rx_info {
                            route.quality = calculate_quality(info.rssi, info.snr);
                        }

                        self.routing_table.update(message.source_id().get(), route);

                        // Check if this was our discovery request
                        if message.destination_id() == Some(self.uid) {
                            if let Some(pending) = self.pending_acks.get_mut(&message.message_id())
                            {
                                pending.acknowledge();
                            }
                        }
                    }
                    AckType::Failure { message_id } => {
                        if let Some(pending) = self.pending_acks.get_mut(message_id) {
                            pending.acknowledge();

                            // Record routing failure
                            if let Some(dest_uid) = pending.destination_uid() {
                                if let Some(route) = self.routing_table.lookup_route(dest_uid.get())
                                {
                                    self.routing_table
                                        .record_failed_delivery(route.next_hop.get());
                                }
                            }
                        }
                    }
                }
            }
            Payload::Discovery(discovery) => {
                // Compute hop count
                let hops = discovery.original_ttl - message.ttl();

                // Send acknowledgment for discovery
                let ack_message = Message::new_ack(
                    self.uid,
                    Some(message.source_id()),
                    AckType::AckDiscovered {
                        hops,
                        last_hop: self.uid,
                    },
                    message.ttl(),
                    false,
                );

                if let Err(e) = self.outqueue.enqueue(ack_message) {
                    error!("Discovery ack enqueue error: {:?}", e);
                }
            }
            Payload::Route(_) => {
                // Handle route messages if needed
            }
        }
    }

    // Simplified ACK sender
    fn send_ack(&mut self, message: &Message) {
        let ack_message = Message::new_ack(
            self.uid,
            Some(message.source_id()),
            AckType::Success {
                message_id: message.message_id(),
            },
            message.ttl(),
            false,
        );

        let _ = self.outqueue.enqueue(ack_message);
    }

    // Simplified routing - returns success status rather than Result
    async fn route_message(&mut self, mut message: Message) -> bool {
        // Extract destination
        let destination_id = match message.destination_id() {
            Some(id) => id.get(),
            None => return false,
        };

        // Look up best route
        if let Some(route) = self.routing_table.lookup_route(destination_id) {
            // Prepare forwarded message
            message = Message::new(
                self.uid,
                Some(route.next_hop),
                message.payload().clone(),
                message.ttl() - 1, // Decrement TTL
                message.req_ack(),
            );

            // Skip if expired
            if message.is_expired() {
                return false;
            }

            // Transmit
            if self.tx_message(message).await.is_ok() {
                self.routing_table
                    .record_successful_delivery(route.next_hop.get());
                return true;
            }
        } else {
            // No route - initiate discovery if not already in progress
            if !self.is_route_discovery_in_progress(destination_id) {
                self.initiate_route_discovery(destination_id);
            }
        }

        false
    }

    // Check if route discovery is in progress
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

    // Start route discovery
    fn initiate_route_discovery(&mut self, destination: u8) {
        if let Some(dest_uid) = NonZeroU8::new(destination) {
            let message =
                Message::new_discovery(self.uid, Some(dest_uid), 3, true, self.device_config);

            let _ = self.outqueue.enqueue(message);
        }
    }

    // Process messages in inqueue
    fn process_inqueue(&mut self) {
        let to_process = cmp::min(self.inqueue.len(), MAX_INQUEUE_PROCESS);
        for _ in 0..to_process {
            if self.inqueue.dequeue().is_ok() {
                // Application layer handles the message content
            }
        }
    }

    // Process outqueue and send messages
    async fn process_outqueue(&mut self) {
        let to_transmit = cmp::min(self.outqueue.len(), MAX_OUTQUEUE_TRANSMIT);
        for _ in 0..to_transmit {
            if let Ok(message) = self.outqueue.dequeue() {
                // Track for acknowledgment if needed
                if message.req_ack() {
                    self.add_pending_ack(&message);
                }

                // Attempt transmission
                let _ = self.tx_message(message).await;
            }
        }
    }

    // Add pending acknowledgment
    fn add_pending_ack(&mut self, message: &Message) {
        let pending_ack = PendingAck::new(
            message.payload().clone(),
            message.destination_id(),
            message.ttl(),
        );

        let message_id = message.message_id();
        if !self.pending_acks.contains_key(&message_id) {
            let _ = self.pending_acks.insert(message_id, pending_ack);
        }
    }

    // Transmit a message
    async fn tx_message(&mut self, message: Message) -> Result<(), RadioError> {
        let buffer: [u8; 70] = message.into();
        let params = &mut self.lora_config.tx_pkt_params;

        self.state = DeviceState::Transmitting;

        self.radio
            .prepare_for_tx(
                &self.lora_config.modulation,
                params,
                self.lora_config.tx_power,
                &buffer,
            )
            .await?;

        // Add small delay
        Timer::after(Duration::from_millis(10)).await;

        // Transmit
        self.radio.tx().await?;

        self.state = DeviceState::Idle;
        Ok(())
    }

    // Start network discovery
    pub fn discover_nodes(&mut self) {
        let discovery_message = Message::new_discovery(self.uid, None, 3, true, self.device_config);

        let _ = self.outqueue.enqueue(discovery_message);
    }

    // Retry pending messages that need it
    fn retry_pending_messages(&mut self) {
        // Identify messages to retry
        let mut to_retry = Vec::<u32, 8>::new(); // Smaller buffer
        let mut to_remove = Vec::<u32, 8>::new(); // Smaller buffer

        // Scan pending acks
        for (id, ack) in &mut self.pending_acks {
            if ack.is_acknowledged {
                to_remove.push(*id).unwrap_or(());
            } else {
                let backoff_ms = calculate_backoff(ack.attempts);
                let now = Instant::now();

                if now.duration_since(ack.timestamp).as_millis() > backoff_ms {
                    if ack.attempts < MAX_ACK_ATTEMPTS {
                        // Ready to retry
                        to_retry.push(*id).unwrap_or(());
                        ack.increment_attempts();
                        ack.update_timestamp();
                    } else {
                        // Max attempts reached
                        to_remove.push(*id).unwrap_or(());

                        // Record failure
                        if let Some(dest_uid) = ack.destination_uid() {
                            if let Some(route) = self.routing_table.lookup_route(dest_uid.get()) {
                                self.routing_table
                                    .record_failed_delivery(route.next_hop.get());
                            }
                        }
                    }
                }
            }
        }

        // Retry messages
        for message_id in to_retry {
            self.retry_message(message_id);
        }

        // Remove completed/failed messages
        for id in to_remove {
            self.pending_acks.remove(&id);
        }
    }

    // Retry specific message
    fn retry_message(&mut self, message_id: u32) {
        if let Some(ack) = self.pending_acks.get(&message_id) {
            let message = Message::new(
                self.uid,
                ack.destination_uid(),
                ack.payload().clone(),
                ack.ttl(),
                true,
            );

            let _ = self.outqueue.enqueue(message);
        }
    }

    // Non-blocking listen with split RX approach
    async fn listen(&mut self, buf: &mut [u8]) {
        self.state = DeviceState::Receiving;

        // Prepare radio for RX (a bit shorter timeout)
        if let Err(e) = self
            .radio
            .prepare_for_rx(
                RxMode::Single(1000), // Shorter timeout for better responsiveness
                &self.lora_config.modulation,
                &self.lora_config.rx_pkt_params,
            )
            .await
        {
            error!("RX prep error: {:?}", e);
            self.state = DeviceState::Idle;
            return;
        }

        // Split the receive operation: start RX but don't wait for completion yet
        if let Err(e) = self.radio.start_rx().await {
            warn!("Start RX error: {:?}", e);
            self.state = DeviceState::Idle;
            return;
        }

        // Short yield to allow other tasks to run
        Timer::after(Duration::from_millis(1)).await;

        // Now complete RX with timeout to avoid indefinite blocking
        match embassy_time::with_timeout(
            Duration::from_millis(500),
            self.radio.complete_rx(&self.lora_config.rx_pkt_params, buf),
        )
        .await
        {
            Ok(Ok((size, status))) => {
                // Capture signal info
                let rx_info = RxInfo {
                    rssi: status.rssi,
                    snr: status.snr,
                };

                // Parse message
                if let Ok(message) = Message::try_from(&mut buf[..size as usize]) {
                    // Process with signal quality info
                    self.handle_message(message, Some(&rx_info)).await;
                }
            }
            Ok(Err(e)) => {
                warn!("RX error: {:?}", e);
            }
            Err(_) => {
                // Timeout on our end, not radio timeout
                // Just reset radio state and continue
            }
        }

        self.state = DeviceState::Idle;
    }

    // Periodic maintenance
    async fn perform_maintenance(&mut self) {
        // Retry pending messages
        self.retry_pending_messages();

        // Update routing table
        self.routing_table.cleanup();

        // Occasionally refresh routes
        let counter = STATS_COUNTER
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        if counter.is_multiple_of(50) {
            self.refresh_routes().await;
        }

        // Log stats occasionally (reduced frequency)
        if counter.is_multiple_of(100) {
            let stats = self.routing_table.stats();
            info!(
                "Routes: {} total, {} active, {} qual",
                stats.total_entries, stats.active_routes, stats.avg_quality
            );
        }
    }

    // Refresh routes that need it
    async fn refresh_routes(&mut self) {
        // Find a small batch of routes to refresh
        let mut count = 0;
        for dest_id in 1..=255u8 {
            if count >= 3 {
                break; // Limit to just a few at a time
            }

            if dest_id != self.uid.get()
                && NonZeroU8::new(dest_id).is_some()
                && self.routing_table.needs_refresh(dest_id)
            {
                self.initiate_route_discovery(dest_id);
                count += 1;

                // Small delay between discoveries
                Timer::after(Duration::from_millis(50)).await;
            }
        }
    }
}

// Cooperative main loop that properly yields to other tasks
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
    let mut last_maintenance = Instant::now();
    let mut last_discovery = Instant::now();

    // Log that we're starting
    info!("Starting QuadraNet device on {}", device.uid().get());

    // Initial discovery
    device.discover_nodes();

    // Main cooperative scheduling loop
    loop {
        // Explicit yield point to allow other tasks to run
        Timer::after(Duration::from_millis(1)).await;

        // Listen for incoming messages (now non-blocking)
        device.listen(buf).await;

        // Process queues with yield points
        device.process_inqueue();
        Timer::after(Duration::from_millis(1)).await;

        device.process_outqueue().await;
        Timer::after(Duration::from_millis(1)).await;

        // Periodic maintenance (less frequent)
        if Instant::now().duration_since(last_maintenance) > Duration::from_millis(2000) {
            device.perform_maintenance().await;
            last_maintenance = Instant::now();
        }

        // Occasional network discovery refresh
        if Instant::now().duration_since(last_discovery) > Duration::from_secs(60) {
            info!("Performing network discovery refresh");
            device.discover_nodes();
            last_discovery = Instant::now();
        }
    }
}

// Backoff calculation helper
#[inline]
fn calculate_backoff(attempt: u8) -> u64 {
    if attempt == 0 {
        return INITIAL_BACKOFF_MS;
    }

    let backoff = INITIAL_BACKOFF_MS * BACKOFF_FACTOR.pow(u32::from(attempt));
    backoff.min(MAX_BACKOFF_MS)
}

// Signal quality helper
#[inline]
fn calculate_quality(rssi: i16, snr: i16) -> u8 {
    // Simplified quality calculation
    let rssi_norm = ((rssi + 130) * 2).clamp(0, 255) as u16;
    let snr_norm = ((snr + 20) * 4).clamp(0, 255) as u16;

    ((rssi_norm + snr_norm * 3) / 4) as u8
}

// Helper to access device state
pub const fn device_state<RK, DLY, IN, OUT>(device: &LoraDevice<RK, DLY, IN, OUT>) -> DeviceState
where
    RK: RadioKind,
    DLY: DelayNs,
    IN: MessageQueue + 'static,
    OUT: MessageQueue + 'static,
{
    device.device_state()
}
