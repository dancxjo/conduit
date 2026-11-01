use conduit_net::{
    NetworkJoinError, NetworkJoinRequest, MAXIMUM_CREDENTIAL_BYTES, MAXIMUM_SSID_BYTES,
};
use zeroize::Zeroize;

/// Limits come from the selected Host implementation, not authored Plot meaning.
#[derive(Clone, Copy)]
pub struct NetworkSetupLimits {
    pub minimum_credential_bytes: usize,
    pub maximum_credential_bytes: usize,
}
impl NetworkSetupLimits {
    pub(crate) fn validate(self) -> Result<(), NetworkSetupRefusal> {
        if self.minimum_credential_bytes == 0
            || self.minimum_credential_bytes > self.maximum_credential_bytes
            || self.maximum_credential_bytes > MAXIMUM_CREDENTIAL_BYTES
        {
            return Err(NetworkSetupRefusal::InvalidLimits);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkSetupRefusal {
    InvalidLimits,
    TooLarge,
    MalformedForm,
    DuplicateField,
    UnknownField,
    MissingField,
    InvalidSsid,
    InvalidCredential,
    Busy,
    Exhausted,
    NoSubmission,
    StaleSubmission,
    NotAcknowledged,
    AlreadyAcknowledged,
    Wire(NetworkJoinError),
}
/// Secret-bearing bounded storage. Deliberately no Debug, Display, Clone or serde.
pub struct NetworkSetupInput {
    ssid: [u8; MAXIMUM_SSID_BYTES],
    ssid_len: usize,
    credential: [u8; MAXIMUM_CREDENTIAL_BYTES],
    credential_len: usize,
}
impl NetworkSetupInput {
    /// Decode the complete application/x-www-form-urlencoded body only. The
    /// HTTP layer must separately bound framing, body length and request lifetime.
    pub fn from_form(body: &[u8], limits: NetworkSetupLimits) -> Result<Self, NetworkSetupRefusal> {
        use NetworkSetupRefusal as E;
        limits.validate()?;
        if body.len() > 3 * (MAXIMUM_SSID_BYTES + limits.maximum_credential_bytes) + 15 {
            return Err(E::TooLarge);
        }
        let mut input = Self {
            ssid: [0; MAXIMUM_SSID_BYTES],
            ssid_len: 0,
            credential: [0; MAXIMUM_CREDENTIAL_BYTES],
            credential_len: 0,
        };
        let (mut ssid_seen, mut credential_seen) = (false, false);
        for field in body.split(|b| *b == b'&') {
            let equal = field
                .iter()
                .position(|b| *b == b'=')
                .ok_or(E::MalformedForm)?;
            let (name, value) = (&field[..equal], &field[equal + 1..]);
            match name {
                b"ssid" => {
                    if ssid_seen {
                        return Err(E::DuplicateField);
                    }
                    ssid_seen = true;
                    input.ssid_len = decode(value, &mut input.ssid)?;
                }
                b"password" => {
                    if credential_seen {
                        return Err(E::DuplicateField);
                    }
                    credential_seen = true;
                    input.credential_len = decode(
                        value,
                        &mut input.credential[..limits.maximum_credential_bytes],
                    )?;
                }
                _ => return Err(E::UnknownField),
            }
        }
        if !ssid_seen || !credential_seen {
            return Err(E::MissingField);
        }
        if input.ssid_len == 0 || core::str::from_utf8(&input.ssid[..input.ssid_len]).is_err() {
            return Err(E::InvalidSsid);
        }
        if input.credential_len < limits.minimum_credential_bytes {
            return Err(E::InvalidCredential);
        }
        Ok(input)
    }
    pub fn request(&self) -> NetworkJoinRequest<'_> {
        NetworkJoinRequest {
            ssid: &self.ssid[..self.ssid_len],
            credential: &self.credential[..self.credential_len],
        }
    }
}
impl Drop for NetworkSetupInput {
    fn drop(&mut self) {
        self.ssid.zeroize();
        self.credential.zeroize();
    }
}
fn decode(mut input: &[u8], output: &mut [u8]) -> Result<usize, NetworkSetupRefusal> {
    let mut length = 0;
    while let Some((&first, rest)) = input.split_first() {
        let value;
        (value, input) = match first {
            b'+' => (b' ', rest),
            b'%' if rest.len() >= 2 => ((hex(rest[0])? << 4) | hex(rest[1])?, &rest[2..]),
            b'%' => return Err(NetworkSetupRefusal::MalformedForm),
            _ => (first, rest),
        };
        *output
            .get_mut(length)
            .ok_or(NetworkSetupRefusal::TooLarge)? = value;
        length += 1;
    }
    Ok(length)
}
fn hex(value: u8) -> Result<u8, NetworkSetupRefusal> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(NetworkSetupRefusal::MalformedForm),
    }
}
