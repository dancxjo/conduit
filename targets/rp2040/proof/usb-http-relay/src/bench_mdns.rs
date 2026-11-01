//! Bounded legacy-unicast mDNS observation for the explicit bench only.
//! DNS/USB correlation is not cryptographic authentication or Conduit authority.
pub const MAX_HOST: usize = 48;
pub const MAX_PACKET: usize = 512;
pub struct Query<'a> {
    pub nonce: [u8; 8],
    pub host: &'a str,
}
impl<'a> Query<'a> {
    pub fn parse(bytes: &'a [u8], mac: [u8; 6]) -> Result<Self, &'static str> {
        if !(9..=8 + MAX_HOST).contains(&bytes.len()) {
            return Err("mdns-query-capacity");
        }
        let nonce: [u8; 8] = bytes[..8].try_into().unwrap();
        if nonce == [0; 8] {
            return Err("mdns-query-nonce");
        }
        let host = core::str::from_utf8(&bytes[8..]).map_err(|_| "mdns-query-host")?;
        let base = b"conduit-climate-";
        if !host.as_bytes().starts_with(base) {
            return Err("mdns-query-host");
        }
        let hex = b"0123456789abcdef";
        for (index, byte) in mac.iter().enumerate() {
            if host.as_bytes().get(base.len() + index * 2) != Some(&hex[(byte >> 4) as usize])
                || host.as_bytes().get(base.len() + index * 2 + 1)
                    != Some(&hex[(byte & 15) as usize])
            {
                return Err("mdns-query-peer");
            }
        }
        let suffix = host
            .as_bytes()
            .get(base.len() + 12..)
            .ok_or("mdns-query-host")?;
        if suffix != b".local" && suffix != b"-2.local" && suffix != b"-3.local" {
            return Err("mdns-query-host");
        }
        Ok(Self { nonce, host })
    }
    pub fn id(&self) -> u16 {
        u16::from_be_bytes([self.nonce[0], self.nonce[1]]).max(1)
    }
    pub fn packet(&self, out: &mut [u8; MAX_PACKET]) -> usize {
        out.fill(0);
        out[..2].copy_from_slice(&self.id().to_be_bytes());
        out[5] = 1;
        let mut cursor = 12;
        for part in self.host.split('.') {
            out[cursor] = part.len() as u8;
            cursor += 1;
            out[cursor..cursor + part.len()].copy_from_slice(part.as_bytes());
            cursor += part.len();
        }
        cursor += 1;
        out[cursor..cursor + 4].copy_from_slice(&[0, 1, 0, 1]);
        cursor + 4
    }
    pub fn answer(
        &self,
        packet: &[u8],
        source: [u8; 4],
        port: u16,
        expected: [u8; 4],
    ) -> Result<(), &'static str> {
        if source != expected || port != 5353 || packet.len() > MAX_PACKET || packet.len() < 12 {
            return Err("mdns-answer-source");
        }
        if word(packet, 0)? != self.id() || word(packet, 2)? != 0x8400 || word(packet, 4)? != 1 {
            return Err("mdns-answer-header");
        }
        let counts = [word(packet, 6)?, word(packet, 8)?, word(packet, 10)?];
        if counts[0] == 0 || counts.iter().map(|n| usize::from(*n)).sum::<usize>() > 32 {
            return Err("mdns-answer-count");
        }
        let mut cursor = 12;
        if !name(packet, &mut cursor, self.host)? {
            return Err("mdns-answer-question");
        }
        if word(packet, cursor)? != 1 || word(packet, cursor + 2)? != 1 {
            return Err("mdns-answer-question");
        }
        cursor += 4;
        let mut found = false;
        for (section, count) in counts.into_iter().enumerate() {
            for _ in 0..count {
                // An unrelated name cannot authenticate the expected host.
                let matching = name(packet, &mut cursor, self.host)?;
                let kind = word(packet, cursor)?;
                let class = word(packet, cursor + 2)?;
                let ttl = u32::from_be_bytes(
                    packet
                        .get(cursor + 4..cursor + 8)
                        .ok_or("mdns-answer-truncated")?
                        .try_into()
                        .unwrap(),
                );
                let length = usize::from(word(packet, cursor + 8)?);
                cursor += 10;
                let data = packet
                    .get(cursor..cursor + length)
                    .ok_or("mdns-answer-truncated")?;
                cursor += length;
                if section == 0 && matching && kind == 1 && class & 0x7fff == 1 {
                    if ttl == 0 || data != expected {
                        return Err("mdns-answer-address");
                    }
                    found = true;
                }
            }
        }
        if cursor != packet.len() || !found {
            return Err("mdns-answer-address");
        }
        Ok(())
    }
}
fn word(packet: &[u8], at: usize) -> Result<u16, &'static str> {
    Ok(u16::from_be_bytes(
        packet
            .get(at..at + 2)
            .ok_or("mdns-answer-truncated")?
            .try_into()
            .unwrap(),
    ))
}
fn name(packet: &[u8], cursor: &mut usize, expected: &str) -> Result<bool, &'static str> {
    let mut at = *cursor;
    let mut jumped = false;
    let mut hops = 0;
    let mut offset = 0;
    let mut matching = true;
    loop {
        hops += 1;
        if hops > 32 {
            return Err("mdns-answer-name");
        }
        let length = *packet.get(at).ok_or("mdns-answer-truncated")?;
        at += 1;
        if length & 0xc0 == 0xc0 {
            let second = *packet.get(at).ok_or("mdns-answer-truncated")?;
            if !jumped {
                *cursor = at + 1;
                jumped = true;
            }
            let target = (usize::from(length & 63) << 8) | usize::from(second);
            if target >= at - 1 {
                return Err("mdns-answer-pointer");
            }
            at = target;
            continue;
        }
        if length & 0xc0 != 0 {
            return Err("mdns-answer-name");
        }
        if length == 0 {
            if !jumped {
                *cursor = at;
            }
            break;
        }
        let label = packet
            .get(at..at + usize::from(length))
            .ok_or("mdns-answer-truncated")?;
        if offset > 0 {
            matching &= expected.as_bytes().get(offset) == Some(&b'.');
            offset += 1;
        }
        matching &= expected.as_bytes().get(offset..offset + label.len()) == Some(label);
        offset += label.len();
        at += label.len();
    }
    Ok(matching && offset == expected.len())
}
