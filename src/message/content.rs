use crate::message::error::MessageError;

const SIZE: usize = 64;

pub struct Content {
    buffer: [u8; SIZE],
}

impl Content {
    pub const SIZE: usize = SIZE;
}

impl TryFrom<&[u8]> for Content {
    type Error = MessageError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let mut content = [0u8; Self::SIZE];
        content.copy_from_slice(&bytes[1..]);

        Ok(Self { buffer: content })
    }
}
