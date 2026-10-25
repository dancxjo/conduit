//! Independent bounded canonical startup bytes; never recovered from a Plan.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;
thread_local! {
    static LOOK_INPUT: RefCell<Box<[u8]>> = RefCell::new(vec![0; conduit_face::MAX_LOOK_BYTES].into_boxed_slice());
}
#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_history_look_input_ptr() -> usize {
    LOOK_INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_history_look_input_capacity() -> usize {
    conduit_face::MAX_LOOK_BYTES
}
#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_prepare_with_history_look(
    basis_len: usize,
    frame_len: usize,
    look_len: usize,
) -> i32 {
    OUTPUT.with(|output| output.borrow_mut().clear());
    if look_len == 0 || look_len > conduit_face::MAX_LOOK_BYTES {
        LOOK_INPUT.with(|input| input.borrow_mut().fill(0));
        return -1;
    }
    let look = LOOK_INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let value = conduit_face::LookInfo::decode(&input[..look_len]);
        input.fill(0);
        value
    });
    let Ok(look) = look else {
        return -2;
    };
    prepare_with_history_look(basis_len, frame_len, Some(&look))
}
