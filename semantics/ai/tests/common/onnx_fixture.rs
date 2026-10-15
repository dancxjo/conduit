// A local Add graph uses the pinned ONNX protobuf field numbers and four inline
// bias values. No downloaded weights or provider library participates here.
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
    model_with_versions(9, 18, extra_graph)
}
pub fn model_with_versions(ir: u64, opset: u64, extra_graph: &[u8]) -> Vec<u8> {
    let bias = [1_f32, 2., 3., 4.]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    let initializer = [
        number(1, 4),
        number(2, 1),
        bytes(8, b"bias"),
        bytes(9, &bias),
    ]
    .concat();
    let node = [
        bytes(1, b"scale"),
        bytes(1, b"bias"),
        bytes(2, b"result"),
        bytes(4, b"Add"),
    ]
    .concat();
    let graph = [
        bytes(1, &node),
        bytes(2, b"tiny-mixed-input"),
        bytes(5, &initializer),
        bytes(11, &port("tokens", 7, bytes(2, b"sequence"))),
        bytes(11, &port("scale", 1, number(1, 1))),
        bytes(12, &port("result", 1, number(1, 4))),
        extra_graph.to_vec(),
    ]
    .concat();
    [number(1, ir), bytes(8, &number(2, opset)), bytes(7, &graph)].concat()
}
