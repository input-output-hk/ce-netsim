use anyhow::{anyhow, Context, Result};
use netsim_core::{Bandwidth, NodeId, Packet};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError, TrySendError};

pub(crate) enum Command<T> {
    Packet(Packet<T>),
    NewNode(NewNodeCommand<T>, SyncSender<NodeId>),
}

pub(crate) struct NewNodeCommand<T> {
    // where to send messages onces they are received
    pub(crate) sender: SyncSender<Packet<T>>,

    // initial upload bandwidth
    pub(crate) upload_bandwidth: Bandwidth,
    pub(crate) upload_buffer: u64,

    pub(crate) download_bandwidth: Bandwidth,
    pub(crate) download_buffer: u64,
}

pub(crate) struct CommandSender<T>(SyncSender<Command<T>>);

pub(crate) struct CommandReceiver<T>(Receiver<Command<T>>);

pub(crate) fn command_channel<T>() -> (CommandSender<T>, CommandReceiver<T>) {
    let (sender, receiver) = sync_channel(1_024 * 1_024);

    (CommandSender(sender), CommandReceiver(receiver))
}

impl<T> CommandSender<T> {
    pub(crate) fn send(&mut self, command: Command<T>) -> Result<(), TrySendError<Command<T>>> {
        self.0.try_send(command)
    }

    pub(crate) fn send_packet(&mut self, packet: Packet<T>) -> Result<(), TrySendError<Packet<T>>> {
        self.send(Command::Packet(packet)).map_err(|err| match err {
            // conver the error to remove the command part
            TrySendError::Disconnected(Command::Packet(packet)) => {
                TrySendError::Disconnected(packet)
            }
            TrySendError::Full(Command::Packet(packet)) => TrySendError::Full(packet),

            // unreachable cases
            TrySendError::Disconnected(_) | TrySendError::Full(_) => {
                unreachable!("We should only get one of the command with packets")
            }
        })
    }

    pub(crate) fn send_new_node(&mut self, new_node: NewNodeCommand<T>) -> Result<NodeId> {
        let (reply, answer) = sync_channel(1);

        self.send(Command::NewNode(new_node, reply))
            .map_err(|error| anyhow!("Failed to send new node command: {error}"))?;

        answer
            .recv()
            .context("Failed to receive response from Multiplexer about adding a new node.")
    }
}

impl<T> Clone for CommandSender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> CommandReceiver<T> {
    pub(crate) fn try_recv(&mut self) -> Result<Command<T>, TryRecvError> {
        self.0.try_recv()
    }
}
