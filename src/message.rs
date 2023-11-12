pub mod content;
pub mod error;
pub mod intent;
mod test;

use crate::device::Uid;
use crate::message::error::MessageError;
use crate::message::intent::Intent;
use core::convert::TryFrom;
use core::mem::size_of;
use crate::message::content::Content;
use crate::message::content::data::DataContent;

const INTENT_SIZE: usize = size_of::<u8>();
const UID_SIZE: usize = size_of::<Uid>();
const LENGTH_SIZE: usize = size_of::<u8>();
const TTL_SIZE: usize = size_of::<u8>();
const CONTENT_SIZE: usize = size_of::<[u8; 64]>();

// Calculate the total message size
const CALCULATED_MESSAGE_SIZE: usize =
    INTENT_SIZE + 3 * UID_SIZE + LENGTH_SIZE + TTL_SIZE + CONTENT_SIZE;

/// Compile-time assertion to check if MESSAGE_SIZE matches the calculated size
//noinspection RsAssertEqual
pub const MESSAGE_SIZE: usize =
    {
        assert!(CALCULATED_MESSAGE_SIZE == size_of::<Message<DataContent>>());
        CALCULATED_MESSAGE_SIZE
    };

#[derive(Debug, Clone)]
pub struct Message<C: Content> {
    pub intent: Intent,
    pub sender_uid: Uid,
    pub receiver_uid: Option<Uid>,
    pub next_hop: Option<Uid>,
    pub ttl: u8,
    pub content: C,
}

impl<C: Content> From<Message<C>> for [u8; MESSAGE_SIZE] {
    fn from(message: Message<C>) -> Self {
        let mut bytes = [0u8; MESSAGE_SIZE];
        bytes[0] = message.intent as u8;
        bytes[1] = message.sender_uid.get();
        bytes[2] = message.receiver_uid.map_or(0, |uid| uid.get());
        bytes[3] = message.next_hop.map_or(0, |uid| uid.get());
        // No need to store length as it's implied by C::SIZE
        bytes[4] = message.ttl;
        bytes[5..(5 + C::SIZE)].copy_from_slice(message.content.as_bytes());
        bytes
    }
}

impl<C: Content> TryFrom<&[u8]> for Message<C> {
    type Error = MessageError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != MESSAGE_SIZE {
            return Err(MessageError::InvalidLength);
        }

        let intent = Intent::try_from(bytes[0])?;
        let sender_uid = Uid::new(bytes[1]).unwrap();
        let receiver_uid = Uid::new(bytes[2]);
        let next_hop = Uid::new(bytes[3]);
        let ttl = bytes[4];

        let content_array: &[u8; C::SIZE] = bytes[5..5 + C::SIZE].try_into().map_err(|_| MessageError::InvalidContent)?;
        let content = C::from_bytes(content_array);

        Ok(Self {
            intent,
            sender_uid,
            receiver_uid,
            next_hop,
            ttl,
            content,
        })
    }
}


impl<C: Content> defmt::Format for Message<C> {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "Message {{\n");
        defmt::write!(f, "    intent: {:?},\n", self.intent);
        defmt::write!(f, "    sender_uid: {:?},\n", self.sender_uid);
        defmt::write!(f, "    receiver_uid: {:?},\n", self.receiver_uid);
        defmt::write!(f, "    next_hop: {:?},\n", self.next_hop);
        defmt::write!(f, "    ttl: {:?},\n", self.ttl);
        // Ensure that the content slice is within bounds before printing
        let content_end = core::cmp::min(self.content.len(), Content::SIZE);
        defmt::write!(f, "    content: {:?},\n", &self.content[0..content_end]);
        defmt::write!(f, "}}\n");
    }
}


impl<C: Content> Message<C> {
    fn new(intent: Intent, sender_uid: Uid, receiver_uid: Option<Uid>, content: [u8; 64]) -> Self {
        Self {
            intent,
            sender_uid,
            receiver_uid,
            next_hop: None,
            ttl: 0,
            content,
        }
    }
}
