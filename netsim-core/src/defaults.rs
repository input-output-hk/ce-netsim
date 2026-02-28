use crate::measure::{Bandwidth, Latency};
use std::time::Duration;

/// Default [`Latency`]
///
/// This is the default value that is used for a [`Link`]
/// [`Latency`].
///
/// ```
/// # use netsim_core::defaults::*;
/// assert_eq!(
///     DEFAULT_LATENCY.to_string(),
///     "5ms"
/// );
/// ```
///
/// [`Link`]: crate::link::Link
pub const DEFAULT_LATENCY: Latency = Latency::new(Duration::from_millis(5));

/// Default upload buffer size
///
/// this is the socket's local upload size. By default we have set a (virtually)
/// infinite size for the buffer.
///
/// See [`Node::set_upload_buffer`] for more details
///
/// [`Node::set_upload_buffer`]: crate::node::Node::set_upload_buffer
///
pub const DEFAULT_UPLOAD_BUFFER: u64 = u64::MAX;

/// Default upload bandwidth
///
/// This is the value that is used to define the default bandwidth
/// for uploading data in the network simulation.
///
/// ```
/// # use netsim_core::defaults::*;
/// // 500 MiB/s = 500 * 1_048_576 bytes/s * 8 bits/byte = 4_194_304_000 bps
/// assert_eq!(
///     DEFAULT_UPLOAD_BANDWIDTH.to_string(),
///     "4.19gbps"
/// );
/// ```
///
#[allow(clippy::declare_interior_mutable_const)]
pub const DEFAULT_UPLOAD_BANDWIDTH: Bandwidth = Bandwidth::new(500 * 1_024 * 1_024 * 8);

/// Default download buffer size
///
/// this is the socket's local download size. By default we have set a (virtually)
/// infinite size for the buffer.
///
/// See [`Node::set_download_buffer`] for more details
///
/// [`Node::set_download_buffer`]: crate::node::Node::set_download_buffer
///
pub const DEFAULT_DOWNLOAD_BUFFER: u64 = u64::MAX;

/// Default download bandwidth
///
/// This is the value that is used to define the default bandwidth
/// for downloading data in the network simulation.
///
/// ```
/// # use netsim_core::defaults::*;
/// // 1 GiB/s = 1_073_741_824 bytes/s * 8 bits/byte = 8_589_934_592 bps
/// assert_eq!(
///     DEFAULT_DOWNLOAD_BANDWIDTH.to_string(),
///     "8.59gbps"
/// );
/// ```
///
#[allow(clippy::declare_interior_mutable_const)]
pub const DEFAULT_DOWNLOAD_BANDWIDTH: Bandwidth = Bandwidth::new(1_024 * 1_024 * 1_024 * 8);
