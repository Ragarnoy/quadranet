use crate::message::error::MessageError;
use defmt::Format;
use lora_phy::mod_params::RadioError;
use snafu::Snafu;

#[derive(Debug, Snafu, Format)]
pub enum DeviceError {
    #[snafu(display("Route not found"))]
    RouteNotFound,
    #[snafu(display("Route error"))]
    RouteError,
    #[snafu(display("Message error: {}", source))]
    MessageError { source: MessageError },
    #[snafu(display("Radio error: {:?}", error))]
    RadioError { error: RadioError },
}

impl From<RadioError> for DeviceError {
    fn from(error: RadioError) -> Self {
        Self::RadioError { error }
    }
}

impl From<MessageError> for DeviceError {
    fn from(error: MessageError) -> Self {
        Self::MessageError { source: error }
    }
}
