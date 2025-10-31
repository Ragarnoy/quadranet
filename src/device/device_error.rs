#[cfg(feature = "defmt")]
use defmt::Format;
use lora_phy::mod_params::RadioError;
use snafu::Snafu;

use crate::message::error::MessageError;

#[derive(Debug, Snafu)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub enum DeviceError {
    #[snafu(display("Route not found"))]
    RouteNotFound,
    #[snafu(display("Route error"))]
    RouteError,
    #[snafu(display("Message error: {}", source))]
    MessageError { source: MessageError },
    #[snafu(display("Radio error: {:?}", error))]
    RadioError { error: RadioError },
    #[snafu(display("Invalid destination"))]
    InvalidDestination,
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
