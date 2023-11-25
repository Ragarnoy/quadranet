use defmt::Format;
use snafu::Snafu;

#[derive(Debug, Snafu, Format)]
pub enum MessageError {
    #[snafu(display("Invalid Message"))]
    InvalidMessage,
}
