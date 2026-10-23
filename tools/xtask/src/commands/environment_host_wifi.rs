//! Foreground installation and narrowly scoped reconnection for the existing
//! climate HIL profile. No network credentials enter these commands.
use std::{error::Error, process::Command};
pub(super) fn dispatch(arguments: &[String]) -> Option<Result<(), Box<dyn Error>>> {
    let args: Vec<_> = arguments.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["setup", "environment-host-wifi"] => Some(setup()),
        ["prove", "environment", "host-wifi"] => Some(connect()),
        _ => None,
    }
}
fn script() -> Result<std::path::PathBuf, Box<dyn Error>> {
    Ok(crate::workspace::workspace_root()?
        .join("bodies/environment/tools/install-host-wifi-test.sh"))
}
fn setup() -> Result<(), Box<dyn Error>> {
    let script = script()?;
    println!("The installer grants this exact Wi-Fi reconnection command:");
    success(Command::new("sh").arg(&script).arg("--print"))?;
    println!("Host sudo authentication is required to install the scoped grant.");
    success(Command::new("sudo").arg("sh").arg(script).arg("--install"))
}
fn connect() -> Result<(), Box<dyn Error>> {
    let facts = Command::new("sh").arg(script()?).arg("--facts").output()?;
    if !facts.status.success() {
        return Err(String::from_utf8_lossy(&facts.stderr)
            .trim()
            .to_owned()
            .into());
    }
    let text = std::str::from_utf8(&facts.stdout)?;
    let lines: Vec<_> = text.lines().collect();
    let [uuid, interface] = lines.as_slice() else {
        return Err("invalid host Wi-Fi profile facts".into());
    };
    let status = Command::new("sudo")
        .args([
            "-n",
            "/usr/bin/nmcli",
            "--wait",
            "30",
            "connection",
            "up",
            "uuid",
            uuid,
            "ifname",
            interface,
        ])
        .status()?;
    if !status.success() {
        return Err("Host Wi-Fi reconnection failed. If sudo authentication is required, run cargo xtask setup environment-host-wifi in a foreground host terminal.".into());
    }
    Ok(())
}
fn success(command: &mut Command) -> Result<(), Box<dyn Error>> {
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("host Wi-Fi setup failed: {status}").into())
    }
}
