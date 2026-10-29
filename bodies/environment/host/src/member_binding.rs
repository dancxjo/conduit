//! Bounded, descriptor-checked reopening of independently pinned member evidence.
use conduit_body::{BodyId, MembershipCredential, SpawnInvitationClaim};
use conduit_core::{BootId, HostId};
use conduit_environment_runtime::member_receipts::{admission, current};
use serde::Deserialize;
use std::{
    ffi::CString,
    fs::File,
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, OpenOptionsExt},
        },
    },
    path::Path,
};
const MAXIMUM_BINDING_BYTES: usize = 16_384;
/// Pins supplied by the actual caller's trust/current-Host configuration, never
/// selected from the file being verified. This value itself creates no authority.
pub struct ExpectedMember<'a> {
    pub root: &'a [u8; 32],
    pub body: &'a BodyId,
    pub host: &'a HostId,
    pub boot: &'a BootId,
}
#[derive(Debug)]
pub enum Refusal {
    Io(std::io::Error),
    Json(serde_json::Error),
    Scope(&'static str),
}
impl From<std::io::Error> for Refusal {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for Refusal {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Receipt {
    Admission(admission::JoinedHostReceipt),
    Current(current::Receipt),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    schema: String,
    invitation: SpawnInvitationClaim,
    root_key: [u8; 32],
    local_host: HostId,
    local_boot: BootId,
    receipt: Receipt,
    #[serde(default)]
    recovery_request: Option<current::Request>,
}
/// Only evidence for the supplied pins. No admission, presence, resource grant,
/// current producer reachability or worker capability follows from possession.
pub struct ValidatedMemberBinding(Binding);
fn id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
impl ValidatedMemberBinding {
    pub fn root(&self) -> &[u8; 32] {
        &self.0.root_key
    }
    /// Retained public claim matched by Body/identifier. Its nonce/expiry are
    /// not independently attested by the receipt; future continuity signing
    /// must also match the actual private bootstrap claim.
    pub fn invitation(&self) -> &SpawnInvitationClaim {
        &self.0.invitation
    }
    pub fn credential(&self) -> &MembershipCredential {
        match &self.0.receipt {
            Receipt::Admission(r) => &r.credential,
            Receipt::Current(r) => &r.credential,
        }
    }
    fn verify(binding: Binding, expected: &ExpectedMember<'_>) -> Result<Self, Refusal> {
        let refuse = Refusal::Scope;
        if binding.root_key != *expected.root
            || binding.invitation.body_id != *expected.body
            || binding.local_host != *expected.host
            || binding.local_boot != *expected.boot
        {
            return Err(refuse("independent member pins differ"));
        }
        let credential = match &binding.receipt {
            Receipt::Admission(receipt) => {
                if ![
                    receipt.producer_host.as_str(),
                    receipt.producer_boot.as_str(),
                ]
                .into_iter()
                .all(id)
                {
                    return Err(refuse("producer identity extent"));
                }
                receipt.verify(expected.root).map_err(refuse)?;
                if binding.schema != "conduit.environment/joined-recorder@1"
                    || binding.recovery_request.is_some()
                    || binding.invitation.invitation_id != receipt.invitation_id
                    || receipt.credential.issued_at_millis > binding.invitation.expires_at_millis
                {
                    return Err(refuse("retained admission scope"));
                }
                &receipt.credential
            }
            Receipt::Current(receipt) => {
                let request = binding
                    .recovery_request
                    .as_ref()
                    .ok_or(refuse("missing recovery request"))?;
                if request.signature.len() != 64
                    || ![
                        receipt.producer_host.as_str(),
                        receipt.producer_boot.as_str(),
                    ]
                    .into_iter()
                    .all(id)
                {
                    return Err(refuse("recovery transcript extent"));
                }
                receipt.verify(expected.root, request).map_err(refuse)?;
                if binding.schema != "conduit.environment/joined-recorder@2"
                    || request.body_id != *expected.body
                    || request.host_id != *expected.host
                    || request.boot_id != *expected.boot
                {
                    return Err(refuse("retained current member scope"));
                }
                &receipt.credential
            }
        };
        if binding.invitation.nonce == [0; 32]
            || credential.body_id != *expected.body
            || credential.host_id != *expected.host
            || credential.boot_id != *expected.boot
        {
            return Err(refuse("retained invitation credential scope"));
        }
        if ![
            credential.body_id.as_str(),
            credential.part_id.as_str(),
            credential.host_id.as_str(),
            credential.boot_id.as_str(),
            credential.credential_id.as_str(),
            binding.invitation.invitation_id.as_str(),
        ]
        .into_iter()
        .all(id)
        {
            return Err(refuse("retained identity extent"));
        }
        Ok(Self(binding))
    }
    pub fn reopen(path: &Path, expected: &ExpectedMember<'_>) -> Result<Self, Refusal> {
        let parent = path
            .parent()
            .ok_or(Refusal::Scope("member binding parent"))?;
        let directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(parent)?;
        private(&directory, true)?;
        let name = CString::new(
            path.file_name()
                .ok_or(Refusal::Scope("member binding filename"))?
                .as_bytes(),
        )
        .map_err(|_| Refusal::Scope("member binding filename"))?;
        // SAFETY: retained owned directory descriptor and NUL-terminated basename;
        // a successful descriptor is transferred exactly once to File ownership.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        private(&file, false)?;
        let mut bytes = Vec::new();
        file.take((MAXIMUM_BINDING_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAXIMUM_BINDING_BYTES {
            return Err(Refusal::Scope("member binding capacity"));
        }
        Self::verify(serde_json::from_slice(&bytes)?, expected)
    }
}
fn private(file: &File, directory: bool) -> Result<(), Refusal> {
    let m = file.metadata()?;
    // SAFETY: reads effective process identity without changing state.
    if m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || if directory { !m.is_dir() } else { !m.is_file() }
    {
        return Err(Refusal::Scope(
            "member binding requires private owned files/directories",
        ));
    }
    Ok(())
}
