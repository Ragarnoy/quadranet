#[cfg(feature = "defmt")]
use defmt::Format;
use embassy_time::Instant;

use crate::device::Uid;

pub mod routing_table;

/// Optimized route object with streamlined structure
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    /// Next hop to reach the destination
    pub next_hop: Uid,

    /// Number of hops to reach the destination
    pub hop_count: u8,

    /// Route quality score (0-255, higher is better)
    pub quality: u8,

    /// When this route was last updated
    pub last_updated: Instant,

    /// Whether this route is currently active
    pub is_active: bool,
}

impl Route {
    /// Create a new route with default values
    #[inline]
    pub fn new(next_hop: Uid, hop_count: u8) -> Self {
        Self {
            next_hop,
            hop_count,
            quality: 0,
            last_updated: Instant::now(),
            is_active: true,
        }
    }

    /// Create a new route with specified quality
    #[inline]
    pub fn with_quality(next_hop: Uid, hop_count: u8, quality: u8) -> Self {
        Self {
            next_hop,
            hop_count,
            quality,
            last_updated: Instant::now(),
            is_active: true,
        }
    }

    /// Update the route timestamp
    #[inline]
    pub fn touch(&mut self) {
        self.last_updated = Instant::now();
    }

    /// Check if the route has expired based on a TTL
    #[inline]
    pub fn is_expired(&self, ttl_seconds: u64) -> bool {
        Instant::now().duration_since(self.last_updated).as_secs() > ttl_seconds
    }
}

/// Optimized link quality tracker
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkQuality {
    /// Signal strength indicator
    pub rssi: i16,

    /// Signal-to-noise ratio
    pub snr: i16,

    /// Success rate (0-100%)
    pub success_rate: u8,

    /// Failure rate (0-100%)
    pub failure_rate: u8,

    /// Last time this link was used
    pub last_used: Instant,
}

impl LinkQuality {
    #[inline]
    pub fn new(rssi: i16, snr: i16) -> Self {
        Self {
            rssi,
            snr,
            success_rate: 100,
            failure_rate: 0,
            last_used: Instant::now(),
        }
    }

    /// Calculate quality score from link metrics (0-255)
    #[inline]
    pub fn calculate_quality(&self) -> u8 {
        // Simplified calculation for less CPU usage
        let rssi_norm = ((self.rssi + 130) * 2).clamp(0, 255) as u16;
        let snr_norm = ((self.snr + 20) * 4).clamp(0, 255) as u16;

        ((rssi_norm + snr_norm * 3 + u16::from(self.success_rate) * 4) / 8) as u8
    }

    /// Record successful message delivery
    #[inline]
    pub fn record_success(&mut self) {
        // Simplified success tracking
        if self.success_rate < 100 {
            self.success_rate = self.success_rate.saturating_add(5).min(100);
        }
        self.failure_rate = self.failure_rate.saturating_sub(5);
        self.last_used = Instant::now();
    }

    /// Record failed message delivery
    #[inline]
    pub fn record_failure(&mut self) {
        // Simplified failure tracking
        if self.failure_rate < 100 {
            self.failure_rate = self.failure_rate.saturating_add(10).min(100);
        }
        self.success_rate = self.success_rate.saturating_sub(10);
        self.last_used = Instant::now();
    }

    /// Update signal metrics
    #[inline]
    pub fn update_signal_metrics(&mut self, rssi: i16, snr: i16) {
        // Simplified exponential smoothing (75% old, 25% new)
        self.rssi = ((i32::from(self.rssi) * 3 + i32::from(rssi)) / 4) as i16;
        self.snr = ((i32::from(self.snr) * 3 + i32::from(snr)) / 4) as i16;
        self.last_used = Instant::now();
    }
}

/// Compact stats structure
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(Format))]
pub struct RoutingStats {
    pub total_entries: usize,
    pub active_routes: usize,
    pub expired_routes: usize,
    pub avg_hop_count: usize,
    pub avg_quality: u8,
}
