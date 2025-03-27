use embassy_time::{Duration, Instant};
use heapless::FnvIndexMap;

use crate::route::{LinkQuality, Route, RoutingStats};

// Reduced constants for routing table configuration
pub const MAX_ROUTES: usize = 16;            // Reduced from 32
pub const MAX_ROUTES_PER_DEST: usize = 1;    // Reduced from 2 - only keep best route
pub const ROUTE_EXPIRY_SECONDS: u64 = 300;   // Routes expire after 5 minutes
pub const ROUTE_REFRESH_SECONDS: u64 = 180;  // Routes should be refreshed after 3 minutes

/// Memory-optimized routing table with minimalist design
pub struct RoutingTable {
    /// Map from destination ID to route
    routes: FnvIndexMap<u8, Route, MAX_ROUTES>,

    /// Map of link qualities for direct node connections
    link_qualities: FnvIndexMap<u8, LinkQuality, MAX_ROUTES>,

    /// Time period before routes are considered expired
    route_ttl: u64,
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

impl RoutingTable {
    /// Create a new routing table with default settings
    pub const fn new() -> Self {
        Self {
            routes: FnvIndexMap::new(),
            link_qualities: FnvIndexMap::new(),
            route_ttl: ROUTE_EXPIRY_SECONDS,
        }
    }

    /// Create a new routing table with more compact parameters
    pub const fn new_compact() -> Self {
        Self {
            routes: FnvIndexMap::new(),
            link_qualities: FnvIndexMap::new(),
            route_ttl: ROUTE_EXPIRY_SECONDS,
        }
    }

    /// Update link quality information based on received message metrics
    pub fn update_link_quality(&mut self, node_id: u8, rssi: i16, snr: i16) {
        if let Some(link) = self.link_qualities.get_mut(&node_id) {
            // Update existing link quality info
            link.update_signal_metrics(rssi, snr);
        } else {
            // Create new link quality entry if space available
            let link = LinkQuality::new(rssi, snr);
            let _ = self.link_qualities.insert(node_id, link);
        }
    }

    /// Record successful message delivery
    pub fn record_successful_delivery(&mut self, node_id: u8) {
        if let Some(link) = self.link_qualities.get_mut(&node_id) {
            link.record_success();

            // Update routes using this link
            for (_, route) in self.routes.iter_mut() {
                if route.next_hop.get() == node_id {
                    route.quality = link.calculate_quality();
                    route.touch();
                }
            }
        }
    }

    /// Record failed message delivery
    pub fn record_failed_delivery(&mut self, node_id: u8) {
        if let Some(link) = self.link_qualities.get_mut(&node_id) {
            link.record_failure();

            // Update routes using this link
            for (_, route) in self.routes.iter_mut() {
                if route.next_hop.get() == node_id {
                    route.quality = link.calculate_quality();
                    route.touch();

                    // Mark as inactive if quality drops too low
                    if route.quality < 50 {
                        route.is_active = false;
                    }
                }
            }
        }
    }

    /// Add or update a route - simplified to always keep best route
    pub fn update(&mut self, destination: u8, new_route: Route) {
        // Update route quality based on link quality if available
        let mut route_to_add = new_route;
        if let Some(link) = self.link_qualities.get(&new_route.next_hop.get()) {
            route_to_add.quality = link.calculate_quality();
        }

        // Check if we already have a route for this destination
        if let Some(existing_route) = self.routes.get(&destination) {
            // Only update if new route is better
            if is_better_route(&route_to_add, existing_route) {
                let _ = self.routes.insert(destination, route_to_add);
            }
        } else {
            // No existing route, add new one
            if self.routes.len() >= MAX_ROUTES {
                // Need to evict a route - find least recently used
                if let Some(lru_dest) = self.find_least_recently_used() {
                    self.routes.remove(&lru_dest);
                }
            }

            // Add new route
            let _ = self.routes.insert(destination, route_to_add);
        }
    }

    /// Look up route to destination
    pub fn lookup_route(&mut self, destination: u8) -> Option<Route> {
        if let Some(route) = self.routes.get_mut(&destination) {
            // Check if route is still valid
            if route.is_active && !route.is_expired(self.route_ttl) {
                route.touch(); // Update usage timestamp
                return Some(*route);
            }
        }

        None
    }

    /// Determine if a route needs refreshing
    pub fn needs_refresh(&self, destination: u8) -> bool {
        if let Some(route) = self.routes.get(&destination) {
            // Check if route is approaching expiry or has low quality
            return route.is_expired(ROUTE_REFRESH_SECONDS) || route.quality < 100;
        }

        // No route exists, so it needs refreshing
        true
    }

    /// Remove expired routes
    pub fn cleanup(&mut self) {
        let now = Instant::now();

        // Clean up link qualities
        self.link_qualities.retain(|_, link| {
            now.duration_since(link.last_used) < Duration::from_secs(self.route_ttl * 2)
        });

        // Clean up routes
        let expired_ttl = self.route_ttl;
        self.routes.retain(|_, route| {
            route.is_active && !route.is_expired(expired_ttl)
        });
    }

    /// Get routing table statistics
    pub fn stats(&self) -> RoutingStats {
        let mut stats = RoutingStats {
            total_entries: self.routes.len(),
            active_routes: 0,
            expired_routes: 0,
            avg_hop_count: 0,
            avg_quality: 0,
        };

        let mut quality_sum: u32 = 0;
        let mut hop_sum: usize = 0;
        let mut route_count = 0;

        for route in self.routes.values() {
            route_count += 1;
            hop_sum += route.hop_count as usize;
            quality_sum += u32::from(route.quality);

            if route.is_active {
                stats.active_routes += 1;
            }

            if route.is_expired(self.route_ttl) {
                stats.expired_routes += 1;
            }
        }

        if route_count > 0 {
            stats.avg_hop_count = hop_sum / route_count;
            stats.avg_quality = (quality_sum / route_count as u32) as u8;
        }

        stats
    }

    /// Find the least recently used route
    fn find_least_recently_used(&self) -> Option<u8> {
        let mut oldest_dest = None;
        let mut oldest_time = None;

        for (dest, route) in &self.routes {
            if oldest_time.is_none() || route.last_updated < oldest_time.unwrap() {
                oldest_dest = Some(*dest);
                oldest_time = Some(route.last_updated);
            }
        }

        oldest_dest
    }
}

/// Helper to compare routes - inlined for performance
#[inline]
fn is_better_route(route1: &Route, route2: &Route) -> bool {
    // Active routes are better than inactive ones
    if route1.is_active && !route2.is_active {
        return true;
    }
    if !route1.is_active && route2.is_active {
        return false;
    }

    // Consider quality first (with significance threshold)
    if route1.quality > route2.quality + 15 {
        return true;
    }
    if route2.quality > route1.quality + 15 {
        return false;
    }

    // Similar quality, prefer fewer hops
    if route1.hop_count < route2.hop_count {
        return true;
    }
    if route2.hop_count < route1.hop_count {
        return false;
    }

    // If all other factors equal, prefer newer route
    route1.last_updated > route2.last_updated
}