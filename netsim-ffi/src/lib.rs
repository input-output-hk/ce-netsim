use ffi_support::ByteBuffer;
use lazy_static::lazy_static;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
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
pub extern "C" fn send_ffi(addr: Address, data: &ByteBuffer) -> bool {
    FFI_IMPL.send(addr, data.as_slice())
}

#[no_mangle]
pub extern "C" fn receive_ffi(data: &mut ByteBuffer, addr: &mut Address) -> bool {
    let Some((address, data_tmp)) = FFI_IMPL.recv() else {
        return false;
    };

    let as_bb = ByteBuffer::from(data_tmp);
    *data = as_bb;
    *addr = address;
    true
}
