//! Bounded portable semantic abnormal-terminal information.
//!
//! This value names why a semantic obligation finally became abnormal. It is
//! not a Back failure, scheduler error, panic payload, or human diagnostic.

use crate::{semantic_digest, GearId, KindId, PortId, SignId};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub const TERMINAL_INFO_ID: &str = "conduit/terminal-info@1";
pub const TERMINAL_INFO_ENCODED_LEN: usize = 34;
pub const MAXIMUM_TERMINAL_CAUSE_SIGNS: usize = 4;
pub const MAXIMUM_TERMINAL_CAUSE_ID_BYTES: usize = 128;
const TERMINAL_INFO_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TerminalCategory {
    RefusalOrAdmission = 0,
    ExecutionFault = 1,
    Cancelled = 2,
    Deadline = 3,
    ProviderOrResourceLoss = 4,
    StaleOrInvalidated = 5,
    UnavailableRealization = 6,
    DomainOwned = 7,
}

impl TerminalCategory {
    fn decode(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::RefusalOrAdmission,
            1 => Self::ExecutionFault,
            2 => Self::Cancelled,
            3 => Self::Deadline,
            4 => Self::ProviderOrResourceLoss,
            5 => Self::StaleOrInvalidated,
            6 => Self::UnavailableRealization,
            7 => Self::DomainOwned,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalInfo {
    category: TerminalCategory,
    cause_digest: [u8; 32],
}

impl TerminalInfo {
    pub const fn new(category: TerminalCategory, cause_digest: [u8; 32]) -> Self {
        Self {
            category,
            cause_digest,
        }
    }

    pub const fn category(self) -> TerminalCategory {
        self.category
    }

    /// Correlates this bounded value with exact retained causal evidence.
    pub const fn cause_digest(self) -> [u8; 32] {
        self.cause_digest
    }

    pub const fn encode(self) -> [u8; TERMINAL_INFO_ENCODED_LEN] {
        let mut encoded = [0_u8; TERMINAL_INFO_ENCODED_LEN];
        encoded[0] = TERMINAL_INFO_VERSION;
        encoded[1] = self.category as u8;
        let mut index = 0;
        while index < self.cause_digest.len() {
            encoded[index + 2] = self.cause_digest[index];
            index += 1;
        }
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, TerminalInfoDecodeRefusal> {
        if encoded.len() != TERMINAL_INFO_ENCODED_LEN {
            return Err(TerminalInfoDecodeRefusal::WrongLength {
                actual: encoded.len(),
            });
        }
        if encoded[0] != TERMINAL_INFO_VERSION {
            return Err(TerminalInfoDecodeRefusal::UnsupportedVersion(encoded[0]));
        }
        let category = TerminalCategory::decode(encoded[1])
            .ok_or(TerminalInfoDecodeRefusal::UnknownCategory(encoded[1]))?;
        let mut cause_digest = [0_u8; 32];
        cause_digest.copy_from_slice(&encoded[2..]);
        Ok(Self {
            category,
            cause_digest,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalInfoDecodeRefusal {
    WrongLength { actual: usize },
    UnsupportedVersion(u8),
    UnknownCategory(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCauseRefusal {
    TooManySigns { actual: usize },
    EmptyIdentity,
    IdentityTooLong { actual: usize },
}

/// Derives the stable causal correlation carried by [`TerminalInfo`].
///
/// The exact identities remain in bounded Sign/evidence storage. The portable
/// terminal value carries their canonical digest so another layer can verify
/// correlation without copying variable-size provenance into every `!` value.
pub fn terminal_cause_digest(
    origin_kind: &KindId,
    origin_gear: &GearId,
    origin_port: &PortId,
    cause_signs: &[SignId],
    domain_kind: Option<&KindId>,
) -> Result<[u8; 32], TerminalCauseRefusal> {
    if cause_signs.len() > MAXIMUM_TERMINAL_CAUSE_SIGNS {
        return Err(TerminalCauseRefusal::TooManySigns {
            actual: cause_signs.len(),
        });
    }
    let mut canonical = Vec::new();
    push_identity(&mut canonical, origin_kind.as_str())?;
    push_identity(&mut canonical, origin_gear.as_str())?;
    push_identity(&mut canonical, origin_port.as_str())?;
    canonical.push(u8::from(domain_kind.is_some()));
    if let Some(kind) = domain_kind {
        push_identity(&mut canonical, kind.as_str())?;
    }
    canonical.push(cause_signs.len() as u8);
    for sign in cause_signs {
        push_identity(&mut canonical, sign.as_str())?;
    }
    Ok(semantic_digest("conduit/terminal-cause@1", &canonical))
}

fn push_identity(canonical: &mut Vec<u8>, identity: &str) -> Result<(), TerminalCauseRefusal> {
    if identity.is_empty() {
        return Err(TerminalCauseRefusal::EmptyIdentity);
    }
    let length = identity.len();
    if length > MAXIMUM_TERMINAL_CAUSE_ID_BYTES {
        return Err(TerminalCauseRefusal::IdentityTooLong { actual: length });
    }
    canonical.push(length as u8);
    canonical.extend_from_slice(identity.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_round_trips_in_one_exact_bounded_shape() {
        for category in [
            TerminalCategory::RefusalOrAdmission,
            TerminalCategory::ExecutionFault,
            TerminalCategory::Cancelled,
            TerminalCategory::Deadline,
            TerminalCategory::ProviderOrResourceLoss,
            TerminalCategory::StaleOrInvalidated,
            TerminalCategory::UnavailableRealization,
            TerminalCategory::DomainOwned,
        ] {
            let info = TerminalInfo::new(category, [category as u8; 32]);
            let encoded = info.encode();
            assert_eq!(encoded.len(), TERMINAL_INFO_ENCODED_LEN);
            assert_eq!(TerminalInfo::decode(&encoded), Ok(info));
        }
    }

    #[test]
    fn malformed_values_and_unbounded_cause_sets_refuse() {
        assert_eq!(
            TerminalInfo::decode(&[0; TERMINAL_INFO_ENCODED_LEN - 1]),
            Err(TerminalInfoDecodeRefusal::WrongLength {
                actual: TERMINAL_INFO_ENCODED_LEN - 1
            })
        );
        let mut encoded = TerminalInfo::new(TerminalCategory::Cancelled, [0; 32]).encode();
        encoded[0] = 2;
        assert_eq!(
            TerminalInfo::decode(&encoded),
            Err(TerminalInfoDecodeRefusal::UnsupportedVersion(2))
        );
        encoded[0] = TERMINAL_INFO_VERSION;
        encoded[1] = 255;
        assert_eq!(
            TerminalInfo::decode(&encoded),
            Err(TerminalInfoDecodeRefusal::UnknownCategory(255))
        );
        let signs = (0..=MAXIMUM_TERMINAL_CAUSE_SIGNS)
            .map(|index| SignId::from(alloc::format!("sign/{index}")))
            .collect::<Vec<_>>();
        assert_eq!(
            terminal_cause_digest(
                &KindId::from("work/kind"),
                &GearId::from("work"),
                &PortId::from("out"),
                &signs,
                None,
            ),
            Err(TerminalCauseRefusal::TooManySigns {
                actual: MAXIMUM_TERMINAL_CAUSE_SIGNS + 1
            })
        );
    }

    #[test]
    fn causal_digest_binds_every_exact_origin_and_domain_fact() {
        let signs = [SignId::from("sign/one"), SignId::from("sign/two")];
        let baseline = terminal_cause_digest(
            &KindId::from("work/kind"),
            &GearId::from("work"),
            &PortId::from("out"),
            &signs,
            Some(&KindId::from("work/domain-fault")),
        )
        .unwrap();
        assert_ne!(
            baseline,
            terminal_cause_digest(
                &KindId::from("work/kind"),
                &GearId::from("other"),
                &PortId::from("out"),
                &signs,
                Some(&KindId::from("work/domain-fault")),
            )
            .unwrap()
        );
        assert_ne!(
            baseline,
            terminal_cause_digest(
                &KindId::from("work/kind"),
                &GearId::from("work"),
                &PortId::from("out"),
                &signs,
                None,
            )
            .unwrap()
        );
    }
}
