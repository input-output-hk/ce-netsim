use ffi_support::{ByteBuffer, ExternError};
use lazy_static::lazy_static;
use std::collections::VecDeque;
use std::slice;
use std::ffi::CString;
use std::io::Read;
use std::sync::{Arc, Mutex};
use ce_netsim::{SimConfiguration, SimContext, SimId, SimSocket, SimSocketConfiguration};

type Address = u64;

/**
Implement this trait to hook up calls from external processes to netsim.
External implementations will ask for messages and send messages through this trait.
**/
pub trait Ffi {
    fn send(&self, addr: Address, data: &[u8]) -> bool;
    fn recv(&self) -> Option<(Address, Vec<u8>)>;
}

pub struct DummyFfi {
    #[allow(clippy::type_complexity)]
    queue: Arc<Mutex<VecDeque<(Address, Vec<u8>)>>>,
}

impl DummyFfi {
    fn default() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
}

impl Ffi for DummyFfi {
    fn send(&self, addr: Address, data: &[u8]) -> bool {
        self.queue.lock().unwrap().push_back((addr, data.to_vec()));
        true
    }
    fn recv(&self) -> Option<(Address, Vec<u8>)> {
        self.queue.lock().unwrap().pop_front()
    }
}

lazy_static! {
    pub static ref FFI_IMPL: DummyFfi = DummyFfi::default();
}

#[no_mangle]
pub extern "C" fn netsim_send(err: &mut ExternError,
                              sim_id: SimId,
                              sim_socket: &'static SimSocket<&str>,
                              data: &ByteBuffer) -> bool {
    unsafe {
        // Convert the raw pointer to a slice of bytes
        let byte_slice = slice::from_raw_parts(data, data.len() as usize);

        // Attempt to convert the byte slice to a string slice
        std::str::from_utf8(byte_slice).ok()
    }
    let s: &str = "";
    //let s = std::str::from_utf8(&data.destroy_into_vec()).unwrap();
    sim_socket.send_to(sim_id, s).unwrap();
    true
}

#[no_mangle]
pub unsafe extern "C" fn netsim_new_context(err: &mut ExternError, context_ptr: &'static mut SimContext<&str>) -> bool {
    let configuration = SimConfiguration {};
    let mut context: SimContext<&'static str> = SimContext::new(configuration);
    *context_ptr = context;
    true
}

#[no_mangle]
pub unsafe extern "C" fn netsim_open_context(err: &mut ExternError,
                                             context: &'static mut SimContext<&str>,
                                             sim_socket: &'static mut SimSocket<&str>) -> bool {
    let cfg = SimSocketConfiguration::default();
    let r = context.open(cfg);
    let net = r.unwrap();
    *sim_socket = net;
    true
}

#[no_mangle]
pub extern "C" fn netsim_receive(data: &mut ByteBuffer, addr: &mut Address) -> bool {
    let Some((address, data_tmp)) = FFI_IMPL.recv() else {
        return false;
    };

    let as_bb = ByteBuffer::from(data_tmp);
    *data = as_bb;
    *addr = address;
    true
}
