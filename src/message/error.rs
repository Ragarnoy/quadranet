#[cfg(feature = "defmt")]
use defmt::Format;
use snafu::Snafu;

#[derive(Debug, Snafu)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub enum MessageError {
    #[snafu(display("Failed to deserialize message"))]
    DeserializationError,
    #[snafu(display("Failed to serialize message"))]
    SerializationError,
}
