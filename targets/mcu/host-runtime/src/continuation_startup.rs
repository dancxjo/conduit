//! Cold installed-state classification without suppressing ordinary Root control.
//! A rejected immutable prefix is retained as evidence, never resumed or erased.
use crate::{
    continuation_artifact::PREFIX_BYTES,
    root::{ContinuationAcceptanceRefusal, InstalledContinuationAcceptance, Root},
};

/// Only Root's opaque verified acceptance can enter the installed Play path.
pub struct Installed<'a> {
    pub acceptance: InstalledContinuationAcceptance,
    pub prefix: &'a [u8; PREFIX_BYTES],
}

/// Reading stale or damaged installed state does not make the Host unavailable
/// for ordinary inspection or independently authenticated Return and Prepare.
// Keep the original already funded acceptance in place; boxing it would add
// another startup allocation solely to shrink the refused alternative.
#[allow(clippy::large_enum_variant)]
pub enum Startup<'a> {
    Absent,
    Accepted(Installed<'a>),
    Rejected {
        prefix: &'a [u8; PREFIX_BYTES],
        refusal: ContinuationAcceptanceRefusal,
    },
}
impl<'a> Startup<'a> {
    pub fn classify(
        prefix: Option<&'a [u8; PREFIX_BYTES]>,
        accept: impl FnOnce(
            &[u8; PREFIX_BYTES],
        )
            -> Result<InstalledContinuationAcceptance, ContinuationAcceptanceRefusal>,
    ) -> Self {
        let Some(prefix) = prefix else {
            return Self::Absent;
        };
        match accept(prefix) {
            Ok(acceptance) => Self::Accepted(Installed { acceptance, prefix }),
            Err(refusal) => Self::Rejected { prefix, refusal },
        }
    }
    pub fn accepted(&self) -> Option<&Installed<'a>> {
        match self {
            Self::Accepted(installed) => Some(installed),
            _ => None,
        }
    }
    pub fn refusal(&self) -> Option<ContinuationAcceptanceRefusal> {
        match self {
            Self::Rejected { refusal, .. } => Some(*refusal),
            _ => None,
        }
    }
    pub fn prefix(&self) -> Option<&'a [u8; PREFIX_BYTES]> {
        match self {
            Self::Absent => None,
            Self::Accepted(installed) => Some(installed.prefix),
            Self::Rejected { prefix, .. } => Some(prefix),
        }
    }
}

/// The same original Root checks on both boards. Scratch is caller-funded and
/// transient; refusal neither creates current membership nor attempts a Play.
pub fn verify<const N: usize>(
    root: &mut Root<'_, N>,
    prefix: &[u8; PREFIX_BYTES],
    scratch: &mut [u8; PREFIX_BYTES],
    source_digest: [u8; 32],
    pins: &[conduit_body::ContinuationBundleEntry<'_>],
) -> Result<InstalledContinuationAcceptance, ContinuationAcceptanceRefusal> {
    let accepted = root.read_continuation_acceptance(prefix, scratch)?;
    root.verify_installed_continuation_boot_digest(
        &accepted,
        prefix,
        scratch,
        root.actual().boot_id.as_str(),
        source_digest,
        pins,
    )?;
    root.restore_continuation_acceptance(&accepted)?;
    Ok(accepted)
}
