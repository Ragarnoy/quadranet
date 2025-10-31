use crate::device::Uid;
use crate::message::payload::Payload;
use embassy_time::Instant;

// Reduce buffer sizes to save memory
pub const MAX_PENDING_ACKS: usize = 8; // Reduced from 32
pub const ACK_WAIT_TIME: u64 = 5;
pub const MAX_ACK_ATTEMPTS: u8 = 3; // Reduced from 5

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingAck {
    pub timestamp: Instant,       // When the message was sent
    pub attempts: u8,             // Number of transmission attempts
    pub is_acknowledged: bool,    // Whether an ACK has been received
    payload: Payload,             // Message payload
    destination_uid: Option<Uid>, // Destination
    ttl: u8,                      // Time-to-live
}

impl PendingAck {
    /// Creates a new pending acknowledgment tracker
    #[inline]
    #[must_use]
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

    /// Returns the payload
    #[inline]
    #[must_use]
    pub const fn payload(&self) -> &Payload {
        &self.payload
    }

    /// Returns the destination UID
    #[inline]
    #[must_use]
    pub const fn destination_uid(&self) -> Option<Uid> {
        self.destination_uid
    }

    /// Returns the TTL
    #[inline]
    #[must_use]
    pub const fn ttl(&self) -> u8 {
        self.ttl
    }

    /// Increments attempt counter
    #[inline]
    pub const fn increment_attempts(&mut self) {
        self.attempts += 1;
    }

    /// Marks as acknowledged
    #[inline]
    pub const fn acknowledge(&mut self) {
        self.is_acknowledged = true;
    }

    /// Updates timestamp
    #[inline]
    pub fn update_timestamp(&mut self) {
        self.timestamp = Instant::now();
    }

    /// Checks if max attempts reached
    #[inline]
    pub const fn is_max_attempts(&self) -> bool {
        self.attempts >= MAX_ACK_ATTEMPTS
    }
}
