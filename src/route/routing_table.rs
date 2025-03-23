use defmt::{debug, warn};
use embassy_time::{Duration, Instant};
use heapless::{FnvIndexMap, Vec};

use crate::route::{LinkQuality, Route, RoutingStats};

// Constants for routing table configuration
pub const MAX_ROUTES: usize = 128; // Maximum number of destinations
pub const MAX_ROUTES_PER_DEST: usize = 3; // Maximum alternative routes per destination
pub const ROUTE_EXPIRY_SECONDS: u64 = 300; // Routes expire after 5 minutes
pub const ROUTE_REFRESH_SECONDS: u64 = 180; // Routes should be refreshed after 3 minutes

/// Enhanced routing table with multiple paths and quality metrics
pub struct RoutingTable {
    /// Map from destination ID to entry containing route options
    routes: FnvIndexMap<u8, RouteEntry, MAX_ROUTES>,

    /// Map of link qualities for direct node connections
    link_qualities: FnvIndexMap<u8, LinkQuality, MAX_ROUTES>,

    /// Time period before routes are considered expired
    route_ttl: u64,
}

/// An entry in the routing table for a specific destination
struct RouteEntry {
    /// Multiple potential routes to the same destination
    routes: Vec<Route, MAX_ROUTES_PER_DEST>,

    /// Index of the currently preferred route
    primary_index: usize,

    /// When this entry was last used for routing
    last_used: Instant,
}

impl Default for RoutingTable {
    fn default() -> Self {
        Self {
            routes: FnvIndexMap::new(),
            link_qualities: FnvIndexMap::new(),
            route_ttl: ROUTE_EXPIRY_SECONDS,
        }
    }
}

/// Helper function to compare routes without needing a self reference
fn is_better_route(route1: &Route, route2: &Route) -> bool {
    // First consider if one route is active and the other isn't
    if route1.is_active && !route2.is_active {
        return true;
    }
    if !route1.is_active && route2.is_active {
        return false;
    }

    // Higher quality routes are better
    if route1.quality > route2.quality + 20 {
        // Significant quality difference
        return true;
    }
    if route2.quality > route1.quality + 20 {
        return false;
    }

    // Similar quality, prefer lower hop count
    if route1.hop_count + 1 < route2.hop_count {
        // Significant hop difference
        return true;
    }
    if route2.hop_count + 1 < route1.hop_count {
        return false;
    }

    // Everything else being equal, prefer more recently updated routes
    route1.last_updated > route2.last_updated
}

impl RoutingTable {
    /// Create a new routing table with custom TTL
    pub fn new(route_ttl: u64) -> Self {
        Self {
            routes: FnvIndexMap::new(),
            link_qualities: FnvIndexMap::new(),
            route_ttl,
        }
    }

    /// Update link quality information based on received message metrics
    pub fn update_link_quality(&mut self, node_id: u8, rssi: i16, snr: i16) {
        if let Some(link) = self.link_qualities.get_mut(&node_id) {
            // Update existing link quality info
            link.update_signal_metrics(rssi, snr);
            debug!(
                "Updated link quality for @{}: RSSI: {}, SNR: {}, Quality: {}",
                node_id,
                link.rssi,
                link.snr,
                link.calculate_quality()
            );
        } else {
            // Create new link quality entry
            let link = LinkQuality::new(rssi, snr);
            let quality = link.calculate_quality();
            if self.link_qualities.insert(node_id, link).is_err() {
                warn!("Link quality table full, couldn't add node @{}", node_id);
            } else {
                debug!(
                    "Added new link quality for @{}: RSSI: {}, SNR: {}, Quality: {}",
                    node_id, rssi, snr, quality
                );
            }
        }
    }

    /// Record successful message delivery to improve link quality score
    pub fn record_successful_delivery(&mut self, node_id: u8) {
        if let Some(link) = self.link_qualities.get_mut(&node_id) {
            link.record_success();

            // Update quality for any routes using this node as next hop
            for entry in self.routes.values_mut() {
                for route in entry.routes.iter_mut() {
                    if route.next_hop.get() == node_id {
                        let new_quality = link.calculate_quality();
                        route.quality = new_quality;
                        route.touch();
                    }
                }
            }
        }
    }

    /// Record failed message delivery to decrease link quality score
    pub fn record_failed_delivery(&mut self, node_id: u8) {
        if let Some(link) = self.link_qualities.get_mut(&node_id) {
            link.record_failure();

            // Update quality for any routes using this node as next hop
            for entry in self.routes.values_mut() {
                for route in entry.routes.iter_mut() {
                    if route.next_hop.get() == node_id {
                        let new_quality = link.calculate_quality();
                        route.quality = new_quality;
                        route.touch();

                        // If quality drops significantly, mark route as inactive
                        if new_quality < 50 {
                            route.is_active = false;
                        }
                    }
                }
            }
        }
    }

