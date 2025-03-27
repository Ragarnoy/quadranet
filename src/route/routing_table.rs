use defmt::{debug, warn};
use embassy_time::{Duration, Instant};
use heapless::{FnvIndexMap, Vec};

use crate::device::Uid;
use crate::route::{LinkQuality, Route, RoutingStats};

// Reduced constants for routing table configuration
pub const MAX_ROUTES: usize = 32;           // Reduced from 128
pub const MAX_ROUTES_PER_DEST: usize = 2;   // Reduced from 3
pub const ROUTE_EXPIRY_SECONDS: u64 = 300;  // Routes expire after 5 minutes
pub const ROUTE_REFRESH_SECONDS: u64 = 180; // Routes should be refreshed after 3 minutes

/// Optimized routing table with memory efficiency improvements
pub struct RoutingTable {
    /// Map from destination ID to entry containing route options
    routes: FnvIndexMap<u8, RouteEntry, MAX_ROUTES>,

    /// Map of link qualities for direct node connections
    link_qualities: FnvIndexMap<u8, LinkQuality, MAX_ROUTES>,

    /// Time period before routes are considered expired
    route_ttl: u64,
}

/// An entry in the routing table for a specific destination
#[derive(Clone)]
struct RouteEntry {
    /// Multiple potential routes to the same destination
    routes: Vec<Route, MAX_ROUTES_PER_DEST>,

    /// Currently preferred route (as an index into routes)
    primary_idx: usize,

    /// When this entry was last used for routing
    last_used: Instant,
}

impl RouteEntry {
    /// Create a new route entry with a single route
    fn new(route: Route) -> Result<Self, ()> {
        let mut routes = Vec::new();
        routes.push(route).map_err(|_| ())?;

        Ok(Self {
            routes,
            primary_idx: 0,
            last_used: Instant::now(),
        })
    }

    // Core methods only, removing rarely used methods to save memory
    fn primary_route(&self) -> Option<Route> {
        self.routes.get(self.primary_idx).copied()
    }

    fn add_route(&mut self, route: Route) -> Result<usize, ()> {
        let idx = self.routes.len();
        self.routes.push(route).map_err(|_| ())?;
        Ok(idx)
    }

    fn find_route_idx_by_next_hop(&self, next_hop: Uid) -> Option<usize> {
        self.routes
            .iter()
            .position(|route| route.next_hop == next_hop)
    }

    fn find_worst_route_idx(&self) -> Option<usize> {
        if self.routes.is_empty() {
            return None;
        }

        let mut worst_idx = 0;
        let mut worst_quality = self.routes[0].quality;

        for (i, route) in self.routes.iter().enumerate().skip(1) {
            if route.quality < worst_quality {
                worst_idx = i;
                worst_quality = route.quality;
            }
        }

        Some(worst_idx)
    }

    fn get_route(&self, idx: usize) -> Option<Route> {
        self.routes.get(idx).copied()
    }

    fn update_route(&mut self, idx: usize, route: Route) -> bool {
        if let Some(existing) = self.routes.get_mut(idx) {
            *existing = route;
            true
        } else {
            false
        }
    }

    fn find_best_route_idx(&self) -> Option<usize> {
        if self.routes.is_empty() {
            return None;
        }

        let mut best_idx = 0;
        let mut best_route = &self.routes[0];

        for (idx, route) in self.routes.iter().enumerate().skip(1) {
            if is_better_route(route, best_route) {
                best_idx = idx;
                best_route = route;
            }
        }

        Some(best_idx)
    }

    fn update_primary_idx(&mut self) {
        if let Some(best_idx) = self.find_best_route_idx() {
            self.primary_idx = best_idx;
        }
    }

    fn find_valid_route_idx(&self, ttl: u64) -> Option<usize> {
        self.routes
            .iter()
            .position(|route| route.is_active && !route.is_expired(ttl))
    }
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
#[inline]
fn is_better_route(route1: &Route, route2: &Route) -> bool {
    // First consider if one route is active and the other isn't
    if route1.is_active && !route2.is_active {
        return true;
    }
    if !route1.is_active && route2.is_active {
        return false;
    }

    // Higher quality routes are better
    if route1.quality > route2.quality + 20 {  // Significant quality difference
        return true;
    }
    if route2.quality > route1.quality + 20 {
        return false;
    }

    // Similar quality, prefer lower hop count
    if route1.hop_count + 1 < route2.hop_count {  // Significant hop difference
        return true;
    }
    if route2.hop_count + 1 < route1.hop_count {
        return false;
    }

    // Everything else being equal, prefer more recently updated routes
    route1.last_updated > route2.last_updated
}

// Only implementing the core methods needed for basic routing
impl RoutingTable {
    /// Create a new routing table with custom TTL
    pub const fn new(route_ttl: u64) -> Self {
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
            debug!("Updated link quality for @{}: RSSI: {}, SNR: {}, Quality: {}",
                   node_id, link.rssi, link.snr, link.calculate_quality());
        } else {
            // Create new link quality entry
            let link = LinkQuality::new(rssi, snr);
            let quality = link.calculate_quality();

            if self.link_qualities.insert(node_id, link).is_err() {
                warn!("Link quality table full, couldn't add node @{}", node_id);
            }
        }
    }

