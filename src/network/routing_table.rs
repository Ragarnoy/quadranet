use crate::network::route::Route;

pub struct RoutingTable {
    // ... fields to manage routing
}

impl RoutingTable {
    pub fn update(&mut self) {
        todo!("Update routing table")
    }

    pub fn lookup_route(&self, destination: u16) -> Option<Route> {
        todo!("Lookup route for a given destination")
    }
}
