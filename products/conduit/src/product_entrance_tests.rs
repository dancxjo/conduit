use super::*;

#[test]
fn resident_entrance_uses_the_workspace_browser_application() {
    let root = std::env::temp_dir().join(format!(
        "conduit-workspace-entrance-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let application = root.join("conduit-workspace");
    std::fs::create_dir_all(&application).unwrap();
    std::fs::write(application.join("index.html"), b"workspace").unwrap();

    let command = workspace_command(&root.join("conduit")).unwrap();
    assert_eq!(command.get_program(), "conduit-browser-host");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            std::ffi::OsStr::new("--application"),
            application.as_os_str(),
            std::ffi::OsStr::new("--mount"),
            std::ffi::OsStr::new("/workspace/"),
        ]
    );

    std::fs::remove_dir_all(root).unwrap();
}
