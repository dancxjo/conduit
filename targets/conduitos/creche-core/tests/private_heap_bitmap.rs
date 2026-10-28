//! Execute the production private service allocator's pure bitmap algorithm.
#[path = "../../shared/private_heap_bitmap.rs"]
mod bitmap;
use bitmap::*;
#[path = "../../creche-service/allocator/bitmap_tests.rs"]
mod tests;
