//! Exhaustive non-browser proof across admission, staging and real HTTP delivery.
use patchbay_html::{demonstration_snapshot, PatchbayHtmlServer};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;

fn get(address: SocketAddr, path: &str) -> (String, Vec<u8>) {
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    write!(stream, "GET /{path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    let split = bytes
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .unwrap();
    (
        String::from_utf8(bytes[..split].to_vec()).unwrap(),
        bytes[split + 4..].to_vec(),
    )
}

#[test]
fn every_admitted_resource_has_exact_staged_and_http_bytes_and_media_type() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let temp = std::env::temp_dir().join(format!("patchbay-registry-{}", std::process::id()));
    std::fs::create_dir(&temp).unwrap();
    let runtime = b"\0asm\x01\0\0\0";
    std::fs::write(temp.join("runtime.wasm"), runtime).unwrap();
    let destination = temp.join("product");
    let status = std::process::Command::new("sh")
        .arg(root.join("products/patchbay/tools/stage-patchbay-product.sh"))
        .arg(temp.join("runtime.wasm"))
        .arg(env!("CARGO_BIN_EXE_patchbay-static-assets"))
        .arg(&destination)
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(status.success());
    let template: Value = serde_json::from_slice(include_bytes!(
        "../assets/patchbay.application.template.json"
    ))
    .unwrap();
    let resources = template["resources"].as_array().unwrap();
    let server = PatchbayHtmlServer::bind_ephemeral(&demonstration_snapshot().unwrap()).unwrap();
    let address = server.local_addr().unwrap();
    let count = resources.len() + 2;
    let worker = std::thread::spawn(move || server.serve_count(count));
    let (headers, manifest) = get(address, "patchbay.application.json");
    assert!(headers.starts_with("HTTP/1.1 200 OK"));
    assert_eq!(
        manifest,
        std::fs::read(destination.join("patchbay.application.json")).unwrap()
    );
    for entry in resources {
        let path = entry["path"].as_str().unwrap();
        let (headers, bytes) = get(address, path);
        assert!(headers.starts_with("HTTP/1.1 200 OK"), "{path}: {headers}");
        let media = match entry["kind"].as_str().unwrap() {
            "module" | "classic-script" => "text/javascript; charset=utf-8",
            "style" => "text/css; charset=utf-8",
            "wasm" => "application/wasm",
            other => panic!("unknown kind: {other}"),
        };
        assert!(
            headers.contains(&format!("Content-Type: {media}\r\n")),
            "{path}: {headers}"
        );
        assert_eq!(
            bytes,
            std::fs::read(destination.join(path)).unwrap(),
            "{path}"
        );
        let expected = match entry["source"].as_str().unwrap() {
            "generated:theme" => patchbay_html::application_theme_css(),
            "supplied:runtime" => runtime.to_vec(),
            source => std::fs::read(root.join(source)).unwrap(),
        };
        assert_eq!(bytes, expected, "{path}");
        assert!(
            !bytes.is_empty() && bytes.len() <= entry["maximum_bytes"].as_u64().unwrap() as usize
        );
    }
    assert!(get(address, "assets/unadmitted.mjs")
        .0
        .starts_with("HTTP/1.1 404"));
    worker.join().unwrap().unwrap();
    std::fs::remove_dir_all(temp).unwrap();
}

#[test]
fn invalid_dynamic_bytes_refuse_before_staging_resources() {
    let destination =
        std::env::temp_dir().join(format!("patchbay-registry-invalid-{}", std::process::id()));
    for runtime in [Vec::new(), vec![0; 5242881]] {
        assert!(patchbay_html::application_resources::stage(&destination, &runtime).is_err());
        assert!(!destination.exists());
    }
}
