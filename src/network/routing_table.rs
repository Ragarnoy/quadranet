use crate::network::route::Route;
use heapless::FnvIndexMap;

pub struct RoutingTable {
    routes: FnvIndexMap<u16, Route, 128>,
}

impl Default for RoutingTable {
    fn default() -> Self {
        Self {
            routes: FnvIndexMap::new(),
        }
    }
}

impl RoutingTable {
    pub fn update(&mut self, destination: u16, route: Route) {
        // Insert or update the route for the given destination
        // Note: `insert` returns an Err if the map is full
        let _ = self.routes.insert(destination, route);
    }

    pub fn lookup_route(&self, destination: u16) -> Option<Route> {
        self.routes.get(&destination).map(|route| Route {
            next_hop: route.next_hop,
        })
    }
}
