use crate::{data::Data, node::NodeId};
use anyhow::{bail, Result};
use std::{fmt, hash::Hash};

/// # [`Packet`] Identifier
///
/// During the lifetime of the packet, this identifier can uniquely
/// identify the packet.
///
#[derive(Debug, Clone, Copy)]
pub struct PacketId(*const ());

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
    from: NodeId,
    to: NodeId,
    bytes_size: u64,
    data: *const T,
    on_drop: Option<OnDrop<T>>,
}

pub struct PacketBuilder<T> {
    from: Option<NodeId>,
    to: Option<NodeId>,
    data: Option<T>,
    on_drop: Option<OnDrop<T>>,
}

impl PacketId {
    /// a _NULL_ packet identifier (i.e. doesn't have a packet to it)
    #[cfg(test)]
    pub(crate) const NULL: Self = Self(std::ptr::null());
}

impl<T> PacketBuilder<T>
where
    T: Data,
{
    pub fn new() -> Self {
        Self::default()
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
        let data = Box::into_raw(Box::new(data));
        let on_drop = self.on_drop;

        Ok(Packet {
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
    pub fn builder() -> PacketBuilder<T> {
        PacketBuilder::new()
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
        let id = unsafe {
            // hide the type so that we don't have to deal with
            // the `T` everywhere in our code.
            std::mem::transmute::<*const T, *const ()>(self.data)
        };

        PacketId(id)
    }

    unsafe fn take_inner(&mut self) -> Option<T> {
        if self.data.is_null() {
            return None;
        }

        let ptr = std::mem::replace(&mut self.data, std::ptr::null());
        let boxed = Box::from_raw(ptr as *mut T);

        let data = *boxed;

        Some(data)
    }

    /// consume the packet and get the inner `T`.
    ///
    pub fn into_inner(mut self) -> T {
        unsafe {
            self.take_inner()
                .expect("We should always have the data available")
        }
    }
}

impl<T> Drop for Packet<T> {
    fn drop(&mut self) {
        if let Some(data) = unsafe { self.take_inner() } {
            if let Some(on_drop) = self.on_drop.take() {
                on_drop.0(data);
            }
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

impl PartialEq for PacketId {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}
impl Eq for PacketId {}
impl Hash for PacketId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (self.0 as u64).hash(state);
    }
}
impl fmt::Display for PacketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016X}", self.0 as u64)
    }
}

impl<T> Default for PacketBuilder<T> {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            data: None,
            on_drop: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FROM: NodeId = NodeId::new(0);
    const TO: NodeId = NodeId::new(1);

    fn packet<T>(data: T) -> Packet<T>
    where
        T: Data,
    {
        Packet::builder()
            .from(FROM)
            .to(TO)
            .data(data)
            .build()
            .unwrap()
    }

    #[test]
    fn packet_id_null() {
        let null = PacketId::NULL;

        assert_eq!(null, PacketId(std::ptr::null()));
        assert_eq!(null.to_string(), "0x0000000000000000");
        assert_eq!(format!("{null:?}"), "PacketId(0x0)");
    }

    #[test]
    fn packet_id_eq() {
        let packet = packet([0; 2]);
        let id1 = packet.id();
        let id2 = PacketId(packet.data as *const ());

        std::mem::drop(packet);

        assert_eq!(id1, id2);
    }

    #[test]
    fn packet_id_ne() {
        let packet1 = packet([0; 2]);
        let packet2 = packet([0; 2]);

        assert_ne!(packet1.id(), packet2.id());
    }

    #[test]
    fn builder_missing_from() {
        let Err(error) = Packet::<()>::builder().build() else {
            panic!("Expecting an error because missing the `from'")
        };

        assert_eq!(error.to_string(), "Missing sender information (`from')");
    }

    #[test]
    fn builder_missing_to() {
        let Err(error) = Packet::<()>::builder().from(NodeId::ZERO).build() else {
            panic!("Expecting an error because missing the `to'")
        };

        assert_eq!(error.to_string(), "Missing recipient information (`to')");
    }

    #[test]
    fn builder_missing_data() {
        let Err(error) = Packet::<()>::builder()
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
        let _packet = Packet::builder()
            .from(NodeId::ZERO)
            .to(NodeId::ONE)
            .data(())
            .build()
            .expect("Should be possible to build a packet without data");
    }

    #[test]
    fn builder_with_on_drop() {
        extern "C" fn on_drop(_: u8) {}

        let _packet = Packet::builder()
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

        let packet = Packet::builder()
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

        let packet = Packet::builder()
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
