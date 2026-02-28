use crate::{data::Data, node::NodeId};
use std::{
    fmt::{self},
    sync::{Arc, atomic::AtomicU64},
};
use thiserror::Error;

/// Error returned by [`PacketBuilder::build`] when a required field is missing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PacketBuildError {
    /// The sender node was not set via [`PacketBuilder::from`].
    #[error("missing sender information (`from')")]
    MissingSender,
    /// The recipient node was not set via [`PacketBuilder::to`].
    #[error("missing recipient information (`to')")]
    MissingRecipient,
    /// The payload was not set via [`PacketBuilder::data`].
    #[error("missing packet content (`data')")]
    MissingData,
}

/// A monotonically increasing generator for unique [`PacketId`] values.
///
/// Obtain a generator from [`Network::packet_id_generator`] or
/// `SimSocket::packet_id_generator` (in the `netsim` crate). It is backed by
/// an atomic counter so the same generator can be cloned and shared across
/// threads — every call to [`generate`](PacketIdGenerator::generate) will
/// produce a distinct ID.
///
/// IDs start at `1`. The generator will panic in a debug build (and produce
/// duplicates in a release build) only after `u64::MAX` packets have been
/// generated, which is not a practical concern.
///
/// [`Network::packet_id_generator`]: crate::network::Network::packet_id_generator
#[derive(Debug, Clone, Default)]
pub struct PacketIdGenerator(Arc<AtomicU64>);

/// A unique identifier for a single packet in the simulation.
///
/// Returned by [`Network::send`] and `SimSocket::send_to` (in the `netsim` crate). Use it to
/// correlate a sent packet with the one received on the other end:
///
/// ```
/// # use netsim_core::{network::{Network, Packet}, NodeId};
/// # let mut network = Network::<()>::new();
/// # let n1 = network.new_node().build();
/// # let n2 = network.new_node().build();
/// # network.configure_link(n1, n2).apply();
/// # let packet = Packet::builder(network.packet_id_generator()).from(n1).to(n2).data(()).build().unwrap();
/// // The ID returned by send matches the ID on the received packet.
/// let sent_id = network.send(packet).unwrap();
/// // (after advancing time) received_packet.id() == sent_id
/// ```
///
/// `PacketId` implements `Display` as a zero-padded 16-digit hex string
/// (`0x0000000000000001`), useful for log messages.
///
/// [`Network::send`]: crate::network::Network::send
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PacketId(u64);

/// Helper function to handle a resource when it is dropped.
///
/// This is particularly useful when we are using FFI and do not
/// want to use intermediate data representations (like a byte encoding).
/// The message can be allocated on the FFI side and be propagated through
/// the [`Network`]. If the message needs to be dropped by the
/// network simulator (because the network policy had it dropped or because
/// the data was corrupted on reception) then we can call this function
/// and _safely_ drop the resource.
///
/// [`Network`]: crate::network::Network
#[derive(Debug, Clone, Copy)]
struct OnDrop<T>(extern "C" fn(T));

/// An envelope that wraps user data for transit through the [`Network`].
///
/// A `Packet` carries:
/// - a unique [`PacketId`] assigned at build time,
/// - the sender's [`NodeId`] (`from`),
/// - the recipient's [`NodeId`] (`to`),
/// - the user payload (`data: T`),
/// - the byte-size of the payload (queried once at build time via [`Data::bytes_size`]),
/// - an optional FFI drop handler (see [`PacketBuilder::on_drop`]).
///
/// ## Building a packet
///
/// Use [`Packet::builder`] with a [`PacketIdGenerator`]:
///
/// ```
/// use netsim_core::{network::{Network, Packet}, NodeId};
///
/// let mut network = Network::<&str>::new();
/// let n1 = network.new_node().build();
/// let n2 = network.new_node().build();
/// network.configure_link(n1, n2).apply();
///
/// let packet = Packet::builder(network.packet_id_generator())
///     .from(n1)
///     .to(n2)
///     .data("hello")
///     .build()
///     .unwrap();
///
/// let packet_id = network.send(packet).unwrap();
/// ```
///
/// ## Accessing received data
///
/// After receiving a packet from [`Network::advance_with`] or
/// `SimSocket::recv_packet` (in the `netsim` crate), extract the payload with
/// [`into_inner`](Packet::into_inner):
///
/// ```
/// # use netsim_core::{network::{Network, Packet}, NodeId};
/// # use std::time::Duration;
/// # let mut network = Network::<&str>::new();
/// # let n1 = network.new_node().build();
/// # let n2 = network.new_node().build();
/// # network.configure_link(n1, n2).apply();
/// # let packet = Packet::builder(network.packet_id_generator()).from(n1).to(n2).data("hello").build().unwrap();
/// # network.send(packet).unwrap();
/// network.advance_with(Duration::from_millis(10), |packet| {
///     let msg: &str = packet.into_inner();
///     println!("received: {msg}");
/// });
/// ```
///
/// [`Data::bytes_size`]: crate::data::Data::bytes_size
/// [`Network`]: crate::network::Network
/// [`Network::advance_with`]: crate::network::Network::advance_with
pub struct Packet<T> {
    id: PacketId,
    from: NodeId,
    to: NodeId,
    bytes_size: u64,
    data: Option<T>,
    on_drop: Option<OnDrop<T>>,
}

