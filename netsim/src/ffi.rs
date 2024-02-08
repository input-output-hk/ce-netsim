
mod ffi {
    use std::str;
    use ffi_support::ByteBuffer;

    type Address = u64;

    #[no_mangle]
    pub extern "C" fn add_numbers(a: i32, b: i32) -> i32 {
        a + b
    }

    #[no_mangle]
    pub extern "C" fn send_ffi(addr: Address, data: &ByteBuffer) -> bool {
        //let as_bb = ByteBuffer::from_vec(data.to_vec());
        send(addr, data.as_slice())
    }

    #[no_mangle]
    pub extern "C" fn receive_ffi(data: &mut ByteBuffer, addr: &mut Address) -> bool {
        let address: Address = 99; // Example value, replace it with the actual value
        let data_tmp: &[u8] = &[]; // Empty slice of u8
        let as_vec: Vec<u8> = data_tmp.to_vec();
        let as_bb = ByteBuffer::from(as_vec);
        *data = as_bb;
        *addr = address;
        true
    }

    pub fn send(addr: Address, data: &[u8]) -> bool {

        println!("Addr is {}", addr);
        match str::from_utf8(data) {
            Ok(s) => println!("String: {}", s),
            Err(e) => println!("Error: {}", e), // Handle UTF-8 decoding errors
        }
        true
    }

    // pub fn recv<'a>() -> (Address, &'a[u8]) {
    //
    // }

}

