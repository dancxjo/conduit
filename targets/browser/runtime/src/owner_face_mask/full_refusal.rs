//! Closed refusals for the explicitly selected Full profile only.
use conduit_face::json_model_preflight::JsonModelRefusal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FullPreparationRefusal {
    Malformed,
    Unsupported,
    Identity,
    InvalidSeal,
    Pressure,
    Lifecycle,
    Preparation,
}
impl FullPreparationRefusal {
    pub(super) const fn status(self) -> i32 {
        match self {
            Self::Malformed => -270,
            Self::Unsupported => -271,
            Self::Identity => -272,
            Self::InvalidSeal => -273,
            Self::Pressure => -274,
            Self::Lifecycle => -275,
            Self::Preparation => -276,
        }
    }
    pub(super) const fn code(self) -> &'static str {
        match self {
            Self::Malformed => "OwnerFaceMalformed",
            Self::Unsupported => "OwnerFaceUnsupported",
            Self::Identity => "OwnerFaceIdentityMismatch",
            Self::InvalidSeal => "OwnerFaceInvalidSeal",
            Self::Pressure => "OwnerFacePressure",
            Self::Lifecycle => "OwnerFaceLifecycle",
            Self::Preparation => "OwnerFacePreparationFailed",
        }
    }
}
impl From<JsonModelRefusal> for FullPreparationRefusal {
    fn from(value: JsonModelRefusal) -> Self {
        match value {
            JsonModelRefusal::Capacity => Self::Pressure,
            JsonModelRefusal::Json => Self::Malformed,
            JsonModelRefusal::Unsupported => Self::Unsupported,
        }
    }
}
// Old shared preparation helpers retain their own unclassified diagnostics.
// Never infer a typed category by matching human-readable error strings.
impl From<String> for FullPreparationRefusal {
    fn from(_: String) -> Self {
        Self::Preparation
    }
}
impl From<&'static str> for FullPreparationRefusal {
    fn from(_: &'static str) -> Self {
        Self::Preparation
    }
}
impl From<FullPreparationRefusal> for String {
    fn from(value: FullPreparationRefusal) -> Self {
        value.code().into()
    }
}

/// The selected profile owns this fixed preparation buffer. Keep writer extent
/// pressure distinct from an unexpected failure in a custom Serialize owner.
pub(super) fn bounded_json(
    value: &impl serde::Serialize,
    maximum: usize,
) -> Result<Vec<u8>, FullPreparationRefusal> {
    use std::io::Write;
    struct Output {
        bytes: Vec<u8>,
        maximum: usize,
        overflow: bool,
    }
    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {
                self.overflow = true;
                return Err(std::io::Error::other("Full output extent"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Output {
        bytes: Vec::new(),
        maximum,
        overflow: false,
    };
    output
        .bytes
        .try_reserve_exact(maximum)
        .map_err(|_| FullPreparationRefusal::Pressure)?;
    if serde_json::to_writer(&mut output, value).is_err() {
        return Err(if output.overflow {
            FullPreparationRefusal::Pressure
        } else {
            FullPreparationRefusal::Preparation
        });
    }
    Ok(output.bytes)
}

#[cfg(test)]
#[path = "full_refusal/tests.rs"]
mod tests;
