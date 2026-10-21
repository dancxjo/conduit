//! Run with rustc from this directory after the independent target build.
//! Uses the real Root admission parser, without linking Root into the guest.
#[path = "../src/domain_image.rs"]
mod domain_image;
#[path = "../src/service_memory.rs"]
mod service_memory;
fn main() {
    let path = std::env::args_os().nth(1).expect("compositor ELF path");
    let bytes = std::fs::read(path).expect("read independently linked guest");
    let admitted =
        service_memory::ServiceImage::compositor(&bytes).expect("immutable compositor admission");
    assert_eq!(admitted.image().entry, 0x400000);
    assert_eq!(admitted.layout().exchange().start(), 0x601000);
    assert_eq!(admitted.layout().output().start(), 0x603000);
    assert_eq!(admitted.layout().metadata().start(), 0x625000);
    assert_eq!(admitted.layout().pixels().start(), 0x666000);
    assert_eq!(admitted.layout().scratch().start(), 0xe67000);
    println!(
        "immutable compositor ELF admitted; entry={:#x}, image_bytes={}, private_grants={}",
        admitted.image().entry,
        bytes.len(),
        admitted.layout().mapped_bytes()
    );
}
