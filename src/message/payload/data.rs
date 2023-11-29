use crate::message::payload::MAX_PAYLOAD_SIZE;

#[derive(Clone, Debug, PartialEq, bitcode::Encode, bitcode::Decode)]
pub enum DataType {
    Text([u8; MAX_PAYLOAD_SIZE]),  // Using heapless::String
    Binary([u8; MAX_PAYLOAD_SIZE]),  // Using heapless::Vec
}