    /// Record successful message delivery to improve link quality score
    pub fn record_successful_delivery(&mut self, node_id: u8) {
        if let Some(link) = self.link_qualities.get_mut(&node_id) {
            link.record_success();

            // Update quality for any routes using this node as next hop
            for entry in self.routes.values_mut() {
                for route in &mut entry.routes {
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
                for route in &mut entry.routes {
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

    // Simplified update method that handles the most common cases efficiently
    pub fn update(&mut self, destination: u8, new_route: Route) {
        // If we have link quality info for this next hop, use it
        let mut route_to_add = new_route;
        if let Some(link) = self.link_qualities.get(&new_route.next_hop.get()) {
            route_to_add.quality = link.calculate_quality();
        }

        // Check if we already have an entry for this destination
        if let Some(entry) = self.routes.get_mut(&destination) {
            // Check if we already have a route through this next hop
            if let Some(idx) = entry.find_route_idx_by_next_hop(new_route.next_hop) {
                // Update existing route
                entry.update_route(idx, route_to_add);
                entry.last_used = Instant::now();

                // Update primary route if needed
                if let Some(primary) = entry.primary_route() {
                    if is_better_route(&route_to_add, &primary) {
                        entry.primary_idx = idx;
                    }
                }

                return;
            }

            // No existing route via this next hop
            if entry.routes.len() < entry.routes.capacity() {
                // Add new route if there's room
                if let Ok(idx) = entry.add_route(route_to_add) {
                    if let Some(primary) = entry.primary_route() {
                        if is_better_route(&route_to_add, &primary) {
                            entry.primary_idx = idx;
                        }
                    } else {
                        entry.primary_idx = idx;
                    }
                }
            } else {
                // Replace worst route if new one is better
                if let Some(worst_idx) = entry.find_worst_route_idx() {
                    if let Some(worst_route) = entry.get_route(worst_idx) {
                        if route_to_add.quality > worst_route.quality {
                            entry.update_route(worst_idx, route_to_add);
                            entry.update_primary_idx();
                        }
                    }
                }
            }

            entry.last_used = Instant::now();
        } else {
            // Create new entry
            if let Ok(entry) = RouteEntry::new(route_to_add) {
                // Check if we need to evict an entry
                if self.routes.len() >= self.routes.capacity() {
                    if let Some(lru_dest) = self.find_least_recently_used() {
                        self.routes.remove(&lru_dest);
                    }
                }

                // Add new entry
                let _ = self.routes.insert(destination, entry);
            }
        }
    }

    /// Look up the best route to a destination
    pub fn lookup_route(&mut self, destination: u8) -> Option<Route> {
        if let Some(entry) = self.routes.get_mut(&destination) {
            // Get the primary route
            let primary_route = entry.primary_route();

            // Check if the primary route is still valid
            if let Some(route) = primary_route {
                if route.is_active && !route.is_expired(self.route_ttl) {
                    entry.last_used = Instant::now();
                    return Some(route);
                }
            }

            // Primary route expired or inactive, try to find another valid route
            if let Some(valid_idx) = entry.find_valid_route_idx(self.route_ttl) {
                if let Some(route) = entry.get_route(valid_idx) {
                    entry.primary_idx = valid_idx;
                    entry.last_used = Instant::now();
                    return Some(route);
                }
            }

            // No valid routes found, but return the primary one anyway
            if let Some(route) = primary_route {
                entry.last_used = Instant::now();
                return Some(route);
            }
        }

        None
    }

    /// Check if routes to a destination need refresh
    pub fn needs_refresh(&self, destination: u8) -> bool {
        if let Some(entry) = self.routes.get(&destination) {
            if let Some(primary) = entry.primary_route() {
                // Check if route is approaching expiry or has low quality
                return primary.is_expired(self.route_ttl) ||
                    (!primary.is_expired(ROUTE_REFRESH_SECONDS) && primary.quality < 100);
            }
        }
        // No route exists, so yes, we need to refresh
        true
    }

    /// Remove expired routes and perform routine maintenance
    pub fn cleanup(&mut self) {
        let now = Instant::now();

        // Retain only non-expired link qualities
        let link_ttl = Duration::from_secs(self.route_ttl * 3);
        self.link_qualities.retain(|_, link| now.duration_since(link.last_used) < link_ttl);

        // Cleanup routing table - using Vec to collect keys to remove
        let mut to_remove = Vec::<u8, 8>::new();

        for (dest, entry) in &mut self.routes {
            // Remove expired or inactive routes
            let old_len = entry.routes.len();
            entry.routes.retain(|route| route.is_active && !route.is_expired(self.route_ttl));

            // If routes were removed, update primary index
            if entry.routes.len() < old_len && entry.primary_idx >= entry.routes.len() {
                entry.update_primary_idx();
            }

            // Mark for removal if no valid routes remain
            if entry.routes.is_empty() {
                let _ = to_remove.push(*dest);
            }
        }

        // Remove entries with no valid routes
        for dest in &to_remove {
            self.routes.remove(dest);
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
            for route in &entry.routes {
                if route.is_active {
                    stats.active_routes += 1;
                }

                if route.is_expired(self.route_ttl) {
                    stats.expired_routes += 1;
                }

                hop_sum += route.hop_count as usize;
                quality_sum += u32::from(route.quality);
                route_count += 1;
            }
        }

        if route_count > 0 {
            stats.avg_hop_count = hop_sum / route_count;
            stats.avg_quality = (quality_sum / route_count as u32) as u8;
        }

        stats
    }

    /// Find the least recently used route entry destination
    fn find_least_recently_used(&self) -> Option<u8> {
        let mut oldest_dest = None;
        let mut oldest_time = None;

        for (dest, entry) in &self.routes {
            if oldest_time.is_none() || entry.last_used < oldest_time.unwrap() {
                oldest_dest = Some(*dest);
                oldest_time = Some(entry.last_used);
            }
        }

        oldest_dest
    }
}