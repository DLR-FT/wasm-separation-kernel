#[no_mangle]
pub fn adder(a: i32, b: i32) -> i32 {
    a + b
}

// #[no_mangle]
// pub fn alloc(len: usize) -> *mut u8 {
//     // create a new mutable buffer with capacity `len`
//     let mut buf = Vec::with_capacity(len);
//     // take a mutable pointer to the buffer
//     let ptr = buf.as_mut_ptr();
//     // take ownership of the memory block and
//     // ensure that its destructor is not
//     // called when the object goes out of scope
//     // at the end of the function
//     std::mem::forget(buf);
//     // return the pointer so the runtime
//     // can write data at this offset
//     return ptr;
// }

#[no_mangle]
pub fn process_data(_data_ptr: u32, _count: u32) -> i32 {
    // use core::slice;
    // let data = unsafe { slice::from_raw_parts(data_ptr as *mut u8, count as _) };
    // let input = f32::from_le_bytes(data.try_into().unwrap());
    unsafe {
        OUTPUT.copy_from_slice(&INPUT);
        OUTPUT[0] += 1;
        OUTPUT[1] += 2;
        OUTPUT[2] += 3;
        OUTPUT[3] += 4;
    };
    unsafe { INPUT.iter().map(|x| *x as i32).sum() }
}

#[no_mangle]
pub static mut INPUT: [u8; 4] = [0u8; 4];

#[no_mangle]
pub static mut OUTPUT: [u8; 4] = [4u8; 4];
