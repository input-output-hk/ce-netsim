use std::{
    ffi::c_void,
    ops::{Deref, DerefMut},
};

pub use netsim::SimId;
use netsim::{HasBytesSize, SimContext as OSimContext, SimSocket as OSimSocket};

#[repr(C)]
pub struct Message {
    pub pointer: *mut c_void,
    pub size: u64,
}

unsafe impl Send for Message {}
unsafe impl Sync for Message {}

impl HasBytesSize for Message {
    fn bytes_size(&self) -> u64 {
        self.size
    }
}

pub struct SimContext(OSimContext<Message>);
pub struct SimSocket(OSimSocket<Message>);

#[repr(u32)]
pub enum SimError {
    /// the function succeed, no error
    Success = 0,

    /// An undefined error
    Undefined = 1,

    /// the function was called with an unexpected null pointer
    NullPointerArgument = 3,

    /// The function is not yet implemented, please report this issue
    /// to maintainers
    NotImplemented = 4,

    /// This indicates it's time to release the socket
    SocketDisconnected = 5,
}

/// Create a new NetSim Context
///
/// This is configured so that messages of type Box<u8> can be shared through
/// the network between nodes.
///
/// # Safety
///
/// This function allocate a pointer upon success and returns the pointer
/// address. Call [`netsim_context_shutdown`] to release the resource.
///
#[no_mangle]
pub unsafe extern "C" fn netsim_context_new(
    output: *mut *mut SimContext,
    on_drop: extern "C" fn(Message),
) -> SimError {
    if output.is_null() {
        return SimError::NotImplemented;
    }

    let configuration = netsim::SimConfiguration {
        on_drop: Some(on_drop.into()),
        ..Default::default()
    };
    let context = Box::new(SimContext(OSimContext::with_config(configuration)));

    *output = Box::into_raw(context);
    SimError::Success
}

/// Shutdown a NetSim context and release assets
///
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
///
#[no_mangle]
pub unsafe extern "C" fn netsim_context_shutdown(context: *mut SimContext) -> SimError {
    if context.is_null() {
        SimError::NullPointerArgument
    } else {
        let context = Box::from_raw(context);
        // SimContext::shutdown takes ownership of the SimContext
        // when using `context.shutdown()` we are relying on the
        // `Deref::deref` function to gain us access to the object
        // so here we bypass the _dereference_ and move `0` (the context)
        // out and call shutdown on it.
        match context.0.shutdown() {
            Ok(()) => SimError::Success,
            Err(error) => {
                // better handle the error, maybe print it to the standard err output
                eprintln!("{error:?}");
                SimError::Undefined
            }
        }
    }
}

/// create a new [`SimSocket`] in the given context
///
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
///
#[no_mangle]
pub unsafe extern "C" fn netsim_context_open(
    context: *mut SimContext,
    output: *mut *mut SimSocket,
) -> SimError {
    if context.is_null() || output.is_null() {
        SimError::NullPointerArgument
    } else {
        let Some(context_mut) = context.as_mut() else {
            return SimError::NullPointerArgument;
        };
        match context_mut.open().map(SimSocket) {
            Ok(sim_socket) => {
                *output = Box::into_raw(Box::new(sim_socket));
                SimError::Success
            }
            Err(error) => {
                // better handle the error, maybe print it to the standard err output
                eprintln!("{error:?}");
                SimError::Undefined
            }
        }
    }
}

/// Access the unique identifier of the [`SimSocket`]
///
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
///
#[no_mangle]
pub unsafe extern "C" fn netsim_socket_id(socket: *mut SimSocket, id: *mut SimId) -> SimError {
    let Some(socket) = socket.as_ref() else {
        return SimError::NullPointerArgument;
    };
    let Some(id) = id.as_mut() else {
        return SimError::NullPointerArgument;
    };

    *id = socket.id();

    SimError::Success
}

/// Release the new [`SimSocket`] resources
///
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
///
#[no_mangle]
pub unsafe extern "C" fn netsim_socket_release(socket: *mut SimSocket) -> SimError {
    if socket.is_null() {
        SimError::NullPointerArgument
    } else {
        let _ = Box::from_raw(socket);
        SimError::Success
    }
}

/// Receive a message from the [`SimSocket`]
///
/// On success the function populate the pointed value `msg` with the
/// received message. As well as `from` with the sender of the message.
///
/// # Safety
///
/// The function checks the parameters to be non null before trying
/// to utilise it. However if the pointers point to a random memory then
/// the function may have unexpected behaviour. Same for `msg` and `from`
///
#[no_mangle]
pub unsafe extern "C" fn netsim_socket_recv(
    socket: *mut SimSocket,
    // pre-allocated byte array
    msg: *mut Message,
    // where we will put the sender ID
    from: *mut SimId,
) -> SimError {
    let Some(socket) = socket.as_mut() else {
        return SimError::NullPointerArgument;
    };
    let Some(msg) = msg.as_mut() else {
        return SimError::NullPointerArgument;
    };
    let Some(from) = from.as_mut() else {
        return SimError::NullPointerArgument;
    };

    if let Some((id, data)) = socket.recv() {
        *msg = data;
        *from = id;

        SimError::Success
    } else {
        // this is usually to signal it is time to release
        // the socket
        SimError::SocketDisconnected
    }
}

/// Send a message to the [`SimSocket`]
///
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
/// This function returns immediately.
///
#[no_mangle]
pub unsafe extern "C" fn netsim_socket_send_to(
    socket: *mut SimSocket,
    // where we will put the sender ID
    to: SimId,
    // pre-allocated byte array
    msg: Message,
) -> SimError {
    let Some(socket) = socket.as_mut() else {
        return SimError::NullPointerArgument;
    };

    if let Err(error) = socket.send_to(to, msg) {
        eprintln!("{error:?}");
        return SimError::Undefined;
    }

    SimError::Success
}

impl Deref for SimContext {
    type Target = OSimContext<Message>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for SimContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Deref for SimSocket {
    type Target = OSimSocket<Message>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for SimSocket {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
