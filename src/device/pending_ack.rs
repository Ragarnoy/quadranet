use embassy_time::Instant;
use crate::device::Uid;
use crate::message::payload::Payload;


pub const MAX_PENDING_ACKS: usize = 32;
pub const ACK_WAIT_TIME: u64 = 5;
pub const MAX_ACK_ATTEMPTS: u8 = 5;

#[derive(Clone, Debug, PartialEq)]
pub struct PendingAck {
    pub timestamp: Instant,
    pub attempts: u8,
    pub is_acknowledged: bool,
    payload: Payload,  // Minimal information needed to recreate the message
    destination_uid: Option<Uid>,
    ttl: u8,
}

impl PendingAck {
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

    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    pub fn destination_uid(&self) -> Option<Uid> {
        self.destination_uid
    }

    pub fn ttl(&self) -> u8 {
        self.ttl
    }

    pub fn increment_attempts(&mut self) {
        self.attempts += 1;
    }

    pub fn is_expired(&self) -> bool {
        self.timestamp.elapsed().as_secs() > ACK_WAIT_TIME
    }

    pub fn is_max_attempts(&self) -> bool {
        self.attempts >= MAX_ACK_ATTEMPTS
    }
}

