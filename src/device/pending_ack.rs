use crate::device::Uid;
use crate::message::payload::Payload;
use embassy_time::Instant;

pub const MAX_PENDING_ACKS: usize = 32;
pub const ACK_WAIT_TIME: u64 = 5;
pub const MAX_ACK_ATTEMPTS: u8 = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct PendingAck {
    pub timestamp: Instant,    // When the message was sent
    pub attempts: u8,          // Number of transmission attempts
    pub is_acknowledged: bool, // Whether an ACK has been received
    payload: Payload,          // Minimal information needed to recreate the message
    destination_uid: Option<Uid>,
    ttl: u8,
}

impl PendingAck {
    /// Creates a new pending acknowledgment tracker
    pub fn new(payload: Payload, destination_uid: Option<Uid>, ttl: u8) -> Self {
        Self {
            timestamp: Instant::now(),
            attempts: 0,
            is_acknowledged: false,
            payload,
            destination_uid,
            ttl,
        }
    }

    /// Returns the payload of the message
    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    /// Returns the destination UID of the message
    pub fn destination_uid(&self) -> Option<Uid> {
        self.destination_uid
    }

    /// Returns the time-to-live value of the message
    pub fn ttl(&self) -> u8 {
        self.ttl
    }

    /// Increments the attempt counter
    pub fn increment_attempts(&mut self) {
        self.attempts += 1;
    }

    /// Checks if the ACK has timed out based on the default timeout
    pub fn is_expired(&self) -> bool {
        self.timestamp.elapsed().as_secs() > ACK_WAIT_TIME
    }

    /// Checks if the maximum number of retry attempts has been reached
    pub fn is_max_attempts(&self) -> bool {
        self.attempts >= MAX_ACK_ATTEMPTS
    }

    /// Updates the timestamp to now
    pub fn update_timestamp(&mut self) {
        self.timestamp = Instant::now();
    }

    /// Mark this pending ACK as acknowledged
    pub fn acknowledge(&mut self) {
        self.is_acknowledged = true;
    }
}
