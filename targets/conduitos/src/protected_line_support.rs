//! Exact refusal for a protected Line profile that ConduitOS cannot yet realize.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectedLineRefusal {
    pub implementation_id: &'static str,
    pub code: &'static str,
    pub detail: &'static str,
}

/// ConduitOS has no admitted entropy source or compiled Noise realization yet.
/// It must not advertise the portable profile merely because its contract types
/// compile for the target.
pub const fn require_protected_line() -> Result<(), ProtectedLineRefusal> {
    Err(ProtectedLineRefusal {
        implementation_id: conduit_protected_line::IMPLEMENTATION_ID,
        code: "protected-line-implementation-unavailable",
        detail: "ConduitOS has no admitted entropy source and Noise session realization",
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn conduitos_refuses_instead_of_advertising_false_protection() {
        let refusal = super::require_protected_line().unwrap_err();
        assert_eq!(
            refusal.implementation_id,
            "conduit.line/noise-nnpsk0-25519-chachapoly-sha256@1"
        );
        assert_eq!(refusal.code, "protected-line-implementation-unavailable");
    }
}
