pub mod error;
pub mod payload;

use crate::device::Uid;
use crate::message::error::MessageError;
use core::convert::TryFrom;
use payload::Payload;

const MAX_TTL: u8 = 10;
const MAX_MESSAGE_SIZE: usize = 70;

#[derive(Debug, PartialEq, bitcode::Encode, bitcode::Decode)]
pub struct Message {
    /// Source ID is the UID of the node that sent the message
    source_id: Uid,
    /// Destination ID is the UID of the node the message is intended for
    destination_id: Option<Uid>,
    /// Payload is the data being sent
    payload: Payload,
    /// Time to live is the number of hops a message can take before it is considered expired
    ttl: u8,
}

impl Message {
    pub fn new(source_id: Uid, destination_id: Option<Uid>, payload: Payload, ttl: u8) -> Self {
        Self {
            source_id,
            destination_id,
            payload,
            ttl: ttl.min(MAX_TTL),
        }
    }
    pub fn source_id(&self) -> Uid {
        self.source_id
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
        self.destination_id == Some(uid)
    }

    fn serialize(&self) -> [u8; MAX_MESSAGE_SIZE] {
        // Serialize the message fields to a byte array
        bitcode::encode(&self).unwrap().as_slice().try_into().unwrap()
    }

    fn deserialize(data: &[u8]) -> Result<Self, MessageError> {
        // Deserialize the byte array into a Message
        if let Ok(message) = bitcode::decode::<Message>(data) {
            Ok(message)
        } else {
            Err(MessageError::InvalidMessage)
        }
    }
}

impl TryFrom<&[u8]> for Message {
    type Error = MessageError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        Self::deserialize(data)
    }
}

impl From<Message> for [u8; MAX_MESSAGE_SIZE] {
    fn from(message: Message) -> Self {
        message.serialize()
    }
}