#[derive(Clone, Copy, Debug, PartialEq, bitcode::Encode, bitcode::Decode)]
pub enum CommandType {
    SetConfig,
}
