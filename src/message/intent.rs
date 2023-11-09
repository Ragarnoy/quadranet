use crate::message::error::MessageError;
use defmt::Format;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum Intent {
    Data,
    Discover,
    Ping,
    Pong,
    Information,
    Error,
    Ack,
}

impl TryFrom<u8> for Intent {
    type Error = MessageError;

    fn try_from(byte: u8) -> Result<Self, <Self as TryFrom<u8>>::Error> {
        match byte {
            0 => Ok(Intent::Data),
            1 => Ok(Intent::Discover),
            2 => Ok(Intent::Ping),
            3 => Ok(Intent::Pong),
            4 => Ok(Intent::Information),
            5 => Ok(Intent::Error),
            6 => Ok(Intent::Ack),
            _ => Err(MessageError::InvalidIntent),
        }
    }
}

