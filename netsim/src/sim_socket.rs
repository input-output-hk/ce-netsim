use crate::{
    sim_link::{SimDownLink, SimUpLink},
    HasBytesSize, SimId,
};
use anyhow::Result;
use netsim_core::{BusSender, Msg};
use std::sync::mpsc;

pub struct SimSocket<T>
where
    T: HasBytesSize,
{
    reader: SimSocketReadHalf<T>,
    writer: SimSocketWriteHalf<T>,
}

pub struct SimSocketReadHalf<T> {
    id: SimId,
    down: SimDownLink<T>,
}

pub struct SimSocketWriteHalf<T>
where
    T: HasBytesSize,
{
    id: SimId,
    up: BusSender<SimUpLink<T>>,
}

/// Result from [`SimSocket::try_recv`] or [`SimSocketReadHalf::try_recv`]
///
pub enum TryRecv<T> {
    /// A message was available
    Some(T),
    /// no messages available
    NoMsg,
    /// the [SimSocket] has been disconnected
    ///
    /// This means the [`crate::SimContext`] has been dropped or shutdown
    Disconnected,
}

impl<T> SimSocket<T>
where
    T: HasBytesSize,
{
    pub(crate) fn new(
        id: SimId,
        to_bus: BusSender<SimUpLink<T>>,
        receiver: SimDownLink<T>,
    ) -> Self {
        Self {
            reader: SimSocketReadHalf { id, down: receiver },
            writer: SimSocketWriteHalf { id, up: to_bus },
        }
    }

    #[inline]
    pub fn id(&self) -> SimId {
        self.reader.id()
    }

    pub fn into_split(self) -> (SimSocketReadHalf<T>, SimSocketWriteHalf<T>) {
        let Self { reader, writer } = self;

        (reader, writer)
    }
}

impl<T> SimSocket<T>
where
    T: HasBytesSize,
{
    pub fn send_to(&self, to: SimId, msg: T) -> Result<()> {
        self.writer.send_to(to, msg)
    }

    /// blocking call to receiving message on the channel
    ///
    /// returns None if the sending end has disconnected (no more senders)
    pub fn recv(&mut self) -> Option<(SimId, T)> {
        self.reader.recv()
    }

    /// Non blocking call to receiving message on the channel
    ///
    pub fn try_recv(&mut self) -> TryRecv<(SimId, T)> {
        self.reader.try_recv()
    }
}

impl<T: HasBytesSize> SimSocketWriteHalf<T> {
    #[inline]
    pub fn id(&self) -> SimId {
        self.id
    }
}

impl<T> SimSocketWriteHalf<T>
where
    T: HasBytesSize,
{
    pub fn send_to(&self, to: SimId, msg: T) -> Result<()> {
        let msg = Msg::new(self.id, to, msg);
        self.up.send_msg(msg)
    }
}

impl<T> SimSocketReadHalf<T> {
    #[inline]
    pub fn id(&self) -> SimId {
        self.id
    }
}

impl<T> SimSocketReadHalf<T>
where
    T: HasBytesSize,
{
    /// blocking call to receiving a message from the network
    pub fn recv(&mut self) -> Option<(SimId, T)> {
        let msg = self.down.recv()?;

        Some((msg.from(), msg.into_content()))
    }

    /// non blocking call to receiving message on the channel
    ///
    pub fn try_recv(&mut self) -> TryRecv<(SimId, T)> {
        match self.down.try_recv() {
            Ok(msg) => TryRecv::Some((msg.from(), msg.into_content())),
            Err(mpsc::TryRecvError::Empty) => TryRecv::NoMsg,
            Err(mpsc::TryRecvError::Disconnected) => TryRecv::Disconnected,
        }
    }
}
