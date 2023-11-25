use crate::route::Route;
use heapless::FnvIndexMap;

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
    }

    pub fn lookup_route(&self, destination: u8) -> Option<Route> {
        self.routes.get(&destination).map(|route| Route {
            next_hop: route.next_hop,
        })
    }
}
