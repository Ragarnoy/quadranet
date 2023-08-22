use core::num::NonZeroU16;

pub struct Message {
    pub sender_uid: u16,
    receiver_uid: Option<NonZeroU16>,
    length: u8,
    content: [u8; 64],
    ttl: u8,
    sequence_number: u32,
}

impl Message {
    pub fn new(
        sender_uid: u16,
        receiver_uid: Option<NonZeroU16>,
        content: [u8; 64],
        ttl: u8,
        sequence_number: u32,
    ) -> Self {
        let length = content.len() as u8;
        Self {
            sender_uid,
            receiver_uid,
            length,
            content,
            ttl,
            sequence_number,
        }
    }
}

impl From<Message> for [u8; 74] {
    // Adjusted size to 74 bytes
    fn from(message: Message) -> Self {
        let mut bytes = [0u8; 74];

        // Convert sender_uid to bytes and copy to the array
        let sender_bytes = message.sender_uid.to_le_bytes();
        bytes[0..2].copy_from_slice(&sender_bytes);

        // Convert receiver_uid to bytes and copy to the array
        let receiver_bytes = message
            .receiver_uid
            .map_or([0u8; 2], |uid| uid.get().to_le_bytes());
        bytes[2..4].copy_from_slice(&receiver_bytes);

        // Copy length
        bytes[4] = message.length;

        // Copy content
        bytes[5..69].copy_from_slice(&message.content);

        // Copy ttl
        bytes[69] = message.ttl;

        // Copy sequence_number
        let sequence_bytes = message.sequence_number.to_le_bytes();
        bytes[70..74].copy_from_slice(&sequence_bytes);

        bytes
    }
}

impl TryFrom<&[u8]> for Message {
    type Error = &'static str;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != 74 {
            return Err("Invalid byte slice length");
        }

        // Deserialize sender_uid
        let sender_uid = u16::from_le_bytes([bytes[0], bytes[1]]);

        // Deserialize receiver_uid
        let receiver_uid_bytes = [bytes[2], bytes[3]];
        let receiver_uid = if receiver_uid_bytes == [0u8; 2] {
            None
        } else {
            Some(
                NonZeroU16::new(u16::from_le_bytes(receiver_uid_bytes))
                    .ok_or("Invalid receiver UID")?,
            )
        };

        // Deserialize length
        let length = bytes[4];

        // Deserialize content
        let mut content = [0u8; 64];
        content.copy_from_slice(&bytes[5..69]);

        // Deserialize ttl
        let ttl = bytes[69];

        // Deserialize sequence_number
        let sequence_number = u32::from_le_bytes([bytes[70], bytes[71], bytes[72], bytes[73]]);

        Ok(Self {
            sender_uid,
            receiver_uid,
            length,
            content,
            ttl,
            sequence_number,
        })
    }
}
