//! Bounded canonical and quoted-content encoding shared by recall paths.
use super::{View, MAX, R};
use alloc::vec::Vec;
// The projector already rejects unselected evidence. Keep the retrieval policy
// explicitly tied to selection as well, rather than treating storage as consent.
pub(super) fn explicitly_selected(request: View<'_>, item: View<'_>) -> Result<bool, R> {
    let selected = field(field(request, "selection")?, "item_identities")?;
    let identity = field(item, "identity")?.value_node();
    for index in 0..8 {
        let Some(candidate) = element(selected, index)? else {
            break;
        };
        if candidate.value_node() == identity {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn field<'a>(v: View<'a>, key: &str) -> Result<View<'a>, R> {
    v.record_field(key)
        .map_err(|_| R::Malformed)?
        .ok_or(R::Malformed)
}
pub(super) fn element(v: View<'_>, index: u16) -> Result<Option<View<'_>>, R> {
    v.collection_index(index).map_err(|_| R::Malformed)
}
pub(super) fn leaf(v: View<'_>) -> Result<&[u8], R> {
    let n = v.value_node();
    if n.len() < 5 || n[0] != 0 {
        return Err(R::Malformed);
    }
    if u32::from_le_bytes(n[1..5].try_into().unwrap()) as usize != n.len() - 5 {
        return Err(R::Malformed);
    }
    Ok(&n[5..])
}
pub(super) fn number(v: View<'_>) -> Result<u64, R> {
    Ok(u64::from_le_bytes(
        leaf(v)?.try_into().map_err(|_| R::Malformed)?,
    ))
}
pub(super) fn put(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    if bytes.len() > MAX - out.len() {
        return Err(R::ByteBound);
    }
    out.extend_from_slice(bytes);
    Ok(())
}
pub(super) fn name(out: &mut Vec<u8>, key: &str) -> Result<(), R> {
    put(out, &(key.len() as u32).to_le_bytes())?;
    put(out, key.as_bytes())
}
pub(super) fn scalar(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    put(out, &[0])?;
    put(out, &(bytes.len() as u32).to_le_bytes())?;
    put(out, bytes)
}
pub(super) fn record(out: &mut Vec<u8>, n: u32) -> Result<(), R> {
    put(out, &[2])?;
    put(out, &n.to_le_bytes())
}
pub(super) fn sequence(out: &mut Vec<u8>, n: usize) -> Result<(), R> {
    put(out, &[1])?;
    put(out, &(n as u32).to_le_bytes())
}
pub(super) fn unit_variant(out: &mut Vec<u8>, tag: &str) -> Result<(), R> {
    put(out, &[3])?;
    name(out, tag)?;
    scalar(out, &[])
}
pub(super) fn tag<'a>(v: View<'_>, tags: &'a [&str]) -> Result<&'a str, R> {
    for tag in tags {
        if v.variant_payload(tag).map_err(|_| R::Malformed)?.is_some() {
            return Ok(tag);
        }
    }
    Err(R::Malformed)
}
pub(super) fn decimal(out: &mut Vec<u8>, mut n: u64) -> Result<(), R> {
    let mut b = [0; 20];
    let mut start = 20;
    loop {
        start -= 1;
        b[start] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    put(out, &b[start..])
}
pub(super) fn hex(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    const H: &[u8] = b"0123456789abcdef";
    for b in bytes {
        put(out, &[H[(b >> 4) as usize], H[(b & 15) as usize]])?;
    }
    Ok(())
}
pub(super) fn quoted(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), R> {
    core::str::from_utf8(bytes).map_err(|_| R::Malformed)?;
    put(out, b"\"")?;
    for b in bytes {
        match b {
            b'"' => put(out, b"\\\"")?,
            b'\\' => put(out, b"\\\\")?,
            0..=31 => {
                put(out, b"\\u00")?;
                hex(out, &[*b])?;
            }
            _ => put(out, &[*b])?,
        }
    }
    put(out, b"\"")
}
