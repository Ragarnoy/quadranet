use defmt::Format;
use embassy_time::Instant;

use crate::device::Uid;

pub mod routing_table;

/// Enhanced route object with quality metrics and timestamps
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
    pub fn touch(&mut self) {
        self.last_updated = Instant::now();
    }

    /// Check if the route has expired based on a TTL
    pub fn is_expired(&self, ttl_seconds: u64) -> bool {
        Instant::now().duration_since(self.last_updated).as_secs() > ttl_seconds
    }

    /// Convenience method to create updated version of this route with a new hop
    pub fn with_additional_hop(&self, new_next_hop: Uid) -> Self {
        Self {
            next_hop: new_next_hop,
            hop_count: self.hop_count + 1,
            quality: self.quality.saturating_sub(10), // Reduce quality for longer paths
            last_updated: Instant::now(),
            is_active: true,
        }
    }
}

/// Metadata about the link quality for a specific connection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkQuality {
    /// Signal strength indicator (-120 to -30 dBm typically)
    pub rssi: i16,

    /// Signal-to-noise ratio (-20 to +10 dB typically)
    pub snr: i16,

    /// Message delivery success rate (0-100%)
    pub success_rate: u8,

    /// Message delivery failure rate (0-100%)
    pub failure_rate: u8,

    /// Last time this link was used
    pub last_used: Instant,
}

impl LinkQuality {
    pub fn new(rssi: i16, snr: i16) -> Self {
        Self {
            rssi,
            snr,
            success_rate: 100, // Assume perfect initially
            failure_rate: 0,
            last_used: Instant::now(),
        }
    }

    /// Calculate a quality score from the link metrics (0-255)
    #[inline]
    pub fn calculate_quality(&self) -> u8 {
        // Normalize RSSI: -120dBm -> 0, -30dBm -> 100
        let rssi_norm = (f32::from(self.rssi + 120) / 90.0 * 100.0).clamp(0.0, 100.0) as u16;

        // Normalize SNR: -20dB -> 0, +10dB -> 100
        let snr_norm = (f32::from(self.snr + 20) / 30.0 * 100.0).clamp(0.0, 100.0) as u16;

        // Calculate combined score with weights
        // Success rate is most important, followed by SNR, then RSSI
        let quality = (u16::from(self.success_rate) * 4
            + snr_norm * 3
            + rssi_norm * 2
            + u16::from(100 - self.failure_rate))
            / 10;

        // Scale to 0-255
        ((quality * 255) / 100) as u8
    }

    /// Record a successful message delivery
    pub fn record_success(&mut self) {
        self.success_rate = ((u16::from(self.success_rate) * 9 + 100) / 10) as u8;
        self.failure_rate = self.failure_rate.saturating_sub(5);
        self.last_used = Instant::now();
    }

    /// Record a failed message delivery
    pub fn record_failure(&mut self) {
        self.failure_rate = ((u16::from(self.failure_rate) * 9 + 100) / 10) as u8;
        self.success_rate = self.success_rate.saturating_sub(10);
        self.last_used = Instant::now();
    }

    /// Update RSSI and SNR values with exponential smoothing
    pub fn update_signal_metrics(&mut self, rssi: i16, snr: i16) {
        // Apply exponential smoothing (70% old, 30% new)
        self.rssi = (f32::from(self.rssi) * 0.7 + f32::from(rssi) * 0.3) as i16;
        self.snr = (f32::from(self.snr) * 0.7 + f32::from(snr) * 0.3) as i16;
        self.last_used = Instant::now();
    }
}

/// Stats structure for monitoring the routing system
#[derive(Debug, Copy, Clone, Format)]
pub struct RoutingStats {
    pub total_entries: usize,  // Number of destinations
    pub active_routes: usize,  // Number of active routes
    pub expired_routes: usize, // Number of expired routes
    pub avg_hop_count: usize,  // Average hop count across all routes
    pub avg_quality: u8,       // Average route quality
}
