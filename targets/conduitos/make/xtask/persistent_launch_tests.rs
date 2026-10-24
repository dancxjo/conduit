use super::*;
use std::os::unix::fs::PermissionsExt;
#[test]
fn sessions_get_independent_private_vars_and_drop_only_their_copies() {
    let root = std::env::temp_dir().join(format!(
        "conduit-launch-vars-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let template = root.join("template.fd");
    fs::write(&template, [42_u8; 4096]).unwrap();
    fs::set_permissions(&template, fs::Permissions::from_mode(0o444)).unwrap();
    let mut first = Session {
        directory: root.join("first"),
        vars: None,
    };
    let mut second = Session {
        directory: root.join("second"),
        vars: None,
    };
    for d in [&first.directory, &second.directory] {
        fs::DirBuilder::new().mode(0o700).create(d).unwrap();
    }
    let a = private_vars(&template, &mut first, false).unwrap();
    let b = private_vars(&template, &mut second, false).unwrap();
    assert_ne!(a, b);
    assert_eq!(
        fs::metadata(&a).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::write(&a, [1_u8; 4096]).unwrap();
    assert_eq!(fs::read(&template).unwrap(), vec![42; 4096]);
    assert_eq!(fs::read(&b).unwrap(), vec![42; 4096]);
    drop(first);
    assert!(!a.exists());
    assert!(b.exists());
    assert!(template.exists());
    drop(second);
    assert!(!b.exists());
    assert!(template.exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn dry_run_does_not_create_private_vars_or_acquire_ownership() {
    let mut session = Session {
        directory: PathBuf::from("nonexistent-conduit-dry-run"),
        vars: None,
    };
    let vars = private_vars(Path::new("no-template-is-read"), &mut session, true).unwrap();
    assert_eq!(vars, session.directory.join("vars.fd"));
    assert!(session.vars.is_none());
    assert!(!session.directory.exists());
}
