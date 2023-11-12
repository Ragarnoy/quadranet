use core::mem::size_of;
use crate::message::content::Content;

const PING_SIZE: usize = size_of::<PingContent>();

pub struct PingContent;

impl Content for PingContent {
    const SIZE: usize = PING_SIZE;

    fn as_bytes(&self) -> &[u8; Self::SIZE] {
        &[]
    }

    fn from_bytes(_bytes: &[u8; Self::SIZE]) -> Self {
        PingContent
    }
}