use defmt::debug;
use heapless::FnvIndexMap;

use crate::route::Route;

pub struct RoutingTable {
    routes: FnvIndexMap<u8, Route, 128>,
}

impl Default for RoutingTable {
    fn default() -> Self {
        Self {
            routes: FnvIndexMap::new(),
        }
    }
}

impl RoutingTable {
    pub fn update(&mut self, destination: u8, route: Route) {
        if let Err((destination, route)) = self.routes.insert(destination, route) {
            // Remove the oldest entry
            let _ = self.routes.remove(&destination);
            // Insert the new entry
            let _ = self.routes.insert(destination, route);
        }
        debug!("ROUTING TABLE UPDATE @{}", destination);
    }

    pub fn lookup_route(&self, destination: u8) -> Option<Route> {
        self.routes.get(&destination).copied()
    }
}
