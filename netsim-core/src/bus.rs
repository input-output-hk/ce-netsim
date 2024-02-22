use crate::Msg;
use anyhow::{anyhow, Result};
use std::sync::mpsc;

pub enum BusMessage<T> {
    Message(Msg<T>),
    Shutdown,
    Disconnected,
}

pub struct BusSender<T> {
    sender: mpsc::Sender<BusMessage<T>>,
}

pub(crate) struct BusReceiver<T> {
    receiver: mpsc::Receiver<BusMessage<T>>,
}

pub(crate) fn open_bus<T>() -> (BusSender<T>, BusReceiver<T>) {
    let (sender, receiver) = mpsc::channel();
    (BusSender::new(sender), BusReceiver::new(receiver))
}

impl<T> BusSender<T> {
    fn new(sender: mpsc::Sender<BusMessage<T>>) -> Self {
        Self { sender }
    }

    pub fn send_msg(&self, msg: Msg<T>) -> Result<()> {
        self.sender
            .send(BusMessage::Message(msg))
            .map_err(|error| anyhow!("failed to send message: {error}"))
    }

    pub(crate) fn send_shutdown(&self) -> Result<()> {
        self.sender
            .send(BusMessage::Shutdown)
            .map_err(|error| anyhow!("failed to send message: {error}"))
    }
}

impl<T> BusReceiver<T> {
    fn new(receiver: mpsc::Receiver<BusMessage<T>>) -> Self {
        Self { receiver }
    }

    pub(crate) fn try_receive(&mut self) -> Option<BusMessage<T>> {
        match self.receiver.try_recv() {
            Ok(bus_msg) => Some(bus_msg),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(BusMessage::Disconnected),
        }
    }
}

impl<T> Clone for BusSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}
