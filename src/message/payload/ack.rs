#[derive(Clone, Copy, Debug, PartialEq, bitcode::Encode, bitcode::Decode)]
pub enum AckType {
    Success,
    Failure,
}
