pub use netsim::SimId;

pub type SimContext = netsim::SimContext<Box<[u8]>>;

pub type SimSocket = netsim::SimSocket<Box<[u8]>>;

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
pub unsafe extern "C" fn netsim_context_new(output: *mut *mut SimContext) -> SimError {
    if output.is_null() {
        return SimError::NotImplemented;
    }

    let configuration = netsim::SimConfiguration::default();
    let context = Box::new(SimContext::new(configuration));

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
        match context.shutdown() {
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
        match context_mut.open() {
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

/// Access the unique dentifier of the [`SimSocket`]
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
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
/// This function will block until a message is received.
/// The function expects size to contain the size of the buffer provided.
/// The data received from the "socket" will be copied into the buffer up to the size but not beyond
/// This implies the buffer will not contain the whole message if the message length
/// is greater than the size of the provided buffer.
/// Finally the size is updated to reflect the length o the data copied into the buffer.
/// If no data is available from the socket, a NoMoreData error is returned.
#[no_mangle]
pub unsafe extern "C" fn netsim_socket_recv(
    socket: *mut SimSocket,
    // pre-allocated byte array
    msg: *mut u8,
    // the maximum size of the pre-allocated byte array
    size: *mut u64,
    // where we will put the sender ID
    from: *mut SimId,
) -> SimError {
    let Some(socket) = socket.as_mut() else {
        return SimError::NullPointerArgument;
    };
    let output = std::slice::from_raw_parts_mut(msg, (*size) as usize);

    let Some((id, data)) = socket.recv() else {
        // this is usually to signal it is time to release
        // the socket
        return SimError::SocketDisconnected;
    };

    *from = id;

    // we need to take the minimum value between
    // what the caller had allocated

    let copy_length = output.len().min(data.len()); // Determine the max length to copy
    output[..copy_length].copy_from_slice(&data[..copy_length]);

    *size = copy_length as u64;

    SimError::Success
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
    msg: *mut u8,
    // the maximum size of the pre-allocated byte array
    size: u64,
) -> SimError {
    let Some(socket) = socket.as_mut() else {
        return SimError::NullPointerArgument;
    };
    let msg = std::slice::from_raw_parts(msg, size as usize)
        .to_vec()
        .into_boxed_slice();

    if let Err(error) = socket.send_to(to, msg) {
        eprintln!("{error:?}");
        return SimError::Undefined;
    }

    SimError::Success
}
