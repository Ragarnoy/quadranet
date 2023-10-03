use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum MessageError {
    #[snafu(display("Invalid UID"))]
    InvalidUid,
    #[snafu(display("Invalid length"))]
    InvalidLength,
    #[snafu(display("Invalid TTL"))]
    InvalidTtl,
    #[snafu(display("Invalid sequence number"))]
    InvalidSequenceNumber,
    #[snafu(display("Invalid intent"))]
    InvalidIntent,
    #[snafu(display("Invalid content"))]
    InvalidContent,
}