    /// Add or update a route to a destination
    pub fn update(&mut self, destination: u8, new_route: Route) {
        let now = Instant::now();

        // If we have link quality info for this next hop, use it
        let mut route_to_add = new_route;
        if let Some(link) = self.link_qualities.get(&new_route.next_hop.get()) {
            route_to_add.quality = link.calculate_quality();
        }

        // Check if entry exists for this destination
        if self.routes.contains_key(&destination) {
            // Get a copy of the primary index before mutable borrows
            let entry = self.routes.get(&destination).unwrap();
            let primary_index = entry.primary_index;
            let primary_route = if primary_index < entry.routes.len() {
                Some(entry.routes[primary_index])
            } else {
                None
            };

            // Now do the mutable work
            let entry = self.routes.get_mut(&destination).unwrap();

            // Check if we already have a route through this next hop
            let mut found_existing = false;
            for i in 0..entry.routes.len() {
                if entry.routes[i].next_hop == new_route.next_hop {
                    // Update existing route
                    entry.routes[i] = route_to_add;
                    found_existing = true;

                    // Check if this should be the primary route
                    if let Some(primary) = primary_route {
                        if is_better_route(&route_to_add, &primary) {
                            entry.primary_index = i;
                        }
                    }

                    debug!(
                        "Updated route to @{} via @{} (hops: {}, quality: {})",
                        destination,
                        new_route.next_hop.get(),
                        new_route.hop_count,
                        route_to_add.quality
                    );
                    break;
                }
            }

            if !found_existing {
                // No existing route via this next hop, add it if there's space
                if entry.routes.len() < entry.routes.capacity() {
                    let idx = entry.routes.len();
                    if entry.routes.push(route_to_add).is_err() {
                        // This shouldn't happen given our check above
                        warn!(
                            "Failed to add route to @{} via @{}",
                            destination,
                            new_route.next_hop.get()
                        );
                        return;
                    }

                    // Check if this new route should be the primary
                    if let Some(primary) = primary_route {
                        if is_better_route(&route_to_add, &primary) {
                            entry.primary_index = idx;
                        }
                    } else {
                        // No primary route, make this one primary
                        entry.primary_index = idx;
                    }

                    debug!(
                        "Added alternate route to @{} via @{} (hops: {}, quality: {})",
                        destination,
                        new_route.next_hop.get(),
                        new_route.hop_count,
                        route_to_add.quality
                    );
                } else {
                    // Vector is full, find the worst route to replace
                    let mut worst_idx = 0;
                    let mut worst_quality = entry.routes[0].quality;

                    // Find worst route without using self methods
                    for i in 1..entry.routes.len() {
                        if entry.routes[i].quality < worst_quality {
                            worst_idx = i;
                            worst_quality = entry.routes[i].quality;
                        }
                    }

                    // Check if new route is better than the worst one
                    if route_to_add.quality > worst_quality {
                        debug!(
                            "Replacing route to @{} via @{} with better route via @{}",
                            destination,
                            entry.routes[worst_idx].next_hop.get(),
                            new_route.next_hop.get()
                        );

                        // Replace the worst route
                        entry.routes[worst_idx] = route_to_add;

                        // Check if this new route should be the primary
                        if worst_idx == entry.primary_index {
                            // We replaced the primary route, need to find the new best
                            for i in 0..entry.routes.len() {
                                if i != worst_idx
                                    && is_better_route(&entry.routes[i], &route_to_add)
                                {
                                    entry.primary_index = i;
                                    break;
                                }
                            }
                        } else if let Some(primary) = primary_route {
                            // Check against existing primary
                            if is_better_route(&route_to_add, &primary) {
                                entry.primary_index = worst_idx;
                            }
                        }
                    }
                }
            }
        } else {
            // Create a new entry for this destination
            let mut routes = Vec::new();
            if routes.push(route_to_add).is_err() {
                warn!("Failed to create route vector for @{}", destination);
                return;
            }

            let entry = RouteEntry {
                routes,
                primary_index: 0,
                last_used: now,
            };

            // If table is full, evict the least recently used entry
            if self.routes.len() >= self.routes.capacity() {
                let lru_dest = self.find_least_recently_used();
                if let Some(dest) = lru_dest {
                    self.routes.remove(&dest);
                    warn!("Evicted route to @{} to make room in routing table", dest);
                }
            }

            // Add the new entry
            if self.routes.insert(destination, entry).is_err() {
                warn!("Failed to add route entry for @{}", destination);
                return;
            }

            debug!(
                "Created new route to @{} via @{} (hops: {}, quality: {})",
                destination,
                new_route.next_hop.get(),
                new_route.hop_count,
                route_to_add.quality
            );
        }
    }