/// Builder for constructing a [`Packet`].
///
/// Obtained via [`Packet::builder`]. All three of [`from`](PacketBuilder::from),
/// [`to`](PacketBuilder::to), and [`data`](PacketBuilder::data) must be set;
/// [`build`](PacketBuilder::build) returns an error if any is missing.
pub struct PacketBuilder<'a, T> {
    generator: &'a PacketIdGenerator,
    from: Option<NodeId>,
    to: Option<NodeId>,
    data: Option<T>,
    on_drop: Option<OnDrop<T>>,
}

impl PacketIdGenerator {
    /// Create a new generator. IDs start at `1`.
    pub fn new() -> Self {
        Self(Arc::new(AtomicU64::new(1)))
    }

    /// Generate a new globally unique [`PacketId`].
    ///
    /// IDs are assigned sequentially with `SeqCst` ordering so that clones of
    /// the same generator never produce duplicates, even across threads.
    ///
    /// # Overflow
    ///
    /// After `u64::MAX` IDs the internal counter wraps to `0`, which is
    /// detected by a `debug_assert!` (panics in debug builds, silent in
    /// release). This is not a practical concern — at 1 billion packets
    /// per second it would take ~584 years to exhaust the space.
    pub fn generate(&self) -> PacketId {
        let id = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        debug_assert!(
            id != 0,
            "PacketIdGenerator overflowed: u64::MAX packet IDs have been generated and the \
            counter wrapped to 0. This should never happen in practice."
        );

        PacketId(id)
    }
}

impl PacketId {
    /// a _NULL_ packet identifier (i.e. doesn't have a packet to it)
    #[cfg(test)]
    pub(crate) const NULL: Self = Self(0);
}

impl<'a, T> PacketBuilder<'a, T>
where
    T: Data,
{
    /// Create a new builder tied to the given ID generator.
    pub fn new(generator: &'a PacketIdGenerator) -> Self {
        Self {
            generator,
            from: None,
            to: None,
            data: None,
            on_drop: None,
        }
    }

    /// Set the sender node. Must be a [`NodeId`] that exists in the network.
    pub fn from(mut self, from: NodeId) -> Self {
        self.from = Some(from);
        self
    }

    /// Set the recipient node. Must be a [`NodeId`] that exists in the network.
    pub fn to(mut self, to: NodeId) -> Self {
        self.to = Some(to);
        self
    }

    /// Set the payload. [`Data::bytes_size`] is called here to capture the
    /// byte size for bandwidth accounting.
    ///
    /// [`Data::bytes_size`]: crate::data::Data::bytes_size
    pub fn data(mut self, data: T) -> Self {
        self.data = Some(data);
        self
    }

    /// Register an FFI-safe drop handler called when the [`Network`] discards
    /// the packet (e.g. due to buffer overflow or packet loss).
    ///
    /// This is intended for FFI scenarios where the payload is allocated on
    /// the C side and needs to be explicitly freed when dropped by the
    /// simulator. For pure-Rust usage the normal [`Drop`] implementation of
    /// `T` is sufficient and this method is not needed.
    ///
    /// [`Network`]: crate::network::Network
    pub fn on_drop(mut self, on_drop: extern "C" fn(T)) -> Self {
        self.on_drop = Some(OnDrop(on_drop));
        self
    }

    /// Finalise the packet.
    ///
    /// # Errors
    ///
    /// Returns an error if any of `from`, `to`, or `data` were not set.
    pub fn build(self) -> Result<Packet<T>, PacketBuildError> {
        let id = self.generator.generate();

        let Some(from) = self.from else {
            return Err(PacketBuildError::MissingSender);
        };
        let Some(to) = self.to else {
            return Err(PacketBuildError::MissingRecipient);
        };
        let Some(data) = self.data else {
            return Err(PacketBuildError::MissingData);
        };
        let bytes_size = data.bytes_size();
        let data = Some(data);
        let on_drop = self.on_drop;

        Ok(Packet {
            id,
            from,
            to,
            bytes_size,
            data,
            on_drop,
        })
    }
}

impl<T> Packet<T>
where
    T: Data,
{
    pub fn builder(generator: &PacketIdGenerator) -> PacketBuilder<'_, T> {
        PacketBuilder::new(generator)
    }
}

impl<T> Packet<T> {
    /// Returns the [`NodeId`] of the node that sent this packet.
    pub fn from(&self) -> NodeId {
        self.from
    }

