use crate::policy::{Bandwidth, Latency, PacketLoss};
use std::time::Duration;

pub const DEFAULT_LATENCY: Latency = Latency::new(Duration::from_millis(5));
pub const DEFAULT_IDLE: Duration = Duration::from_micros(500);

pub const DEFAULT_UPLOAD_BANDWIDTH: Bandwidth =
    Bandwidth::bits_per(1_024 * 1_024 * 1_024, Duration::from_secs(1));
pub const DEFAULT_DOWNLOAD_BANDWIDTH: Bandwidth =
    Bandwidth::bits_per(8 * 1_024 * 1_024 * 1_024, Duration::from_secs(1));
pub const DEFAULT_PACKET_LOSS: PacketLoss = PacketLoss::NONE;
