//! Executes the production bitmap allocator in the exact fixed private grant.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "private_heap/mod.rs"]
mod grant_proof;
fn main() {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    grant_proof::run();
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    println!("SKIP exact fixed-grant regression requires Linux x86_64 mmap");
}