    /// Returns the [`NodeId`] of the intended recipient.
    pub fn to(&self) -> NodeId {
        self.to
    }

    /// Returns the unique [`PacketId`] assigned when the packet was built.
    ///
    /// This is the same ID returned by [`Network::send`] and
    /// `SimSocket::send_to` (in the `netsim` crate), so you can correlate
    /// sends with receives.
    ///
    /// [`Network::send`]: crate::network::Network::send
    pub fn id(&self) -> PacketId {
        self.id
    }

    fn take_inner(&mut self) -> Option<T> {
        self.data.take()
    }

    /// Consume the packet and return the inner payload.
    ///
    /// Calling this prevents the `on_drop` callback (if any) from being
    /// invoked, because ownership of the data has been transferred to the
    /// caller.
    ///
    /// # Why this cannot panic
    ///
    /// `data` is always `Some` after construction. The only code that
    /// sets it to `None` is [`take_inner`](Self::take_inner), which is
    /// called in two places: here and in [`Drop::drop`]. Because
    /// `into_inner` takes `self` **by value**, `Drop` cannot have run
    /// yet — so `data` is guaranteed to still be `Some`.
    pub fn into_inner(mut self) -> T {
        let Some(data) = self.take_inner() else {
            panic!(
                "Packet::into_inner() called but data was already taken — this is a bug in Packet"
            )
        };
        data
    }
}

impl<T> Drop for Packet<T> {
    fn drop(&mut self) {
        if let Some(data) = self.take_inner()
            && let Some(on_drop) = self.on_drop.take()
        {
            on_drop.0(data);
        }
    }
}

impl<T> Data for Packet<T>
where
    T: Data + Send + 'static,
{
    fn bytes_size(&self) -> u64 {
        self.bytes_size
    }
}
//unsafe impl<T> Send for Packet<T> {}
impl<T> fmt::Debug for Packet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(&format!("Packet<{}>", std::any::type_name::<T>()))
            .field("from", &self.from)
            .field("to", &self.to)
            .field("bytes_size", &self.bytes_size)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for PacketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016x}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_id_null() {
        let null = PacketId::NULL;

        assert_eq!(null, PacketId(0));
        assert_eq!(null.to_string(), "0x0000000000000000");
        assert_eq!(format!("{null:?}"), "PacketId(0)");
    }

    #[test]
    fn builder_missing_from() {
        let err = Packet::<()>::builder(&PacketIdGenerator::new())
            .build()
            .unwrap_err();
        assert_eq!(err, PacketBuildError::MissingSender);
    }

    #[test]
    fn builder_missing_to() {
        let err = Packet::<()>::builder(&PacketIdGenerator::new())
            .from(NodeId::ZERO)
            .build()
            .unwrap_err();
        assert_eq!(err, PacketBuildError::MissingRecipient);
    }

    #[test]
    fn builder_missing_data() {
        let err = Packet::<()>::builder(&PacketIdGenerator::new())
            .from(NodeId::ZERO)
            .to(NodeId::ONE)
            .build()
            .unwrap_err();
        assert_eq!(err, PacketBuildError::MissingData);
    }

    #[test]
    fn builder_without_on_drop() {
        let _packet = Packet::builder(&PacketIdGenerator::new())
            .from(NodeId::ZERO)
            .to(NodeId::ONE)
            .data(())
            .build()
            .expect("Should be possible to build a packet without data");
    }

    #[test]
    fn builder_with_on_drop() {
        extern "C" fn on_drop(_: u8) {}

        let _packet = Packet::builder(&PacketIdGenerator::new())
            .from(NodeId::ZERO)
            .to(NodeId::ONE)
            .data(0)
            .on_drop(on_drop)
            .build()
            .expect("Should be possible to build a packet without data");
    }

    #[test]
    fn packet_manual_drop() {
        use std::sync::atomic::{AtomicU8, Ordering};

        static COUNTER: AtomicU8 = AtomicU8::new(0);
        extern "C" fn on_drop(value: u8) {
            COUNTER.store(value, Ordering::Relaxed);
        }

        let packet = Packet::builder(&PacketIdGenerator::new())
            .from(NodeId::ZERO)
            .to(NodeId::ONE)
            .data(1u8)
            .on_drop(on_drop)
            .build()
            .expect("Should be possible to build a packet without data");

        std::mem::drop(packet);

        assert_eq!(COUNTER.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn packet_into_inner_no_drop() {
        extern "C" fn on_drop(_value: u8) {
            panic!("On drop shouldn't be called")
        }

        let packet = Packet::builder(&PacketIdGenerator::new())
            .from(NodeId::ZERO)
            .to(NodeId::ONE)
            .data(1u8)
            .on_drop(on_drop)
            .build()
            .expect("Should be possible to build a packet without data");

        let inner = packet.into_inner();

        assert_eq!(inner, 1);
    }
}
