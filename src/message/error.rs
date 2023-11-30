use defmt::Format;
use snafu::Snafu;

#[derive(Debug, Snafu, Format)]
pub enum MessageError {
    #[snafu(display("Failed to deserialize message"))]
    DeserializationError,
    #[snafu(display("Failed to serialize message"))]
    SerializationError,
}
