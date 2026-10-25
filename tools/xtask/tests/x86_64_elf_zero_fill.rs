//! Exercise the actual native linker script; no firmware/boot success is implied.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::{fs, path::PathBuf, process::Command};

struct Directory(PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn output(command: &mut Command) -> Vec<u8> {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}
#[test]
fn got_is_file_backed_before_the_zero_fill_tail_and_within_image_bounds() {
    let directory = Directory(
        std::env::temp_dir().join(format!(
            "conduit-elf-layout-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )),
    );
    fs::create_dir(&directory.0).unwrap();
    let assembly = directory.0.join("layout.S");
    let object = directory.0.join("layout.o");
    let elf = directory.0.join("layout.elf");
    fs::write(&assembly, b".section .text,\"ax\"\n.global conduitos_start\nconduitos_start: ret\n.section .data,\"aw\"\n.quad 1\n.section .got,\"aw\"\n.global got_marker\ngot_marker: .quad 2\n.section .bss,\"aw\",@nobits\n.balign 4096\n.skip 67108864\n").unwrap();
    output(
        Command::new("cc")
            .arg("-c")
            .arg(&assembly)
            .arg("-o")
            .arg(&object),
    );
    let sysroot =
        String::from_utf8(output(Command::new("rustc").args(["--print", "sysroot"]))).unwrap();
    let linker =
        PathBuf::from(sysroot.trim()).join("lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld");
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../targets/conduitos/firmware/linker/x86_64.ld");
    output(
        Command::new(linker)
            .args(["-flavor", "gnu", "-z", "max-page-size=4096", "-T"])
            .arg(script)
            .arg(&object)
            .arg("-o")
            .arg(&elf),
    );
    assert!(
        fs::metadata(&elf).unwrap().len() < 64 * 1024,
        "zero-fill storage must not become ELF file bytes"
    );
    let headers = String::from_utf8(output(Command::new("readelf").arg("-lW").arg(&elf))).unwrap();
    let data = headers
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .find(|fields| fields.first() == Some(&"LOAD") && fields.get(6) == Some(&"RW"))
        .unwrap();
    let hex = |text: &str| u64::from_str_radix(text.trim_start_matches("0x"), 16).unwrap();
    assert!(
        hex(data[5]) >= 64 * 1024 * 1024,
        "zero-fill memory must remain reserved"
    );
    assert!(
        hex(data[4]) < 64 * 1024,
        "writable file backing must remain small"
    );
    let symbols = String::from_utf8(output(Command::new("nm").arg("-n").arg(&elf))).unwrap();
    let symbol = |name| {
        symbols
            .lines()
            .find_map(|line| {
                let fields = line.split_whitespace().collect::<Vec<_>>();
                (fields.len() == 3 && fields[2] == name).then(|| hex(fields[0]))
            })
            .unwrap()
    };
    assert!(
        symbol("__conduitos_image_end") >= symbol("got_marker") + 8,
        "image ownership must include the file-backed GOT"
    );
}
