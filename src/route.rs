use crate::device::Uid;

pub mod routing_table;

pub struct Route {
    pub next_hop: Uid,  // UID of the next node in the path
    // ... other possible fields like cost, hop_count, etc.
}
