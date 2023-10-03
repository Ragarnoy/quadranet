use crate::device::Uid;
use crate::message::error::MessageError;

pub enum Intent {
    Data,
    Discover,
    Ping,
    Query,
    Command,
    Information,
    Error,
}

pub struct Content {
    intent: Intent,
    content: [u8; 63],
}

impl Content {
    pub fn discover(route: u16) -> Self {
        let mut content = [0u8; 63];
        content[..3].copy_from_slice(&route.to_be_bytes());
        Self { intent: Intent::Discover, content }
    }

    pub fn ping(uid: Uid) -> Self {
        let mut content = [0u8; 63];
        content[..3].copy_from_slice(&uid.get().to_be_bytes());
        Self { intent: Intent::Ping, content }
    }


}

impl TryFrom<&[u8]> for Content {
    type Error = MessageError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let intent = match bytes[0] {
                0 => Intent::Data,
                1 => Intent::Discover,
                2 => Intent::Ping,
                3 => Intent::Query,
                4 => Intent::Command,
                5 => Intent::Information,
                6 => Intent::Error,
                _ => return Err(MessageError::InvalidIntent),
        };


        let mut content = [0u8; 63];
        content.copy_from_slice(&bytes[1..]);

        Ok(Self { intent, content })
    }
}