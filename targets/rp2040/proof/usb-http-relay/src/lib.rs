//! Bounded transport framing only. No Body, native capability or HTTP authority
//! is established here. Bench AP mode is an explicit separate USB command.
#![no_std]
pub const ROLE: &str = "conduit.pico-usb-http-relay@1";
pub const BUILD_ID: &str = env!("CONDUIT_RELAY_BUILD_ID");
pub const BOOTSEL_QUERY: u8 = 4;
pub const BOOTSEL_REQUEST: u8 = 5;
pub const BOOTSEL_CHALLENGE: u8 = 132;
pub const BOOTSEL_ACK: u8 = 133;
pub fn bootsel_matches(payload: &[u8]) -> bool {
    payload == BUILD_ID.as_bytes()
}
pub use conduit_device_carrier_protocol::{wipe, Frame, Scratch, HEADER, MAX_REQUEST};
pub const MAX_RESPONSE: usize = 65536;
pub const CHUNK: usize = 512;
pub const JOIN: u8 = 1;
pub const REQUEST: u8 = 2;
pub const STATUS: u8 = 3;
pub const SENSOR_STATUS: u8 = 6;
pub const SENSOR_DIAGNOSTIC: u8 = 7;
pub const TEST_AP_START: u8 = 8;
pub const TEST_AP_STOP: u8 = 9;
pub const MDNS_QUERY: u8 = 10;
/// Explicit opt-in fixed-port IPv4 carrier peer selection.
pub const SELECT_PEER: u8 = 11;
pub mod bench_mdns;
pub const DATA: u8 = 128;
pub const DONE: u8 = 129;
pub const ERROR: u8 = 130;
pub const INFORMATION: u8 = 131;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Header,
    Version,
    Kind,
    Capacity,
    Pipeline,
    Incomplete,
    Stale,
    Credentials,
    Peer,
}

impl From<conduit_device_carrier_protocol::Refusal> for Refusal {
    fn from(value: conduit_device_carrier_protocol::Refusal) -> Self {
        use conduit_device_carrier_protocol::Refusal as R;
        match value {
            R::Header => Self::Header,
            R::Version => Self::Version,
            R::Kind => Self::Kind,
            R::Capacity => Self::Capacity,
            R::Pipeline => Self::Pipeline,
            R::Incomplete => Self::Incomplete,
            R::Stale => Self::Stale,
        }
    }
}
pub struct Decoder {
    frame: conduit_device_carrier_protocol::FrameDecoder<{ HEADER + MAX_REQUEST }>,
    join_limit: usize,
    selected_peer: bool,
}
impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}
impl Decoder {
    pub const fn new() -> Self {
        Self {
            frame: conduit_device_carrier_protocol::FrameDecoder::new(),
            join_limit: 97,
            selected_peer: false,
        }
    }
    pub const fn with_join_limit(join_limit: usize) -> Result<Self, Refusal> {
        if join_limit < 2 || join_limit > MAX_REQUEST {
            return Err(Refusal::Capacity);
        }
        let mut decoder = Self::new();
        decoder.join_limit = join_limit;
        Ok(decoder)
    }
    pub const fn with_selected_peer(mut self) -> Self {
        self.selected_peer = true;
        self
    }
    pub fn reset(&mut self) {
        self.frame.reset()
    }
    pub fn accept(&mut self, input: &[u8]) -> Result<(), Refusal> {
        let join_limit = self.join_limit;
        let selected_peer = self.selected_peer;
        self.frame
            .accept(input, |header| {
                use conduit_device_carrier_protocol::Refusal as R;
                let length = header.length;
                match header.kind {
                    JOIN if (2..=join_limit).contains(&length) => {}
                    SELECT_PEER if selected_peer && length == 4 => {}
                    SELECT_PEER if selected_peer => return Err(R::Capacity),
                    REQUEST if (1..=MAX_REQUEST).contains(&length) => {}
                    TEST_AP_START if length == 6 => {}
                    MDNS_QUERY if (9..=56).contains(&length) => {}
                    STATUS | BOOTSEL_QUERY | SENSOR_STATUS | SENSOR_DIAGNOSTIC | TEST_AP_STOP
                        if length == 0 => {}
                    BOOTSEL_REQUEST if length == 64 => {}
                    JOIN | REQUEST | STATUS | BOOTSEL_QUERY | BOOTSEL_REQUEST | SENSOR_STATUS
                    | SENSOR_DIAGNOSTIC | TEST_AP_START | TEST_AP_STOP | MDNS_QUERY => {
                        return Err(R::Capacity)
                    }
                    _ => return Err(R::Kind),
                }
                Ok(())
            })
            .map_err(Into::into)
    }
    pub fn has_partial(&self) -> bool {
        self.frame.has_partial()
    }
    pub fn complete(&self) -> bool {
        self.frame.complete()
    }
    pub fn frame(&self) -> Result<Frame<'_>, Refusal> {
        self.frame.frame().map_err(Into::into)
    }
}
#[derive(Default)]
pub struct Session {
    inner: conduit_device_carrier_protocol::Session,
}
impl Session {
    pub fn admit(&mut self, id: u32) -> Result<(), Refusal> {
        self.inner.admit(id).map_err(Into::into)
    }
    pub fn reset(&mut self) {
        self.inner.reset()
    }
}
pub fn header(kind: u8, id: u32, length: usize) -> Result<[u8; HEADER], Refusal> {
    conduit_device_carrier_protocol::header(kind, id, length).map_err(Into::into)
}
pub struct Credentials<'a> {
    pub ssid: &'a str,
    pub password: &'a [u8],
}
/// Explicit bench peer selection controls DHCP address assignment only; it
/// carries no Body, admission or signature authority.
pub fn test_peer_mac(payload: &[u8]) -> Result<[u8; 6], Refusal> {
    let mac: [u8; 6] = payload.try_into().map_err(|_| Refusal::Peer)?;
    if mac == [0; 6] || mac[0] & 1 != 0 {
        return Err(Refusal::Peer);
    }
    Ok(mac)
}
pub fn credentials(bytes: &[u8]) -> Result<Credentials<'_>, Refusal> {
    let (&ssid_len, tail) = bytes.split_first().ok_or(Refusal::Credentials)?;
    let (&password_len, tail) = tail.split_first().ok_or(Refusal::Credentials)?;
    let (ssid_len, password_len) = (usize::from(ssid_len), usize::from(password_len));
    if !(1..=32).contains(&ssid_len)
        || password_len > 63
        || (password_len != 0 && password_len < 8)
        || tail.len() != ssid_len + password_len
    {
        return Err(Refusal::Credentials);
    }
    let ssid = core::str::from_utf8(&tail[..ssid_len]).map_err(|_| Refusal::Credentials)?;
    if ssid.as_bytes().contains(&0) || tail[ssid_len..].contains(&0) {
        return Err(Refusal::Credentials);
    }
    Ok(Credentials {
        ssid,
        password: &tail[ssid_len..],
    })
}
/// Require the exact on-link ESP AP subnet; a default router is unnecessary.
/// Refuse an advertised router other than the fixed ESP peer.
pub fn peer_network(address: [u8; 4], prefix: u8, gateway: Option<[u8; 4]>) -> bool {
    address[..3] == [192, 168, 4]
        && (2..=254).contains(&address[3])
        && prefix == 24
        && (gateway.is_none() || gateway == Some([192, 168, 4, 1]))
}
