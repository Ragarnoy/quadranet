use defmt::Format;
use crate::message::error::MessageError;

#[derive(Debug, Clone, Copy, Format)]
pub enum Intent {
    Data,
    Discover,
    Ping,
    Pong,
    Information,
    Error,
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
            _ => Err(MessageError::InvalidIntent),
        }
    }
}