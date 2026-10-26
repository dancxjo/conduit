//! A replacement carrier process retains the Body instead of repeating birth.
use super::*;
#[test]
fn resume_keeps_body_identity_and_changes_local_incarnation() {
    let state = std::env::temp_dir().join(format!(
        "conduit-owner-resume-{}",
        std::fs::read_to_string("/proc/sys/kernel/random/uuid")
            .unwrap()
            .trim()
    ));
    // Random directory creation refuses collision rather than overwriting custody.
    std::fs::create_dir(&state).unwrap();
    let result = (|| -> Result<()> {
        provision(&state)?;
        let first = Authority::new(&state)?;
        let source = checked(conduit_environment_runtime::construction::compile_default())?;
        let body = first.birth(&source, None)?;
        let identity = body.evidence().body_id.clone();
        let second = Authority::new(&state)?;
        let resumed = second.resume(&first.state.join("body-born.json"))?;
        assert_eq!(resumed.evidence().body_id, identity);
        assert_ne!(first.host.boot_id, second.host.boot_id);
        assert_eq!(resumed.evidence().body.state, b::BodyState::Lulled);
        Ok(())
    })();
    std::fs::remove_dir_all(&state).unwrap();
    result.unwrap();
}
