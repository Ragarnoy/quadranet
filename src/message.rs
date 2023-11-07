pub mod content;
pub mod error;
pub mod intent;

use crate::device::Uid;
use crate::message::error::MessageError;
use crate::message::intent::Intent;
use core::convert::TryFrom;
use core::mem::size_of;
use core::num::NonZeroU8;

const INTENT_SIZE: usize = size_of::<u8>(); // Assuming Intent is stored as u8
const UID_SIZE: usize = size_of::<Uid>();
const LENGTH_SIZE: usize = size_of::<u8>();
const TTL_SIZE: usize = size_of::<u8>();
const CONTENT_SIZE: usize = size_of::<[u8; 64]>();

// Calculate the total message size
const CALCULATED_MESSAGE_SIZE: usize =
    INTENT_SIZE + 3 * UID_SIZE + LENGTH_SIZE + TTL_SIZE + CONTENT_SIZE;

// Compile-time assertion to check if MESSAGE_SIZE matches the calculated size
pub const MESSAGE_SIZE: usize = CALCULATED_MESSAGE_SIZE;

#[derive(Debug, Clone)]
pub struct Message {
    pub intent: Intent,
    pub sender_uid: Uid,
    pub receiver_uid: Option<Uid>,
    pub next_hop: Option<Uid>,
    length: u8,
    pub ttl: u8,
    pub content: [u8; 64],
}

impl defmt::Format for Message {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "Message {{\n");
        defmt::write!(f, "    intent: {:?},\n", self.intent);
        defmt::write!(f, "    sender_uid: {:?},\n", self.sender_uid);
        defmt::write!(f, "    receiver_uid: {:?},\n", self.receiver_uid);
        defmt::write!(f, "    next_hop: {:?},\n", self.next_hop);
        defmt::write!(f, "    length: {:?},\n", self.length);
        defmt::write!(f, "    ttl: {:?},\n", self.ttl);
        // Ensure that the content slice is within bounds before printing
        let content_end = core::cmp::min(self.content.len(), self.length as usize);
        defmt::write!(f, "    content: {:?},\n", &self.content[0..content_end]);
        defmt::write!(f, "}}\n");
    }
}


impl Message {
    // General constructor
    fn new(intent: Intent, sender_uid: Uid, receiver_uid: Option<Uid>, content: [u8; 64]) -> Self {
        let length = content.iter().take_while(|byte| **byte != 0).count() as u8;
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
        let sender_uid = NonZeroU8::new(bytes[1]).ok_or(MessageError::InvalidUid)?;

        // Deserialize receiver_uid
        let receiver_uid = if bytes[2] == 0 {
            None
        } else {
            Some(NonZeroU8::new(bytes[2]).ok_or(MessageError::InvalidUid)?)
        };

        // Deserialize next_hop
        let next_hop = if bytes[3] == 0 {
            None
        } else {
            Some(NonZeroU8::new(bytes[3]).ok_or(MessageError::InvalidUid)?)
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

#[cfg(test)]
mod test {
    use crate::device::Uid;
    use crate::message::intent::Intent;
    use crate::message::Message;
    use core::convert::TryFrom;
    use core::num::NonZeroU8;

    #[test]
    fn test_serialize_message() {
        let sender_uid = Uid::try_from(NonZeroU8::new(0x01).unwrap()).unwrap();
        let receiver_uid = Uid::try_from(NonZeroU8::new(0x02).unwrap()).unwrap();
        let mut content: [u8; 64] = [0x0; 64];
        content[0..4].copy_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        let message = Message::data(sender_uid, receiver_uid, content);
        let bytes: [u8; 70] = message.into();
        assert_eq!(bytes[0], Intent::Data as u8);
        assert_eq!(bytes[1], sender_uid.get());
        assert_eq!(bytes[2], receiver_uid.get());
        assert_eq!(bytes[3], 0);
        assert_eq!(bytes[4], 4);
        assert_eq!(bytes[5], 0);
        assert_eq!(bytes[6..10], [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_deserialize_message() {
        let sender_uid = Uid::try_from(NonZeroU8::new(0x01).unwrap()).unwrap();
        let receiver_uid = Uid::try_from(NonZeroU8::new(0x02).unwrap()).unwrap();
        let mut content: [u8; 64] = [0x0; 64];
        content[0..4].copy_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        let message = Message::data(sender_uid, receiver_uid, content);
        let bytes: [u8; 70] = message.into();
        let deserialized_message = Message::try_from(&bytes[..]).unwrap();
        assert_eq!(deserialized_message.intent, Intent::Data);
        assert_eq!(deserialized_message.sender_uid, sender_uid);
        assert_eq!(deserialized_message.receiver_uid, Some(receiver_uid));
        assert_eq!(deserialized_message.length, 4);
        assert_eq!(deserialized_message.ttl, 0);
        assert_eq!(deserialized_message.content, content);
    }

    #[test]
    fn test_message_ping() {
        let sender_uid = Uid::try_from(NonZeroU8::new(0x01).unwrap()).unwrap();
        let message = Message::ping(sender_uid);
        assert_eq!(message.intent, Intent::Ping, "Ping intent");
        assert_eq!(message.sender_uid, sender_uid);
        assert_eq!(message.receiver_uid, None);
        assert_eq!(message.length, 0);
        assert_eq!(message.ttl, 0);
        assert_eq!(message.content, [0u8; 64]);
    }

    #[test]
    fn test_message_data() {
        let sender_uid = Uid::try_from(NonZeroU8::new(0x01).unwrap()).unwrap();
        let receiver_uid = Uid::try_from(NonZeroU8::new(0x02).unwrap()).unwrap();
        let mut content: [u8; 64] = [0x0; 64];
        content[0..4].copy_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        let message = Message::data(sender_uid, receiver_uid, content);
        assert_eq!(message.intent, Intent::Data);
        assert_eq!(message.sender_uid, sender_uid);
        assert_eq!(message.receiver_uid, Some(receiver_uid));
        assert_eq!(message.length, 4);
        assert_eq!(message.ttl, 0);
        assert_eq!(message.content, content);
    }
}
