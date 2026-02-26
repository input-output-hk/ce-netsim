use crate::{data::Data, node::NodeId};
use anyhow::{Result, bail};
use std::{
    fmt::{self},
    sync::{Arc, atomic::AtomicU64},
};

/// a generator for monotonicaly increasing **unique** [`PacketId`]
///
#[derive(Debug, Clone, Default)]
pub struct PacketIdGenerator(Arc<AtomicU64>);

/// # [`Packet`] Identifier
///
/// During the lifetime of the packet, this identifier can uniquely
/// identify the packet.
///
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

/// # A wrapped message
///
/// A [`Packet`] is a message with the sender identifier, the
/// receiver identifier and the message's data itself. It also
/// embed other metadata and the optional method to handle
/// the message's data on drop.
///
pub struct Packet<T> {
    id: PacketId,
    from: NodeId,
    to: NodeId,
    bytes_size: u64,
    data: Option<T>,
    on_drop: Option<OnDrop<T>>,
}

pub struct PacketBuilder<'a, T> {
    generator: &'a PacketIdGenerator,
    from: Option<NodeId>,
    to: Option<NodeId>,
    data: Option<T>,
    on_drop: Option<OnDrop<T>>,
}

impl PacketIdGenerator {
    pub fn new() -> Self {
        Self(Arc::new(AtomicU64::new(1)))
    }

    /// generate a new unique identifier
    pub fn generate(&self) -> PacketId {
        let id = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        debug_assert!(
            id != 0,
            "The only case this can be equal to 0 is if the generator overflowed. If this \
            happens it means we have generated `u64::MAX` unique paquet identifier and we \
            wrapped around on overflow. This shouldn't happen!"
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
    pub fn new(generator: &'a PacketIdGenerator) -> Self {
        Self {
            generator,
            from: None,
            to: None,
            data: None,
            on_drop: None,
        }
    }

    pub fn from(mut self, from: NodeId) -> Self {
        self.from = Some(from);
        self
    }

    pub fn to(mut self, to: NodeId) -> Self {
        self.to = Some(to);
        self
    }

    pub fn data(mut self, data: T) -> Self {
        self.data = Some(data);
        self
    }

    pub fn on_drop(mut self, on_drop: extern "C" fn(T)) -> Self {
        self.on_drop = Some(OnDrop(on_drop));
        self
    }

    pub fn build(self) -> Result<Packet<T>> {
        let id = self.generator.generate();

        let Some(from) = self.from else {
            bail!("Missing sender information (`from')")
        };
        let Some(to) = self.to else {
            bail!("Missing recipient information (`to')")
        };
        let Some(data) = self.data else {
            bail!("Missing packet content (`data')")
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
    pub fn from(&self) -> NodeId {
        self.from
    }

    pub fn to(&self) -> NodeId {
        self.to
    }

    pub fn id(&self) -> PacketId {
        self.id
    }

    fn take_inner(&mut self) -> Option<T> {
        self.data.take()
    }

    /// consume the packet and get the inner `T`.
    ///
    pub fn into_inner(mut self) -> T {
        self.take_inner()
            .expect("We should always have the data available")
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
unsafe impl<T> Send for Packet<T> {}
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
        let Err(error) = Packet::<()>::builder(&PacketIdGenerator::new()).build() else {
            panic!("Expecting an error because missing the `from'")
        };

        assert_eq!(error.to_string(), "Missing sender information (`from')");
    }

    #[test]
    fn builder_missing_to() {
        let Err(error) = Packet::<()>::builder(&PacketIdGenerator::new())
            .from(NodeId::ZERO)
            .build()
        else {
            panic!("Expecting an error because missing the `to'")
        };

        assert_eq!(error.to_string(), "Missing recipient information (`to')");
    }

    #[test]
    fn builder_missing_data() {
        let Err(error) = Packet::<()>::builder(&PacketIdGenerator::new())
            .from(NodeId::ZERO)
            .to(NodeId::ONE)
            .build()
        else {
            panic!("Expecting an error because missing the `data'")
        };

        assert_eq!(error.to_string(), "Missing packet content (`data')");
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
        static mut COUNTER: u8 = 0;
        extern "C" fn on_drop(value: u8) {
            unsafe { COUNTER = value }
        }

        let packet = Packet::builder(&PacketIdGenerator::new())
            .from(NodeId::ZERO)
            .to(NodeId::ONE)
            .data(1u8)
            .on_drop(on_drop)
            .build()
            .expect("Should be possible to build a packet without data");

        std::mem::drop(packet);

        let counter = unsafe { COUNTER };
        assert_eq!(counter, 1);
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
