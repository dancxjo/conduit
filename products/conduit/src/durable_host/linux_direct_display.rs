//! Optional Linux session preparation. Never installs or activates a service.
use std::path::Path;

pub(super) fn prepare(
    manifest: &Path,
    state: &Path,
    output: &Path,
    user: &str,
    tty: u8,
) -> Result<(), String> {
    #[cfg(all(target_os = "linux", feature = "native-display"))]
    {
        linux::prepare(manifest, state, output, user, tty)
    }
    #[cfg(not(all(target_os = "linux", feature = "native-display")))]
    {
        let _ = (manifest, state, output, user, tty);
        Err("direct display preparation requires the installed Linux product with native-display; ordinary user-service startup remains available".into())
    }
}

#[cfg(all(target_os = "linux", any(test, feature = "native-display")))]
fn validate_session(user: &str, tty: u8) -> Result<(), String> {
    if !(1..=63).contains(&tty)
        || user.is_empty()
        || user.len() > 32
        || !user
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
        || user.starts_with('-')
        || user == "root"
    {
        return Err("direct display requires an unprivileged local account name and a dedicated tty from 1 through 63".into());
    }
    Ok(())
}

#[cfg(all(target_os = "linux", any(test, feature = "native-display")))]
fn shell_argument(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(all(target_os = "linux", any(test, feature = "native-display")))]
fn definition(executable: &Path, state: &Path, user: &str, tty: u8) -> Result<String, String> {
    validate_session(user, tty)?;
    // The colon prefix disables systemd dollar expansion, preserving literal paths.
    let executable = super::systemd_exec_argument(executable)?;
    let state = super::systemd_exec_argument(state)?;
    Ok(format!(
        "[Unit]\nDescription=Conduit optional direct display session\nAfter=systemd-user-sessions.service systemd-logind.service\nRequires=systemd-logind.service\nConflicts=getty@tty{tty}.service\nConditionPathExists=/dev/tty{tty}\n\n[Service]\nUser={user}\nPAMName=login\nTTYPath=/dev/tty{tty}\nStandardInput=tty\nStandardOutput=journal\nStandardError=journal\nTTYReset=yes\nTTYVHangup=yes\nTTYVTDisallocate=yes\nUtmpIdentifier=tty{tty}\nEnvironment=XDG_SESSION_TYPE=tty XDG_SEAT=seat0 XDG_VTNR={tty}\nExecStart=:{executable} host service run --state-dir {state}\nRestart=on-failure\nRestartSec=5s\nUMask=0077\n\n[Install]\nWantedBy=multi-user.target\n"
    ))
}

#[cfg(all(target_os = "linux", feature = "native-display"))]
mod linux {
    use super::super::{
        bounded_read, owner_lock, read_installation, restrict_directory, validate_manifest,
        verify_installed_release, ReleaseManifest,
    };
    use super::*;
    use std::{fs, os::unix::fs::MetadataExt, process::Command};

