//! getrandom backend for Monty's wasm target.

use std::sync::atomic::{AtomicU64, Ordering};

// getrandom 0.3+ requires an explicit backend for wasm32-unknown-unknown.
// ahash (via monty/jiter) uses it for hash randomisation, not crypto.
static SEED_CTR: AtomicU64 = AtomicU64::new(0x5EED_C0DE_CAFE_F00D);

#[unsafe(no_mangle)]
unsafe extern "C" fn __getrandom_v03_custom(dest: *mut u8, len: usize) -> u32 {
    let buf = unsafe { core::slice::from_raw_parts_mut(dest, len) };
    let mut state = SEED_CTR.fetch_add(1, Ordering::Relaxed);
    for chunk in buf.chunks_mut(8) {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let bytes = state.to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    0
}
