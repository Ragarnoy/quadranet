use crate::device::Uid;

pub mod routing_table;

#[derive(Clone, Copy, Debug, PartialEq, bitcode::Encode, bitcode::Decode)]
pub struct Route {
    pub next_hop: Uid,
    pub hop_count: u8,
}
