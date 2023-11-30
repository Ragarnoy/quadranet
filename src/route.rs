use crate::device::Uid;

pub mod routing_table;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Route {
    pub next_hop: Uid,
    pub hop_count: u8,
}
