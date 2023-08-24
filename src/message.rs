use crate::device::Uid;
use core::num::NonZeroU16;
use snafu::Snafu;

const MESSAGE_SIZE: usize = 74;

pub struct Message {
    pub sender_uid: Uid,
    pub receiver_uid: Option<Uid>,
    pub next_hop: Option<Uid>,
    length: u8,
    content: [u8; 64],
    ttl: u8,
    sequence_number: u16,
}

impl Message {
    pub fn new(
        sender_uid: Uid,
        receiver_uid: Option<Uid>,
        content: [u8; 64],
        ttl: u8,
        sequence_number: u16,
    ) -> Self {
        let length = content.len() as u8;
        Self {
            sender_uid,
            receiver_uid,
            next_hop: None,
            length,
            content,
            ttl,
            sequence_number,
        }
    }

    pub fn new_discovery(sender_uid: Uid, sequence_number: u16) -> Self {
        Self::new(sender_uid, None, [0u8; 64], 0, sequence_number)
    }
}

impl From<Message> for [u8; 74] {
    // Adjusted size to 74 bytes
    fn from(message: Message) -> Self {
        let mut bytes = [0u8; 74];

        // Convert sender_uid to bytes and copy to the array
        let sender_bytes = message.sender_uid.get().to_le_bytes();
        bytes[0..2].copy_from_slice(&sender_bytes);

        // Convert receiver_uid to bytes and copy to the array
        let receiver_bytes = message
            .receiver_uid
            .map_or([0u8; 2], |uid| uid.get().to_le_bytes());
        bytes[2..4].copy_from_slice(&receiver_bytes);

        let next_hop_bytes = message
            .next_hop
            .map_or([0u8; 2], |uid| uid.get().to_le_bytes());
        bytes[4..6].copy_from_slice(&next_hop_bytes);

        // Copy length
        bytes[6] = message.length;

        // Copy content
        bytes[7..71].copy_from_slice(&message.content);

        // Copy ttl
        bytes[71] = message.ttl;

        // Copy sequence_number
        let sequence_bytes = message.sequence_number.to_le_bytes();
        bytes[72..=74].copy_from_slice(&sequence_bytes);

        bytes
    }
}

impl TryFrom<&[u8]> for Message {
    type Error = MessageError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != 74 {
            return Err(MessageError::InvalidLength);
        }

        // Deserialize sender_uid
        let sender_uid = NonZeroU16::new(u16::from_le_bytes([bytes[0], bytes[1]]))
            .ok_or(MessageError::InvalidUid)?;

        // Deserialize receiver_uid
        let receiver_uid_bytes = [bytes[2], bytes[3]];
        let receiver_uid = if receiver_uid_bytes == [0u8; 2] {
            None
        } else {
            Some(
                NonZeroU16::new(u16::from_le_bytes(receiver_uid_bytes))
                    .ok_or(MessageError::InvalidUid)?,
            )
        };

        let next_hop = [bytes[4], bytes[5]];
        let next_hop = if next_hop == [0u8; 2] {
            None
        } else {
            Some(
                NonZeroU16::new(u16::from_le_bytes(next_hop))
                    .ok_or(MessageError::InvalidUid)?,
            )
        };

        // Deserialize length
        let length = bytes[6];

        // Deserialize content
        let mut content = [0u8; 64];
        content.copy_from_slice(&bytes[7..72]);

        // Deserialize ttl
        let ttl = bytes[72];

        // Deserialize sequence_number
        let sequence_number = u16::from_le_bytes([bytes[72], bytes[73]]);

        Ok(Self {
            sender_uid,
            receiver_uid,
            next_hop,
            length,
            content,
            ttl,
            sequence_number,
        })
    }
}

#[derive(Debug, Snafu)]
pub enum MessageError {
    #[snafu(display("Invalid UID"))]
    InvalidUid,
    #[snafu(display("Invalid length"))]
    InvalidLength,
    #[snafu(display("Invalid TTL"))]
    InvalidTtl,
    #[snafu(display("Invalid sequence number"))]
    InvalidSequenceNumber,
}