use crate::device::Uid;

#[derive(Clone, Copy, Debug, PartialEq, bitcode::Encode, bitcode::Decode)]
pub enum DiscoveryType {
    Request {
        original_ttl: u8,
    },
    Response {
        hops: u8,
        last_hop: Uid,
    },
}
