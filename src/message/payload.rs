
#[derive(Clone, Debug, PartialEq, bitcode::Encode, bitcode::Decode)]
pub enum DataType {
    Text([u8; 64]),  // Using heapless::String
    Binary([u8; 64]),  // Using heapless::Vec
}

#[derive(Clone, Copy, Debug, PartialEq, bitcode::Encode, bitcode::Decode)]
pub enum CommandType {
    SetConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, bitcode::Encode, bitcode::Decode)]
pub enum AckType {
    Success,
    Failure,
}

#[derive(Clone, Debug, PartialEq, bitcode::Encode, bitcode::Decode)]
pub enum Payload {
    Data(DataType),
    Command(CommandType),
    Ack(AckType)
    // Other payload types...
}
