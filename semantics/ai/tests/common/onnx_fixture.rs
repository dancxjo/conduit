// Synthetic metadata uses the pinned ONNX protobuf field numbers. No downloaded
// weights, provider library or network access participates in these tests.
fn varint(mut n: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (n & 127) as u8;
        n >>= 7;
        out.push(byte | if n == 0 { 0 } else { 128 });
        if n == 0 {
            return out;
        }
    }
}
pub fn bytes(field: u64, value: &[u8]) -> Vec<u8> {
    [
        varint(field * 8 + 2),
        varint(value.len() as u64),
        value.to_vec(),
    ]
    .concat()
}
pub fn number(field: u64, value: u64) -> Vec<u8> {
    [varint(field * 8), varint(value)].concat()
}
pub fn port(name: &str, dtype: u64, dimension: Vec<u8>) -> Vec<u8> {
    let shape = bytes(1, &dimension);
    let tensor = [number(1, dtype), bytes(2, &shape)].concat();
    [bytes(1, name.as_bytes()), bytes(2, &bytes(1, &tensor))].concat()
}
// Symbol-only consumers use the shared wire builders without this fixture.
#[allow(dead_code)]
pub fn model(extra_graph: &[u8]) -> Vec<u8> {
    let graph = [
        bytes(2, b"tiny-mixed-input"),
        bytes(11, &port("tokens", 7, bytes(2, b"sequence"))),
        bytes(11, &port("scale", 1, number(1, 1))),
        bytes(12, &port("result", 1, number(1, 4))),
        extra_graph.to_vec(),
    ]
    .concat();
    [number(1, 9), bytes(8, &number(2, 18)), bytes(7, &graph)].concat()
}
