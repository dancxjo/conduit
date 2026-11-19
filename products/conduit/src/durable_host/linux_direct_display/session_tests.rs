use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn install_activates_after_verification_and_restores_getty_on_failure() {
    for mode in ["success", "failure", "desktop"] {
        let root = std::env::temp_dir().join(crate::durable_host::fresh_identity("display", mode));
        fs::create_dir_all(&root).unwrap();
        let mut process = Command::new("/bin/sh")
            .arg("-c")
            .arg(concat!(
                include_str!("../linux_direct_display_activate.sh"),
                "\n",
                include_str!("session_fixture.sh"),
            ))
            .arg("fixture")
            .arg(&root)
            .arg(mode)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        process
            .stdin
            .take()
            .unwrap()
            .write_all(b"exact unit\n")
            .unwrap();
        let status = process.wait().unwrap();
        let commands = fs::read_to_string(root.join("commands")).unwrap_or_default();
        let unit = root.join("system/conduit-direct-display.service");
        match mode {
            "success" => {
                assert!(status.success());
                assert_eq!(fs::read_to_string(unit).unwrap(), "exact unit\n");
                assert!(commands.ends_with("enable --now conduit-direct-display.service\n"));
            }
            "failure" => {
                assert!(!status.success() && !unit.exists());
                assert!(commands.ends_with("enable getty@tty1.service\nstart getty@tty1.service\n"));
            }
            _ => {
                assert!(!status.success() && !unit.exists());
                assert!(commands.is_empty());
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
