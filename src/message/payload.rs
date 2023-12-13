use core::mem::size_of;

use defmt::Format;
use serde::{Deserialize, Serialize};

use ack::AckType;
use command::CommandType;
use data::DataType;
use route::RouteType;

use crate::message::MAX_MESSAGE_SIZE;

pub mod ack;
pub mod command;
pub mod data;
pub mod discovery;
pub mod route;

/// This constant is the maximum size of the payload in bytes
pub const MAX_PAYLOAD_SIZE: usize =
    MAX_MESSAGE_SIZE - size_of::<u8>() - size_of::<u8>() - size_of::<u8>() - size_of::<u8>();

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Format)]
pub enum Payload {
    Data(DataType),
    Command(CommandType),
    Ack(AckType),
    Route(RouteType),
    Discovery  { original_ttl: u8 },
    // Other payload types...
}
