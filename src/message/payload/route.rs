use defmt::Format;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Format)]
pub enum RouteType {
    Error,
    Request,
    Response,
}