    pub(super) fn prepare(
        manifest: &Path,
        state: &Path,
        output: &Path,
        user: &str,
        tty: u8,
    ) -> Result<(), String> {
        validate_session(user, tty)?;
        let state = fs::canonicalize(state).map_err(|e| format!("resolve installed state: {e}"))?;
        let _host = owner_lock::acquire(&state)?;
        let _body = owner_lock::body(&state)?;
        let installation = read_installation(&state.join("installation.json"))?;
        if installation.body_state.is_some() {
            return Err("direct display currently supports joined/member presentation only; owned-Body interactive bootstrap is not integrated".into());
        }
        let selection = installation
            .selected_host_configuration
            .as_ref()
            .ok_or("direct display requires retained checked Host configuration")?;
        selection
            .scanout_parameters()?
            .ok_or("checked Host configuration does not select initialized display/scanout")?;
        conduit_workspace_make::CheckedNativeMaskInitialization::from_configuration(
            &selection.checked()?,
        )?;
        let manifest: ReleaseManifest =
            serde_json::from_slice(&bounded_read(manifest, 256 * 1024)?)
                .map_err(|e| format!("release manifest: {e}"))?;
        validate_manifest(&manifest)?;
        if manifest.bundle_sha256 != installation.release_bundle_sha256
            || manifest.source_identity != installation.release_source_identity
        {
            return Err("release manifest differs from the installed Host release".into());
        }
        let executable = fs::canonicalize(&installation.product_executable)
            .map_err(|e| format!("resolve installed executable: {e}"))?;
        let current = fs::canonicalize(std::env::current_exe().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if current != executable {
            return Err("run prepare-direct-display through this Host's exact installed executable to verify its compiled native-display support".into());
        }
        let release = executable
            .parent()
            .ok_or("installed executable has no release directory")?;
        if !manifest
            .files
            .iter()
            .any(|file| Path::new(&file.path).file_name() == executable.file_name())
        {
            return Err("installed executable is absent from the exact release manifest".into());
        }
        verify_installed_release(release, &manifest.files)?;
        let identity = Command::new("id")
            .args(["-u", "--", user])
            .output()
            .map_err(|e| format!("inspect session account: {e}"))?;
        let uid = String::from_utf8_lossy(&identity.stdout)
            .trim()
            .parse::<u32>()
            .map_err(|_| "session account has no numeric uid")?;
        if !identity.status.success()
            || uid == 0
            || fs::metadata(&state).map_err(|e| e.to_string())?.uid() != uid
        {
            return Err("session account must be unprivileged and own installed Host state".into());
        }
        let unit = definition(&executable, &state, user, tty)?;
        fs::create_dir(output).map_err(|e| format!("create new review directory: {e}"))?;
        restrict_directory(output)?;
        let output = fs::canonicalize(output).map_err(|e| e.to_string())?;
        fs::write(output.join("conduit-direct-display.service"), unit)
            .map_err(|e| e.to_string())?;
        let source = shell_argument(
            output
                .join("conduit-direct-display.service")
                .to_str()
                .ok_or("review path is not UTF-8")?,
        );
        let review = format!(
            "# Optional Linux direct display session — review before activation\n\nPrepared for Host `{}`, release `{}`, checked configuration `{}`, account `{user}`, tty{tty}. No service or host settings were changed. Existing normal user-service installation remains the default.\n\nThis is read-only joined/member Face presentation using the same checked IMAGE, Mask, compositor and physical Show acknowledgment as the installed Host. It is not the full interactive ConduitOS GUI: keyboard routing and owned-Body bootstrap remain future work under #6325 and the shared Host GUI goal. No physical output or successful logind session is proved by preparation.\n\nBefore activation, verify this Linux installation provides systemd-logind, PAM login session support (/etc/pam.d/login), an active seat0 local VT, SDL2 with KMSDRM, GBM/EGL/Mesa, readable input devices and authorized DRM master access. Verify the selected tty and DRM device are unused; do not replace an active desktop session. Secure Boot, distribution PAM policy and seat ACLs require local verification. Do not grant broad device access to work around refusals. The generated unit deliberately uses the existing installed executable and private state; it creates no second Body or receiver. Reprepare after changing the installed release or checked configuration.\n\nRecord the current state before changing anything:\n\n```sh\nsystemctl --user is-enabled conduit-host.service\nsystemctl --user is-active conduit-host.service\nsudo systemctl is-enabled getty@tty{tty}.service\nsudo systemctl is-active getty@tty{tty}.service\nsudo systemctl cat conduit-direct-display.service\n```\n\nRetain those results privately. If conduit-direct-display.service already exists, stop and preserve it; this recipe must not overwrite an existing installation. Ensure remote maintenance access before surrendering a local VT. Review the generated unit and verify its syntax with `systemd-analyze verify` first. Activation requires a separate explicit approval; no command below has been executed. Run user-service commands as `{user}` in that user's session:\n\n```sh\nsystemctl --user disable --now conduit-host.service\nsudo install -m 0644 -- {source} /etc/systemd/system/conduit-direct-display.service\nsudo systemctl daemon-reload\nsudo systemctl enable --now conduit-direct-display.service\nsudo journalctl -u conduit-direct-display.service -b\n```\n\nThe unit conflicts with getty@tty{tty}, but does not mask it or change the default boot target. First verify an acknowledged HDMI Show, session/DRM loss refusal, cold reboot and maintained remote access. Historical pixels do not prove a current owner encounter.\n\nRollback: disable/stop conduit-direct-display.service, remove only the unit installed by this recipe, and daemon-reload. Restore conduit-host.service and getty@tty{tty} to their individually recorded enabled/active states (including disabled/inactive states); do not blindly enable either. Never run the system and user Host services together. If activation fails, preserve the journal and restore the recorded states before another attempt.\n\n```sh\nsudo systemctl disable --now conduit-direct-display.service\nsudo rm -- /etc/systemd/system/conduit-direct-display.service\nsudo systemctl daemon-reload\n```\n\nThen apply only the enable/disable and start/stop operations matching the saved state. No desktop configuration, device permissions, credentials or biography are removed by this recipe.\n",
            installation.host_id, installation.release_bundle_sha256, selection.configuration_id()
        );
        fs::write(output.join("REVIEW.md"), review).map_err(|e| e.to_string())?;
        println!("prepared optional direct-display review artifacts at {}; activation and physical proof remain pending", output.display());
        Ok(())
    }
}

#[cfg(all(test, not(all(target_os = "linux", feature = "native-display"))))]
#[test]
fn preparation_refuses_without_compiled_native_display_before_writing() {
    let error = prepare(
        Path::new("missing-manifest"),
        Path::new("missing-state"),
        Path::new("must-not-be-created"),
        "conduit",
        7,
    )
    .unwrap_err();
    assert!(error.contains("requires the installed Linux product with native-display"));
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn session_refuses_privilege_and_unit_injection() {
        for user in ["root", "", "-user", "user\nExecStart=evil", "user%u", "a b"] {
            assert!(validate_session(user, 7).is_err());
        }
        assert!(validate_session("conduit-user", 7).is_ok());
        assert!(validate_session("conduit", 0).is_err());
        assert!(validate_session("conduit", 64).is_err());
    }
    #[test]
    fn definition_keeps_exact_paths_and_explicit_session() {
        let unit = definition(
            Path::new("/opt/Conduit $bin/%x"),
            Path::new("/srv/body state/%h"),
            "conduit",
            7,
        )
        .unwrap();
        assert!(unit.contains("ExecStart=:\"/opt/Conduit $bin/%%x\" host service run --state-dir \"/srv/body state/%%h\""));
        assert!(unit.contains("PAMName=login\nTTYPath=/dev/tty7"));
        assert!(unit.contains("User=conduit\n"));
        assert!(unit.contains("WantedBy=multi-user.target"));
        assert!(!unit.contains("SupplementaryGroups"));
        assert!(definition(Path::new("/bad\npath"), Path::new("/state"), "conduit", 7).is_err());
    }
    #[test]
    fn recipe_quotes_shell_metacharacters_as_data() {
        assert_eq!(shell_argument("/a'b/$(secret)"), "'/a'\\''b/$(secret)'");
    }
}
