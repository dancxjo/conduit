//! Private host-side owner key custody. Only the public key enters firmware.
use ed25519_dalek::SigningKey;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::{error::Error, path::Path};
#[cfg(unix)]
use std::{
    fs,
    io::{Read, Write},
};
#[cfg(unix)]
pub fn open(directory: &Path, create: bool) -> Result<SigningKey, Box<dyn Error>> {
    if create && !directory.exists() {
        fs::DirBuilder::new().mode(0o700).create(directory)?;
    }
    let metadata = fs::symlink_metadata(directory)?;
    if !metadata.is_dir() || metadata.permissions().mode() & 0o777 != 0o700 {
        return Err("owner key directory must be a private directory (0700)".into());
    }
    let private = directory.join("owner.seed");
    if create && !private.exists() {
        let mut seed = [0; 32];
        fs::File::open("/dev/urandom")?.read_exact(&mut seed)?;
        if seed == [0; 32] {
            return Err("owner entropy unavailable".into());
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&private)?;
        file.write_all(&seed)?;
        file.sync_all()?;
        seed.fill(0);
    }
    let metadata = fs::symlink_metadata(&private)?;
    if !metadata.is_file() || metadata.len() != 32 || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err("owner seed must be a private 32-byte regular file (0600)".into());
    }
    let mut seed = [0; 32];
    fs::File::open(&private)?.read_exact(&mut seed)?;
    if seed == [0; 32] {
        return Err("owner seed must not be zero".into());
    }
    let key = SigningKey::from_bytes(&seed);
    seed.fill(0);
    let public = directory.join("owner.pub");
    let expected = key.verifying_key().to_bytes();
    if create && !public.exists() {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&public)?;
        file.write_all(&expected)?;
        file.sync_all()?;
    }
    let metadata = fs::symlink_metadata(&public)?;
    if !metadata.is_file() || metadata.len() != 32 || fs::read(&public)? != expected {
        return Err("owner public key does not match retained private key".into());
    }
    Ok(key)
}

#[cfg(not(unix))]
pub fn open(_: &Path, _: bool) -> Result<SigningKey, Box<dyn Error>> {
    Err("private owner-key provisioning currently requires Unix file permissions".into())
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn private_custody_reuses_exact_key_and_refuses_substitution_or_public_seed() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("conduit-owner-key-{}-{stamp}", std::process::id()));
        let key = open(&directory, true).unwrap();
        assert_eq!(
            open(&directory, false).unwrap().verifying_key(),
            key.verifying_key()
        );
        let seed = directory.join("owner.seed");
        fs::set_permissions(&seed, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(open(&directory, false).is_err());
        fs::set_permissions(&seed, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(directory.join("owner.pub"), [0; 32]).unwrap();
        assert!(open(&directory, true).is_err());
        fs::write(directory.join("owner.pub"), key.verifying_key().to_bytes()).unwrap();
        fs::rename(&seed, directory.join("other.seed")).unwrap();
        std::os::unix::fs::symlink("other.seed", &seed).unwrap();
        assert!(open(&directory, true).is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}
