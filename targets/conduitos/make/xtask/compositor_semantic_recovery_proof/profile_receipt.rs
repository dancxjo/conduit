//! Bind actual guest memory admission to the same resolved normal native Make.
use super::super::ConduitosError;
use serde_json::Value;
const PREFIX: &str = "CONDUIT_COMPOSITOR_SEMANTIC_RECOVERY_PROFILE ";
pub(super) fn validate(
    transcript: &str,
    profile: &str,
    build: &str,
    image: &str,
    arena: u64,
) -> Result<Value, ConduitosError> {
    let refusal = || {
        ConduitosError::refusal(
            "semantic-recovery-profile-mismatch",
            "requires one exact current native Make identity, explicit proof feature and admitted runtime budget",
        )
    };
    let mut lines = transcript
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX));
    let value: Value =
        serde_json::from_str(lines.next().ok_or_else(refusal)?).map_err(|_| refusal())?;
    let expected = serde_json::json!({"schema":"conduit.conduitos/compositor-semantic-recovery-profile@1",
        "proof_feature":"compositor-semantic-recovery-proof","profile_id":profile,
        "build_id":build,"image_binding":image,"runtime_arena_bytes":arena,
        "runtime_arena_ceiling":arena});
    if arena == 0 || lines.next().is_some() || value != expected {
        return Err(refusal());
    }
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_same_native_admission_and_distinct_explicit_feature() {
        let value = serde_json::json!({"schema":"conduit.conduitos/compositor-semantic-recovery-profile@1",
            "proof_feature":"compositor-semantic-recovery-proof","profile_id":"native-profile",
            "build_id":"native-build","image_binding":"native-image",
            "runtime_arena_bytes":268435456,"runtime_arena_ceiling":268435456});
        let text = format!("{PREFIX}{value}\n");
        assert_eq!(
            validate(
                &text,
                "native-profile",
                "native-build",
                "native-image",
                268435456
            )
            .unwrap(),
            value
        );
        for (profile, build, image, arena) in [
            ("other", "native-build", "native-image", 268435456),
            ("native-profile", "other", "native-image", 268435456),
            ("native-profile", "native-build", "other", 268435456),
            ("native-profile", "native-build", "native-image", 16777216),
        ] {
            assert!(validate(&text, profile, build, image, arena).is_err());
        }
        for field in value.as_object().unwrap().keys() {
            let mut bad = value.clone();
            bad[field] = Value::Null;
            assert!(validate(
                &format!("{PREFIX}{bad}\n"),
                "native-profile",
                "native-build",
                "native-image",
                268435456
            )
            .is_err());
        }
        assert!(validate(
            &format!("{text}{text}"),
            "native-profile",
            "native-build",
            "native-image",
            268435456
        )
        .is_err());
    }
}
