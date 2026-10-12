#[path = "verify_storage.rs"]
mod storage;
fn main() {
    let arguments: Vec<_> = std::env::args().collect();
    assert_eq!(
        arguments.len(),
        4,
        "serial.log mode expected-block-or-terminal-tag"
    );
    let log = std::fs::read_to_string(&arguments[1]).unwrap();
    match arguments[2].as_str() {
        "native-storage-reuse" => {
            storage::check_many(&log, &std::fs::read(&arguments[3]).unwrap(), 16)
        }
        "native-storage-one-read" => storage::check(&log, &std::fs::read(&arguments[3]).unwrap()),
        "native-storage-terminal" => storage::check_terminal(&log, &arguments[3]),
        _ => panic!("unsupported proof mode"),
    }
}