    /// Look up the best route to a destination
    pub fn lookup_route(&mut self, destination: u8) -> Option<Route> {
        let now = Instant::now();

        if let Some(entry) = self.routes.get_mut(&destination) {
            entry.last_used = now;

            // Check if the primary route is still valid
            if entry.primary_index < entry.routes.len()
                && entry.routes[entry.primary_index].is_active
                && !entry.routes[entry.primary_index].is_expired(self.route_ttl)
            {
                // Return the primary route
                return Some(entry.routes[entry.primary_index]);
            }

            // Primary route expired or inactive, try to find another valid route
            for (i, route) in entry.routes.iter().enumerate() {
                if route.is_active && !route.is_expired(self.route_ttl) {
                    // Update the primary index
                    entry.primary_index = i;
                    return Some(*route);
                }
            }

            // No valid routes found, but return the primary one anyway
            // as a best-effort (caller can decide what to do)
            if entry.primary_index < entry.routes.len() {
                warn!(
                    "Using expired route to @{} via @{} as best effort",
                    destination,
                    entry.routes[entry.primary_index].next_hop.get()
                );
                return Some(entry.routes[entry.primary_index]);
            }
        }

        None
    }

    /// Get all known routes for a destination (for diagnostics)
    pub fn get_all_routes(&self, destination: u8) -> Option<Vec<Route, MAX_ROUTES_PER_DEST>> {
        self.routes
            .get(&destination)
            .map(|entry| entry.routes.clone())
    }

    /// Remove expired routes and perform routine maintenance
    pub fn cleanup(&mut self) {
        let now = Instant::now();
        let mut to_remove = Vec::<u8, 16>::new();

        // First, clean up link quality records that are too old
        let link_ttl = Duration::from_secs(self.route_ttl * 3); // Keep link info longer than routes
        self.link_qualities
            .retain(|_, link| now.duration_since(link.last_used) < link_ttl);

        // Then clean up the routing table
        for (dest, entry) in self.routes.iter_mut() {
            let mut has_valid_routes = false;

            // Remove expired or inactive routes
            entry.routes.retain(|route| {
                let valid = route.is_active && !route.is_expired(self.route_ttl);
                has_valid_routes |= valid;
                valid
            });

            // If no valid routes remain, mark for removal
            if !has_valid_routes || entry.routes.is_empty() {
                to_remove.push(*dest).unwrap_or_else(|_| {
                    warn!("Failed to add @{} to removal list", dest);
                });
            } else if entry.primary_index >= entry.routes.len() {
                // Update primary index if needed
                entry.primary_index = 0;

                // Find the best route
                for i in 1..entry.routes.len() {
                    if is_better_route(&entry.routes[i], &entry.routes[entry.primary_index]) {
                        entry.primary_index = i;
                    }
                }
            }
        }

        // Remove entries with no valid routes
        for dest in to_remove.iter() {
            self.routes.remove(dest);
            debug!("Removed route entry to @{} - no valid routes", dest);
        }
    }

    /// Get statistics about the current routing table
    pub fn stats(&self) -> RoutingStats {
        let mut stats = RoutingStats {
            total_entries: self.routes.len(),
            active_routes: 0,
            expired_routes: 0,
            avg_hop_count: 0,
            avg_quality: 0,
        };

        let mut hop_sum = 0;
        let mut quality_sum: u32 = 0;
        let mut route_count = 0;

        for entry in self.routes.values() {
            for route in entry.routes.iter() {
                if route.is_active {
                    stats.active_routes += 1;
                }

                if route.is_expired(self.route_ttl) {
                    stats.expired_routes += 1;
                }

                hop_sum += route.hop_count as usize;
                quality_sum += route.quality as u32;
                route_count += 1;
            }
        }

        if route_count > 0 {
            stats.avg_hop_count = hop_sum / route_count;
            stats.avg_quality = (quality_sum / route_count as u32) as u8;
        }

        stats
    }

    /// Check if routes to a destination need refresh
    pub fn needs_refresh(&self, destination: u8) -> bool {
        if let Some(entry) = self.routes.get(&destination) {
            if entry.primary_index < entry.routes.len() {
                let primary = &entry.routes[entry.primary_index];
                // Check if route is approaching expiry or has low quality
                return primary.is_expired(self.route_ttl)
                    || (!primary.is_expired(ROUTE_REFRESH_SECONDS) && primary.quality < 100);
            }
        }
        // No route exists, so yes, we need to refresh
        true
    }

    /// Find the least recently used route entry destination
    fn find_least_recently_used(&self) -> Option<u8> {
        let mut oldest_dest = None;
        let mut oldest_time = None;

        for (dest, entry) in self.routes.iter() {
            if oldest_time.is_none() || entry.last_used < oldest_time.unwrap() {
                oldest_dest = Some(*dest);
                oldest_time = Some(entry.last_used);
            }
        }

        oldest_dest
    }
}
