use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Format)]
pub enum RouteType {
    Request,
    Response,
    Error,
}
