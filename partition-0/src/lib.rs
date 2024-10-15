#[no_mangle]
pub fn process() -> i32 {
    // use core::slice;
    // let data = unsafe { slice::from_raw_parts(data_ptr as *mut u8, count as _) };
    // let input = f32::from_le_bytes(data.try_into().unwrap());
    unsafe {
        OUTPUT.copy_from_slice(INPUT.as_ref());
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
