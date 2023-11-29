use core::mem::size_of;
use ack::AckType;
use command::CommandType;
use data::DataType;
use discovery::DiscoveryType;
use route::RouteType;
use crate::message::MAX_MESSAGE_SIZE;

pub mod data;
pub mod command;
pub mod discovery;
pub mod route;
mod ack;

/// This constant is the maximum size of the payload in bytes
pub const MAX_PAYLOAD_SIZE: usize = MAX_MESSAGE_SIZE - size_of::<u8>() - size_of::<u8>() - size_of::<u8>() - size_of::<u8>();

#[derive(Clone, Debug, PartialEq, bitcode::Encode, bitcode::Decode)]
pub enum Payload {
    Data(DataType),
    Command(CommandType),
    Ack(AckType),
    Route(RouteType),
    Discovery(DiscoveryType),
    // Other payload types...
}
