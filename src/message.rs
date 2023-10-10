pub mod content;
pub mod error;
pub mod intent;

use core::fmt::Display;
use crate::device::Uid;
use core::num::NonZeroU8;
use crate::message::error::MessageError;
use crate::message::intent::Intent;
use core::convert::TryFrom;
use defmt::Format;

pub const MESSAGE_SIZE: usize = 70;  // Adjusted size

#[derive(Debug, Clone, Format)]
pub struct Message {
    pub intent: Intent,
    pub sender_uid: Uid,
    pub receiver_uid: Option<Uid>,
    pub next_hop: Option<Uid>,
    length: u8,
    pub ttl: u8,
    pub content: [u8; 64],
}

impl Display for Message {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "----------------------------------------")?;
        writeln!(f, "| Field        | Value                |")?;
        writeln!(f, "|--------------|----------------------|")?;
        writeln!(f, "| Intent       | {:<20?} |", self.intent)?;
        writeln!(f, "| Sender UID   | {:<20} |", self.sender_uid.get())?;
        writeln!(f, "| Receiver UID | {:<20} |", self.receiver_uid.map_or(0, |uid| uid.get()))?;
        writeln!(f, "| Next Hop     | {:<20} |", self.next_hop.map_or(0, |uid| uid.get()))?;
        writeln!(f, "| Length       | {:<20} |", self.length)?;
        writeln!(f, "| ttl          | {:<20} |", self.ttl)?;
        writeln!(f, "| Content      | {:?} |", &self.content[0..self.length as usize])?;
        writeln!(f, "----------------------------------------")
    }
}

impl Message {
    // General constructor
    fn new(intent: Intent, sender_uid: Uid, receiver_uid: Option<Uid>, content: [u8; 64]) -> Self {
        let length = content.len() as u8;
        Self {
            intent,
            sender_uid,
            receiver_uid,
            next_hop: None,
            length,
            ttl: 0,
            content,
        }
    }

    /// Returns the length of the message
    pub fn length(&self) -> u8 {
        self.length
    }

    /// Specialized constructor for Ping
    pub fn ping(sender_uid: Uid) -> Self {
        Self::new(Intent::Ping, sender_uid, None, [0u8; 64])
    }

    pub fn pong(sender_uid: Uid, receiver_uid: Uid) -> Self {
        Self::new(Intent::Pong, sender_uid, Some(receiver_uid), [0u8; 64])
    }

    // Specialized constructor for Data
    pub fn data(sender_uid: Uid, receiver_uid: Uid, content: [u8; 64]) -> Self {
        Self::new(Intent::Data, sender_uid, Some(receiver_uid), content)
    }

    // Specialized constructor for Discover
    pub fn discover(sender_uid: Uid, depth: u8) -> Self {
        let mut content = [0u8; 64];
        content[0] = depth; // Store the depth in the first byte of the content
        Self::new(Intent::Discover, sender_uid, None, content)
    }

    // Specialized constructor for Information
    // Placeholder: Requires implementation of information-specific logic
    pub fn information(sender_uid: Uid) -> Self {
        Self::new(Intent::Information, sender_uid, None, [0u8; 64])
    }

    // Specialized constructor for Error
    pub fn error(sender_uid: Uid, content: [u8; 64]) -> Self {
        Self::new(Intent::Error, sender_uid, None, content)
    }
}


impl From<Message> for [u8; MESSAGE_SIZE] {
    fn from(message: Message) -> Self {
        let mut bytes = [0u8; MESSAGE_SIZE];

        // Convert intent to bytes and copy to the array
        bytes[0] = message.intent as u8;

        // Convert sender_uid to bytes and copy to the array
        bytes[1] = message.sender_uid.get();

        // Convert receiver_uid to bytes and copy to the array
        bytes[2] = message.receiver_uid.map_or(0, |uid| uid.get());

        // Convert next_hop to bytes and copy to the array
        bytes[3] = message.next_hop.map_or(0, |uid| uid.get());

        // Copy length
        bytes[4] = message.length;

        // Copy ttl
        bytes[5] = message.ttl;

        // Copy content
        bytes[6..70].copy_from_slice(&message.content);

        bytes
    }
}

impl TryFrom<&[u8]> for Message {
    type Error = MessageError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != MESSAGE_SIZE {
            return Err(MessageError::InvalidLength);
        }

        // Deserialize intent
        let intent = Intent::try_from(bytes[0])?;

        // Deserialize sender_uid
        let sender_uid = NonZeroU8::new(bytes[1])
            .ok_or(MessageError::InvalidUid)?;

        // Deserialize receiver_uid
        let receiver_uid = if bytes[2] == 0 {
            None
        } else {
            Some(NonZeroU8::new(bytes[2])
                .ok_or(MessageError::InvalidUid)?)
        };

        // Deserialize next_hop
        let next_hop = if bytes[3] == 0 {
            None
        } else {
            Some(NonZeroU8::new(bytes[3])
                .ok_or(MessageError::InvalidUid)?)
        };

        // Deserialize length
        let length = bytes[4];

        // Deserialize ttl
        let ttl = bytes[5];

        // Deserialize content
        let mut content = [0u8; 64];
        content.copy_from_slice(&bytes[6..70]);

        Ok(Self {
            intent,
            sender_uid,
            receiver_uid,
            next_hop,
            length,
            ttl,
            content,
        })
    }
}

