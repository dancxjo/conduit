//! Selected carrier endpoint only; no discovery, authentication or Body claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Peer {
    pub address: [u8; 4],
    pub generation: u64,
    local: [u8; 4],
    prefix: u8,
}
fn number(ip: [u8; 4]) -> u32 {
    u32::from_be_bytes(ip)
}
pub(crate) fn private(ip: [u8; 4]) -> bool {
    ip[0] == 10 || (ip[0] == 172 && (16..=31).contains(&ip[1])) || (ip[0] == 192 && ip[1] == 168)
}
impl Peer {
    pub fn select(
        address: [u8; 4],
        local: [u8; 4],
        prefix: u8,
        generation: u64,
    ) -> Result<Self, &'static str> {
        if !private(address) || !private(local) || !(1..=30).contains(&prefix) || generation == 0 {
            return Err("peer-not-private-unicast");
        }
        let mask = u32::MAX << (32 - prefix);
        let network = number(local) & mask;
        let target = number(address);
        if target == number(local)
            || target & mask != network
            || target == network
            || target == network | !mask
        {
            return Err("peer-not-on-link-unicast");
        }
        Ok(Self {
            address,
            generation,
            local,
            prefix,
        })
    }
    pub fn matches(&self, local: [u8; 4], prefix: u8, generation: u64) -> bool {
        self.local == local && self.prefix == prefix && self.generation == generation
    }
}
#[derive(Clone, Copy, Default)]
pub(crate) struct Attachment {
    pub generation: u64,
    current: Option<([u8; 4], u8)>,
    pub peer: Option<Peer>,
}
impl Attachment {
    pub fn observe(&mut self, current: Option<([u8; 4], u8)>) -> Result<(), &'static str> {
        if self.current != current {
            self.invalidate()?;
            self.current = current;
        }
        Ok(())
    }
    pub fn invalidate(&mut self) -> Result<(), &'static str> {
        self.peer = None;
        self.current = None;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("attachment-generation-exhausted")?;
        Ok(())
    }
    pub fn select(&mut self, address: [u8; 4]) -> Result<(), &'static str> {
        let (local, prefix) = self.current.ok_or("wifi-unavailable")?;
        self.peer = Some(Peer::select(address, local, prefix, self.generation)?);
        Ok(())
    }
    pub fn ready(&self) -> bool {
        self.current.is_some_and(|(ip, prefix)| {
            self.peer
                .is_some_and(|p| p.matches(ip, prefix, self.generation))
        })
    }
}
