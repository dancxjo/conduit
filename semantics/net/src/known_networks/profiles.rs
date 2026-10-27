use crate::NetworkJoinRequest;
pub const MAXIMUM_KNOWN_NETWORKS: usize = 4;
pub const KNOWN_SSID_BYTES: usize = 32;
pub const KNOWN_CREDENTIAL_BYTES: usize = 63;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownSecurity {
    Wpa2Personal,
    Wpa3Personal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownProfileKey {
    pub slot: u8,
    pub generation: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnownNetworkRefusal {
    Ssid,
    Credential,
    Capacity,
    Exhausted,
    StaleProfile,
    Clock,
    Bounds,
    Outcome,
}
/// Owning private ingress. No Clone/Debug/Serialize implementation. A rejected
/// upsert drops and volatile-wipes this value. Borrowed constructor inputs remain
/// the caller's custody and must be erased independently when appropriate.
pub struct PrivateNetworkInput {
    ssid: [u8; KNOWN_SSID_BYTES],
    ssid_len: u8,
    credential: [u8; KNOWN_CREDENTIAL_BYTES],
    credential_len: u8,
    security: KnownSecurity,
    priority: i8,
}
impl PrivateNetworkInput {
    pub fn new(
        ssid: &[u8],
        credential: &[u8],
        security: KnownSecurity,
        priority: i8,
    ) -> Result<Self, KnownNetworkRefusal> {
        if ssid.is_empty()
            || ssid.len() > KNOWN_SSID_BYTES
            || core::str::from_utf8(ssid).is_err()
            || ssid.contains(&0)
        {
            return Err(KnownNetworkRefusal::Ssid);
        }
        if !(8..=KNOWN_CREDENTIAL_BYTES).contains(&credential.len())
            || !credential.iter().all(|b| (0x20..=0x7e).contains(b))
        {
            return Err(KnownNetworkRefusal::Credential);
        }
        let mut input = Self {
            ssid: [0; KNOWN_SSID_BYTES],
            ssid_len: ssid.len() as u8,
            credential: [0; KNOWN_CREDENTIAL_BYTES],
            credential_len: credential.len() as u8,
            security,
            priority,
        };
        input.ssid[..ssid.len()].copy_from_slice(ssid);
        input.credential[..credential.len()].copy_from_slice(credential);
        Ok(input)
    }
}
fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        // SAFETY: exclusively borrowed initialized bytes; volatile writes cannot escape.
        unsafe { core::ptr::write_volatile(byte, 0) }
    }
}
impl PrivateNetworkInput {
    fn erase(&mut self) {
        wipe(&mut self.ssid);
        wipe(&mut self.credential);
        self.ssid_len = 0;
        self.credential_len = 0;
    }
}
impl Drop for PrivateNetworkInput {
    fn drop(&mut self) {
        self.erase();
    }
}
#[cfg(test)]
mod erasure_tests {
    use super::*;
    #[test]
    fn drop_erasure_routine_clears_entire_private_backing() {
        let mut input = PrivateNetworkInput::new(
            b"known",
            b"private-test-pass",
            KnownSecurity::Wpa2Personal,
            0,
        )
        .unwrap();
        input.erase();
        assert_eq!(input.ssid, [0; KNOWN_SSID_BYTES]);
        assert_eq!(input.credential, [0; KNOWN_CREDENTIAL_BYTES]);
        assert_eq!((input.ssid_len, input.credential_len), (0, 0));
    }
}
struct Profile {
    input: PrivateNetworkInput,
    generation: u64,
}
/// Fixed private custody, never serialized through general-purpose formatting.
/// Persistent adapters must define their own bounded, commit-last private format.
/// Secret-bearing containers deliberately refuse general debug formatting:
/// ```compile_fail
/// use conduit_net::KnownNetworkProfiles;
/// let profiles = KnownNetworkProfiles::new();
/// let _ = format!("{:?}", profiles);
/// ```
pub struct KnownNetworkProfiles {
    profiles: [Option<Profile>; MAXIMUM_KNOWN_NETWORKS],
    revision: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownProfileInfo {
    pub key: KnownProfileKey,
    pub security: KnownSecurity,
    pub priority: i8,
}
impl Default for KnownNetworkProfiles {
    fn default() -> Self {
        Self::new()
    }
}
impl KnownNetworkProfiles {
    pub const fn new() -> Self {
        Self {
            profiles: [None, None, None, None],
            revision: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.profiles.iter().flatten().count()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Same SSID replaces security, preference and secret in its existing slot.
    /// Capacity/exhaustion refuses atomically, preserving every saved profile.
    pub fn upsert(
        &mut self,
        input: PrivateNetworkInput,
    ) -> Result<KnownProfileKey, KnownNetworkRefusal> {
        let slot = self
            .profiles
            .iter()
            .position(|p| {
                p.as_ref().is_some_and(|p| {
                    p.input.ssid[..p.input.ssid_len as usize]
                        == input.ssid[..input.ssid_len as usize]
                })
            })
            .or_else(|| self.profiles.iter().position(Option::is_none))
            .ok_or(KnownNetworkRefusal::Capacity)?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(KnownNetworkRefusal::Exhausted)?;
        self.profiles[slot] = Some(Profile {
            input,
            generation: revision,
        });
        self.revision = revision;
        Ok(KnownProfileKey {
            slot: slot as u8,
            generation: revision,
        })
    }
    fn get(&self, key: KnownProfileKey) -> Result<&Profile, KnownNetworkRefusal> {
        self.profiles
            .get(key.slot as usize)
            .and_then(Option::as_ref)
            .filter(|p| p.generation == key.generation)
            .ok_or(KnownNetworkRefusal::StaleProfile)
    }
    pub fn info(&self, key: KnownProfileKey) -> Result<KnownProfileInfo, KnownNetworkRefusal> {
        let p = self.get(key)?;
        Ok(KnownProfileInfo {
            key,
            security: p.input.security,
            priority: p.input.priority,
        })
    }
    pub fn entries(&self) -> impl Iterator<Item = KnownProfileInfo> + '_ {
        self.profiles.iter().enumerate().filter_map(|(slot, p)| {
            p.as_ref().map(|p| KnownProfileInfo {
                key: KnownProfileKey {
                    slot: slot as u8,
                    generation: p.generation,
                },
                security: p.input.security,
                priority: p.input.priority,
            })
        })
    }
    pub fn matches(&self, key: KnownProfileKey, ssid: &[u8], security: KnownSecurity) -> bool {
        self.get(key).is_ok_and(|p| {
            p.input.security == security && &p.input.ssid[..p.input.ssid_len as usize] == ssid
        })
    }
    /// Callback cannot return a borrow of the secret. This supplies only the
    /// unchanged single-selected-network request; it performs no join/read grant.
    pub fn with_request<T>(
        &self,
        key: KnownProfileKey,
        f: impl FnOnce(NetworkJoinRequest<'_>) -> T,
    ) -> Result<T, KnownNetworkRefusal> {
        let p = self.get(key)?;
        Ok(f(NetworkJoinRequest {
            ssid: &p.input.ssid[..p.input.ssid_len as usize],
            credential: &p.input.credential[..p.input.credential_len as usize],
        }))
    }
}

#[cfg(test)]
mod exhaustion_tests {
    use super::*;
    #[test]
    fn generation_exhaustion_does_not_replace_committed_private_profile() {
        let mut profiles = KnownNetworkProfiles::new();
        let key = profiles
            .upsert(
                PrivateNetworkInput::new(
                    b"known",
                    b"original-pass",
                    KnownSecurity::Wpa2Personal,
                    0,
                )
                .unwrap(),
            )
            .unwrap();
        profiles.revision = u64::MAX;
        assert_eq!(
            profiles.upsert(
                PrivateNetworkInput::new(
                    b"known",
                    b"replacement-pass",
                    KnownSecurity::Wpa3Personal,
                    1
                )
                .unwrap()
            ),
            Err(KnownNetworkRefusal::Exhausted)
        );
        profiles
            .with_request(key, |r| assert_eq!(r.credential, b"original-pass"))
            .unwrap();
        assert_eq!(
            profiles.info(key).unwrap().security,
            KnownSecurity::Wpa2Personal
        );
    }
}
