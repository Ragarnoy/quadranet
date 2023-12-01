use crate::message::MAX_MESSAGE_SIZE;
use ack::AckType;
use command::CommandType;
use core::mem::size_of;
use data::DataType;
use defmt::Format;
use discovery::DiscoveryType;
use route::RouteType;
use serde::{Deserialize, Serialize};

mod ack;
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
    Discovery(DiscoveryType),
    // Other payload types...
}
