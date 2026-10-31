//! Hostname/MAC correlation data only; a spelling never authenticates a Root.
pub(crate) fn parse(bytes: &[u8]) -> Result<crate::protocol::bench_mdns::Query<'_>, &'static str> {
    let prefix = b"conduit-climate-";
    if bytes.get(8..8 + prefix.len()) != Some(prefix.as_slice()) {
        return Err("mdns-query-host");
    }
    let encoded = bytes
        .get(8 + prefix.len()..8 + prefix.len() + 12)
        .ok_or("mdns-query-host")?;
    let mut mac = [0; 6];
    fn digit(b: u8) -> Result<u8, &'static str> {
        match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err("mdns-query-host"),
        }
    }
    for (out, pair) in mac.iter_mut().zip(encoded.chunks_exact(2)) {
        *out = digit(pair[0])? << 4 | digit(pair[1])?;
    }
    crate::protocol::bench_mdns::Query::parse(bytes, mac)
}
