//! Fallible descriptive identities and bounded generation labels. These values
//! confer no readiness and are built before transferring actual provider owners.
use alloc::string::String;
use conduit_core as c;
use conduit_network_join_call::installation::NetworkNativeIdentity;
use core::fmt::Write;

#[derive(Clone, Copy)]
pub(crate) enum Role {
    Credentials,
    Station,
}
impl Role {
    fn facts(self) -> [&'static str; 8] {
        match self {
            Self::Credentials => [
                "base/esp32/private-network-input",
                "instance/esp32/private-network-input",
                "pool/esp32/private-network-input",
                "generation/private-network-input/",
                "envelope/esp32/private-network-input",
                "native/esp32-private-network-input-root@1",
                "esp32/private-network-input@1",
                "esp32/private-network-input-base@1",
            ],
            Self::Station => [
                "base/esp32/wifi-station",
                "instance/esp32/wifi-station",
                "pool/esp32/wifi-station",
                "generation/wifi-station/",
                "envelope/esp32/wifi-station",
                "native/esp32-wifi-station-root@1",
                "esp32/wifi-station@1",
                "esp32/wifi-station-base@1",
            ],
        }
    }
}

pub(crate) struct GenerationText {
    bytes: [u8; 80],
    length: usize,
}
impl GenerationText {
    fn new(prefix: &str, generation: u64) -> Result<Self, ()> {
        let mut text = Self {
            bytes: [0; 80],
            length: 0,
        };
        text.write_str(prefix).map_err(|_| ())?;
        write!(&mut text, "{generation}").map_err(|_| ())?;
        Ok(text)
    }
    pub(crate) fn attachment(generation: u64) -> Result<Self, ()> {
        Self::new("attachment/esp32/wifi-station/", generation)
    }
    pub(crate) fn as_str(&self) -> &str {
        // All writes copy complete UTF-8 strings; only these initialized bytes
        // are exposed and every write checks capacity before modifying length.
        core::str::from_utf8(&self.bytes[..self.length]).expect("complete UTF-8 writes")
    }
}
impl Write for GenerationText {
    fn write_str(&mut self, value: &str) -> core::fmt::Result {
        let end = self
            .length
            .checked_add(value.len())
            .ok_or(core::fmt::Error)?;
        let destination = self
            .bytes
            .get_mut(self.length..end)
            .ok_or(core::fmt::Error)?;
        destination.copy_from_slice(value.as_bytes());
        self.length = end;
        Ok(())
    }
}

pub(crate) fn prepare(
    role: Role,
    host: &c::HostId,
    boot: &c::BootId,
    generation: u64,
) -> Result<NetworkNativeIdentity, ()> {
    prepare_with(role, host, boot, generation, copy_text)
}

fn copy_text(value: &str) -> Result<String, ()> {
    let mut owned = String::new();
    owned.try_reserve_exact(value.len()).map_err(|_| ())?;
    owned.push_str(value);
    Ok(owned)
}

fn prepare_with(
    role: Role,
    host: &c::HostId,
    boot: &c::BootId,
    generation: u64,
    mut copy: impl FnMut(&str) -> Result<String, ()>,
) -> Result<NetworkNativeIdentity, ()> {
    let [base, instance, resource, prefix, envelope, artifact, implementation, base_implementation] =
        role.facts();
    let label = GenerationText::new(prefix, generation)?;
    Ok(NetworkNativeIdentity {
        host: c::HostId(copy(host.as_str())?),
        boot: c::BootId(copy(boot.as_str())?),
        base: c::HostBaseId(copy(base)?),
        instance: c::BaseInstanceId(copy(instance)?),
        provider_generation: generation,
        resource: c::ResourcePoolId(copy(resource)?),
        resource_generation: c::ResourceGenerationId(copy(label.as_str())?),
        envelope: c::CapabilityEnvelopeId(copy(envelope)?),
        artifact: c::ArtifactId(copy(artifact)?),
        implementation: c::ImplementationId(copy(implementation)?),
        base_implementation: c::BaseImplementationId(copy(base_implementation)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_allocation_null_at_each_identity_field_releases_preceding_strings() {
        let host = c::HostId::from("host/温室");
        let boot = c::BootId::from("boot/é");
        for role in [Role::Credentials, Role::Station] {
            for refused_field in 0..10 {
                let mut field = 0;
                let (result, observed) = crate::allocation_probe::observe(|| {
                    prepare_with(role, &host, &boot, u64::MAX, |value| {
                        let refusal = (field == refused_field).then(|| {
                            crate::allocation_probe::refuse_next_allocation_size(value.len())
                        });
                        field += 1;
                        let result = copy_text(value);
                        drop(refusal);
                        result
                    })
                });
                assert!(result.is_err());
                assert_eq!(field, refused_field + 1);
                assert_eq!(observed.retained_bytes(), 0);
                assert_eq!(observed.allocations, observed.deallocations);
            }
        }
    }
    #[test]
    fn owned_identity_and_attachment_preserve_exact_legacy_transcripts() {
        let host = c::HostId::from("host/温室");
        let boot = c::BootId::from("boot/é");
        for (role, name) in [
            (Role::Credentials, "private-network-input"),
            (Role::Station, "wifi-station"),
        ] {
            for generation in [0, 1, u64::MAX] {
                let actual = prepare(role, &host, &boot, generation).unwrap();
                let expected = NetworkNativeIdentity {
                    host: host.clone(),
                    boot: boot.clone(),
                    base: format!("base/esp32/{name}").into(),
                    instance: format!("instance/esp32/{name}").into(),
                    provider_generation: generation,
                    resource: format!("pool/esp32/{name}").into(),
                    resource_generation: c::ResourceGenerationId(format!(
                        "generation/{name}/{generation}"
                    )),
                    envelope: format!("envelope/esp32/{name}").into(),
                    artifact: format!("native/esp32-{name}-root@1").into(),
                    implementation: format!("esp32/{name}@1").into(),
                    base_implementation: format!("esp32/{name}-base@1").into(),
                };
                assert_eq!(actual, expected);
                let (attachment, observed) = crate::allocation_probe::observe(|| {
                    GenerationText::attachment(generation).unwrap()
                });
                assert_eq!(
                    attachment.as_str(),
                    format!("attachment/esp32/wifi-station/{generation}")
                );
                assert_eq!((observed.allocations, observed.reallocations), (0, 0));
            }
        }
    }
}
