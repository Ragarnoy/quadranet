use core::convert::TryFrom;

use defmt::Format;
use serde::{Deserialize, Serialize};

use payload::Payload;

use crate::device::Uid;
use crate::message::error::MessageError;

pub mod error;
pub mod payload;

#[cfg(test)]
mod test;

const MAX_TTL: u8 = 10;
const MAX_MESSAGE_SIZE: usize = 70;
static mut MESSAGE_ID_COUNTER: u32 = 0;

fn generate_message_id() -> u32 {
    unsafe {
        let id = MESSAGE_ID_COUNTER;
        MESSAGE_ID_COUNTER = MESSAGE_ID_COUNTER.wrapping_add(1);
        id
    }
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Format)]
pub struct Message {
    message_id: u32,
    /// Source ID is the UID of the node that sent the message
    source_id: Uid,
    /// Destination ID is the UID of the node the message is intended for
    destination_id: Option<Uid>,
    /// Time to live is the number of hops a message can take before it is considered expired
    ttl: u8,
    /// Req ack is a flag that indicates if the message requires an acknowledgement
    req_ack: bool,
    /// Payload is the data being sent
    payload: Payload,
}

impl Message {
    pub fn new(source_id: Uid, destination_id: Option<Uid>, payload: Payload, ttl: u8, require_ack: bool) -> Self {
        Self {
            message_id: generate_message_id(),
            source_id,
            destination_id,
            payload,
            req_ack: require_ack,
            ttl: ttl.min(MAX_TTL),
        }
    }

    pub fn source_id(&self) -> Uid {
        self.source_id
    }

    pub fn message_id(&self) -> u32 {
        self.message_id
    }

    pub fn set_message_id(&mut self, message_id: u32) {
        self.message_id = message_id;
    }

    pub fn req_ack(&self) -> bool {
        self.req_ack
    }

    pub fn destination_id(&self) -> Option<Uid> {
        self.destination_id
    }

    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    pub fn ttl(&self) -> u8 {
        self.ttl
    }

    pub fn decrement_ttl(&mut self) {
        self.ttl = self.ttl.saturating_sub(1);
    }

    pub fn is_expired(&self) -> bool {
        self.ttl == 0
    }

    pub fn is_for_me(&self, uid: Uid) -> bool {
        self.destination_id == Some(uid) || self.destination_id.is_none()
    }
}

impl TryFrom<&mut [u8]> for Message {
    type Error = MessageError;

    fn try_from(data: &mut [u8]) -> Result<Self, Self::Error> {
        postcard::from_bytes_cobs(data).map_err(|_| MessageError::DeserializationError)
    }
}

impl From<Message> for [u8; MAX_MESSAGE_SIZE] {
    fn from(message: Message) -> Self {
        let mut data = [0; MAX_MESSAGE_SIZE];
        let _ = postcard::to_slice_cobs(&message, &mut data);
        data
    }
}
