//! Bounded IPv4 hostname mDNS profile (RFC 6762); no Body/Host admission.
//! DNS-SD service publication and IPv6 address records are outside this profile.
pub const MAXIMUM_PACKET: usize = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Packet,
    Name,
    Capacity,
}
pub struct Name {
    wire: [u8; 80],
    length: usize,
}
impl Name {
    pub fn new(label: &str) -> Result<Self, Refusal> {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(Refusal::Name);
        }
        let mut wire = [0; 80];
        wire[0] = label.len() as u8;
        for (index, byte) in label.bytes().enumerate() {
            wire[index + 1] = byte.to_ascii_lowercase();
        }
        let at = 1 + label.len();
        wire[at..at + 7].copy_from_slice(b"\x05local\0");
        Ok(Self {
            wire,
            length: at + 7,
        })
    }
    fn bytes(&self) -> &[u8] {
        &self.wire[..self.length]
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Interest {
    pub question: bool,
    pub unicast: bool,
    pub suppress: bool,
    pub conflict: bool,
    pub truncated: bool,
    pub question_end: usize,
    pub question_count: u16,
    pub id: u16,
}
fn number(packet: &[u8], at: usize) -> Result<u16, Refusal> {
    Ok(u16::from_be_bytes(
        packet
            .get(at..at + 2)
            .ok_or(Refusal::Packet)?
            .try_into()
            .unwrap(),
    ))
}
fn name(packet: &[u8], start: usize, expected: &Name) -> Result<(usize, bool), Refusal> {
    let mut cursor = start;
    let mut end = None;
    let mut matched = true;
    let mut expanded = 0;
    let mut hops = 0;
    for _ in 0..48 {
        let size = *packet.get(cursor).ok_or(Refusal::Packet)?;
        if size & 0xc0 == 0xc0 {
            let next = *packet.get(cursor + 1).ok_or(Refusal::Packet)?;
            let target = (usize::from(size & 63) << 8) | usize::from(next);
            if target >= cursor || hops >= 16 {
                return Err(Refusal::Packet);
            }
            end.get_or_insert(cursor + 2);
            cursor = target;
            hops += 1;
            continue;
        }
        if size > 63 {
            return Err(Refusal::Packet);
        }
        let length = usize::from(size);
        let bytes = packet
            .get(cursor..cursor + 1 + length)
            .ok_or(Refusal::Packet)?;
        if expanded + bytes.len() > 255 {
            return Err(Refusal::Packet);
        }
        matched &= expected
            .bytes()
            .get(expanded..expanded + bytes.len())
            .is_some_and(|known| {
                known
                    .iter()
                    .zip(bytes)
                    .all(|(a, b)| *a == b.to_ascii_lowercase())
            });
        expanded += bytes.len();
        cursor += bytes.len();
        if length == 0 {
            return Ok((
                end.unwrap_or(cursor),
                matched && expanded == expected.length,
            ));
        }
    }
    Err(Refusal::Packet)
}
pub fn inspect(packet: &[u8], expected: &Name, address: [u8; 4]) -> Result<Interest, Refusal> {
    if packet.len() < 12 || packet.len() > MAXIMUM_PACKET {
        return Err(Refusal::Packet);
    }
    let flags = number(packet, 2)?;
    if flags & 0x780f != 0 {
        return Err(Refusal::Packet);
    }
    let questions = number(packet, 4)?;
    let answers = number(packet, 6)?;
    let authority = number(packet, 8)?;
    let additional = number(packet, 10)?;
    if questions > 8 || u32::from(answers) + u32::from(authority) + u32::from(additional) > 16 {
        return Err(Refusal::Packet);
    }
    let response = flags & 0x8000 != 0;
    let mut interest = Interest {
        id: number(packet, 0)?,
        question_count: questions,
        truncated: flags & 0x0200 != 0,
        ..Interest::default()
    };
    let mut cursor = 12;
    for _ in 0..questions {
        let (next, matched) = name(packet, cursor, expected)?;
        let kind = number(packet, next)?;
        let class = number(packet, next + 2)?;
        if !response && matched && matches!(kind, 1 | 255) && class & 0x7fff == 1 {
            interest.question = true;
            interest.unicast |= class & 0x8000 != 0;
        }
        cursor = next + 4;
    }
    interest.question_end = cursor;
    for index in 0..u32::from(answers) + u32::from(authority) + u32::from(additional) {
        let (next, matched) = name(packet, cursor, expected)?;
        let kind = number(packet, next)?;
        let class = number(packet, next + 2)?;
        let ttl = u32::from_be_bytes(
            packet
                .get(next + 4..next + 8)
                .ok_or(Refusal::Packet)?
                .try_into()
                .unwrap(),
        );
        let length = usize::from(number(packet, next + 8)?);
        let data = packet
            .get(next + 10..next + 10 + length)
            .ok_or(Refusal::Packet)?;
        if matched && kind == 1 && class & 0x7fff == 1 && length == 4 {
            if !response && index < u32::from(answers) && data == address && ttl >= 60 {
                interest.suppress = true;
            }
            let proposed = !response
                && index >= u32::from(answers)
                && index < u32::from(answers) + u32::from(authority);
            // Simultaneous unique A probes compare network-order RDATA. A responder
            // with the lexicographically later address wins the tie, as RFC 6762 8.2.
            if ttl > 0 && data != address && (response || (proposed && data > address.as_slice())) {
                interest.conflict = true;
            }
        }
        cursor = next + 10 + length;
    }
    if cursor != packet.len() {
        return Err(Refusal::Packet);
    }
    Ok(interest)
}
fn put(output: &mut [u8], at: &mut usize, bytes: &[u8]) -> Result<(), Refusal> {
    let end = at.checked_add(bytes.len()).ok_or(Refusal::Capacity)?;
    output
        .get_mut(*at..end)
        .ok_or(Refusal::Capacity)?
        .copy_from_slice(bytes);
    *at = end;
    Ok(())
}
fn question(
    output: &mut [u8],
    at: &mut usize,
    name: &Name,
    kind: u16,
    class: u16,
) -> Result<(), Refusal> {
    put(output, at, name.bytes())?;
    put(output, at, &kind.to_be_bytes())?;
    put(output, at, &class.to_be_bytes())
}
fn record(
    output: &mut [u8],
    at: &mut usize,
    name: &Name,
    address: [u8; 4],
    class: u16,
    ttl: u32,
) -> Result<(), Refusal> {
    question(output, at, name, 1, class)?;
    put(output, at, &ttl.to_be_bytes())?;
    put(output, at, &4u16.to_be_bytes())?;
    put(output, at, &address)
}
pub fn probe(name: &Name, address: [u8; 4], output: &mut [u8]) -> Result<usize, Refusal> {
    if output.len() < 12 {
        return Err(Refusal::Capacity);
    }
    output[..12].fill(0);
    output[5] = 1;
    output[9] = 1;
    let mut at = 12;
    question(output, &mut at, name, 255, 0x8001)?;
    record(output, &mut at, name, address, 1, 120)?;
    Ok(at)
}
pub fn answer(
    name: &Name,
    address: [u8; 4],
    legacy: Option<(&[u8], Interest)>,
    output: &mut [u8],
) -> Result<usize, Refusal> {
    if output.len() < 12 {
        return Err(Refusal::Capacity);
    }
    output[..12].fill(0);
    output[2] = 0x84;
    output[7] = 1;
    let mut at = 12;
    if let Some((query, interest)) = legacy {
        output[..2].copy_from_slice(&interest.id.to_be_bytes());
        output[4..6].copy_from_slice(&interest.question_count.to_be_bytes());
        put(
            output,
            &mut at,
            query
                .get(12..interest.question_end)
                .ok_or(Refusal::Packet)?,
        )?;
    }
    record(
        output,
        &mut at,
        name,
        address,
        if legacy.is_some() { 1 } else { 0x8001 },
        if legacy.is_some() { 10 } else { 120 },
    )?;
    Ok(at)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn query(name: &Name, kind: u16, class: u16) -> Vec<u8> {
        let mut out = vec![0; 512];
        out[5] = 1;
        let mut at = 12;
        question(&mut out, &mut at, name, kind, class).unwrap();
        out.truncate(at);
        out
    }
    #[test]
    fn probes_announcements_legacy_and_known_answer_suppression() {
        let name = Name::new("conduit-climate-001122334455").unwrap();
        let address = [192, 168, 1, 9];
        let mut out = [0; 512];
        let n = probe(&name, address, &mut out).unwrap();
        assert!(inspect(&out[..n], &name, address).unwrap().question);
        let n = answer(&name, address, None, &mut out).unwrap();
        assert!(!inspect(&out[..n], &name, address).unwrap().conflict);
        assert!(
            inspect(&out[..n], &name, [192, 168, 1, 8])
                .unwrap()
                .conflict
        );
        let mut q = query(&name, 1, 1);
        q[..2].copy_from_slice(&0x1234u16.to_be_bytes());
        let interest = inspect(&q, &name, address).unwrap();
        let n = answer(&name, address, Some((&q, interest)), &mut out).unwrap();
        assert_eq!(&out[..2], &[0x12, 0x34]);
        assert_eq!(&out[12..interest.question_end], &q[12..]);
        assert_eq!(
            inspect(&out[..n], &name, address).unwrap().question_count,
            1
        );
        q[7] = 1;
        let mut at = q.len();
        q.resize(512, 0);
        record(&mut q, &mut at, &name, address, 1, 120).unwrap();
        q.truncate(at);
        assert!(inspect(&q, &name, address).unwrap().suppress);
    }
    #[test]
    fn compressed_questions_tie_breaking_and_malformed_packets_are_bounded() {
        let name = Name::new("device").unwrap();
        let mut q = query(&name, 1, 1);
        q[5] = 2;
        q.extend_from_slice(&[0xc0, 12, 0, 255, 0x80, 1]);
        assert!(inspect(&q, &name, [1, 2, 3, 4]).unwrap().question);
        assert!(inspect(&q, &name, [1, 2, 3, 4]).unwrap().unicast);
        let mut out = [0; 512];
        let n = probe(&name, [1, 2, 3, 5], &mut out).unwrap();
        assert!(inspect(&out[..n], &name, [1, 2, 3, 4]).unwrap().conflict);
        assert!(!inspect(&out[..n], &name, [1, 2, 3, 6]).unwrap().conflict);
        for length in 0..q.len() {
            let _ = inspect(&q[..length], &name, [1, 2, 3, 4]);
        }
        let mut cycle = [0; 18];
        cycle[5] = 1;
        cycle[12] = 0xc0;
        cycle[13] = 12;
        assert_eq!(inspect(&cycle, &name, [1, 2, 3, 4]), Err(Refusal::Packet));
        assert!(Name::new("bad.local").is_err());
        assert!(answer(&name, [1, 2, 3, 4], None, &mut [0; 12]).is_err());
    }
}
