//! Complete canonical validation over borrowed immutable scan evidence.
use super::{ScanCompletion, ScanFailure};
use crate::{KnownScanObservation, KnownSecurity, ObservedNetworkSecurity};

pub struct ScanCompletionView<'a> {
    bytes: &'a [u8],
    completion: ScanCompletion,
}
impl<'a> ScanCompletionView<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, ScanFailure> {
        if bytes.len() < 24 || &bytes[..4] != b"CNR1" {
            return Err(ScanFailure::Malformed);
        }
        let completion = ScanCompletion {
            generation: u64::from_le_bytes(bytes[4..12].try_into().unwrap()),
            completed_ms: u64::from_le_bytes(bytes[12..20].try_into().unwrap()),
            count: bytes[20],
            total: u16::from_le_bytes(bytes[21..23].try_into().unwrap()),
        };
        if completion.generation == 0
            || completion.count > 16
            || completion.total < u16::from(completion.count)
            || bytes.len() != 24 + 44 * usize::from(completion.count)
            || bytes[23] != u8::from(completion.truncated())
        {
            return Err(ScanFailure::Malformed);
        }
        for row in bytes[24..].chunks_exact(44) {
            if observation(row)?.observed_ms > completion.completed_ms {
                return Err(ScanFailure::Malformed);
            }
        }
        Ok(Self { bytes, completion })
    }
    pub fn completion(&self) -> ScanCompletion {
        self.completion
    }
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
    pub fn observations(
        &self,
    ) -> impl ExactSizeIterator<Item = KnownScanObservation<'a>> + Clone + '_ {
        self.bytes[24..]
            .chunks_exact(44)
            .map(|row| observation(row).expect("same completely validated immutable scan extent"))
    }
}
fn observation(row: &[u8]) -> Result<KnownScanObservation<'_>, ScanFailure> {
    let length = usize::from(row[0]);
    if length > 32 || row[1 + length..33].iter().any(|b| *b != 0) {
        return Err(ScanFailure::Malformed);
    }
    let security = match row[35] {
        0 => ObservedNetworkSecurity::Unknown,
        1 => ObservedNetworkSecurity::Unsupported,
        2 => ObservedNetworkSecurity::Known(KnownSecurity::Wpa2Personal),
        3 => ObservedNetworkSecurity::Known(KnownSecurity::Wpa3Personal),
        _ => return Err(ScanFailure::Malformed),
    };
    Ok(KnownScanObservation {
        ssid: &row[1..1 + length],
        security,
        rssi_dbm: i16::from_le_bytes(row[33..35].try_into().unwrap()),
        observed_ms: u64::from_le_bytes(row[36..44].try_into().unwrap()),
    })
}
