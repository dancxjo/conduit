//! Run the unchanged production bitmap allocator inside its exact Linux grant.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "collection_private_heap/proof.rs"]
mod proof;
fn main() {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    proof::run();
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    println!("SKIP exact fixed-grant proof requires Linux x86_64 mmap");
}
