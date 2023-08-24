use snafu::Snafu;
use crate::device::DeviceError;
use crate::message::MessageError;

#[derive(Debug, Snafu)]
pub enum MeshError {
    #[snafu(display("Route not found"))]
    RouteNotFound,
    #[snafu(display("Route error"))]
    RouteError,
    #[snafu(display("Message error: {}", source))]
    MessageError {
        source: MessageError,
    },
    #[snafu(display("Radio error: {}", source))]
    DeviceError {
        source: DeviceError,
    }
}

impl From<DeviceError> for MeshError {
    fn from(error: DeviceError) -> Self {
        Self::DeviceError { source: error }
    }
}

impl From<MessageError> for MeshError {
    fn from(error: MessageError) -> Self {
        Self::MessageError { source: error }
    }
}