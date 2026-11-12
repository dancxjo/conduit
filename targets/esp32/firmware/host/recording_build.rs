//! Checked recorder construction and Source agreement, never runtime custody.
use conduit_core::{ArtifactId, ImplementationId};
use conduit_host_make::CheckedHostConfiguration;
use conduit_mcu_source_build as source_build;
use source_build::native_calibration::{NativeDescription, Slot};
const APPEND_IMPLEMENTATION: &str = conduit_host_esp32_make::physical::RECORDING;
const SERVICE_IMPLEMENTATION: &str = conduit_host_esp32_make::physical::RECORD_SERVICE;
#[derive(Clone, Copy)]
pub struct Recording {
    pub residence: Option<source_build::recording::CheckedRecordingConstruction>,
    pub read_operations: u32,
    pub service: bool,
}
impl Recording {
    pub fn checked(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        let service = checked
            .resolved_bases()
            .iter()
            .any(|(kind, implementation)| {
                kind == source_build::recording::BASE_KIND
                    && implementation == SERVICE_IMPLEMENTATION
            });
        if service {
            let selected = source_build::native_build::checked_record_service(
                checked,
                SERVICE_IMPLEMENTATION,
                8192,
                655360,
            )?
            .ok_or("missing checked record-service residence")?;
            Ok(Self {
                residence: Some(selected.residence),
                read_operations: selected.maximum_read_operations,
                service,
            })
        } else {
            Ok(Self {
                residence: source_build::native_build::checked_recording(
                    checked,
                    APPEND_IMPLEMENTATION,
                    8192,
                    655360,
                )?,
                read_operations: 0,
                service,
            })
        }
    }
    pub fn emit_cfg(self) {
        println!("cargo:rustc-check-cfg=cfg(record_service)");
        if self.service {
            println!("cargo:rustc-cfg=record_service");
        }
    }
    /// Only one actual installed recorder Kind may select the one optional image.
    pub fn description<'a>(
        self,
        kinds: impl Iterator<Item = &'a str>,
        artifact: &ArtifactId,
    ) -> Result<Option<NativeDescription<'static>>, String> {
        use conduit_resource_snapshot::{native_append, native_service, service_contract};
        let mut append = 0usize;
        let mut service = 0usize;
        for kind in kinds {
            append += usize::from(kind == native_append::APPEND_KIND);
            service += usize::from(kind == service_contract::SERVICE_KIND);
        }
        if append + service == 0 {
            return if self.service {
                Err("checked record-service Base requires exactly one Source ServiceKind".into())
            } else {
                Ok(None)
            };
        }
        if append + service != 1 {
            return Err(format!("Source has {append} append and {service} service recorder nodes; exactly one is supported"));
        }
        if self.residence.is_none() {
            return Err("Source recorder requires its explicitly checked NOR Base".into());
        }
        if self.service != (service == 1) {
            let base = if self.service {
                "record-service"
            } else {
                "record-append"
            };
            let source = if service == 1 {
                "ServiceKind"
            } else {
                "AppendKind"
            };
            return Err(format!(
                "checked {base} Base cannot realize Source {source}"
            ));
        }
        let (base, pool, implementation) = if self.service {
            (
                "esp32/record-service",
                "pool/esp32/record-service",
                SERVICE_IMPLEMENTATION,
            )
        } else {
            (
                "esp32/record-append",
                "pool/esp32/record-append",
                APPEND_IMPLEMENTATION,
            )
        };
        let implementation_id = ImplementationId::from(implementation);
        let definition = if self.service {
            native_service::capability_offer(artifact, &implementation_id)
                .map_err(|e| format!("finite record-service definition: {e:?}"))?
        } else {
            native_append::capability_offer(artifact, &implementation_id)
                .map_err(|e| format!("finite append definition: {e:?}"))?
        };
        Ok(Some(NativeDescription {
            slot: Slot {
                base,
                pool,
                implementation,
            },
            class: native_append::APPEND_CLASS,
            mechanism: native_append::APPEND_MECHANISM,
            definition,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_make::*;
    use conduit_resource_snapshot::{native_append, native_service, service_contract};
    const SOURCE: &str = include_str!("../../profiles/blue-wroom32.host.conduit");
    fn checked(source: &str) -> Result<CheckedHostConfiguration, String> {
        let packages = MakePackageSet::compose(&[&conduit_host_esp32_make::Esp32MakePackage])
            .map_err(|e| format!("{e:?}"))?;
        let catalog = MakeCatalog::canonical().with_packages(&packages);
        let parsed = parse_host_configuration_conduit(source).map_err(|e| format!("{e:?}"))?;
        check_host_configuration(parsed, &catalog, &packages).map_err(|e| format!("{e:?}"))
    }
    fn service() -> Recording {
        let source = SOURCE
            .replace(APPEND_IMPLEMENTATION, SERVICE_IMPLEMENTATION)
            .replace("slots: 1264", "slots: 1264, read_operations: 8192");
        Recording::checked(&checked(&source).unwrap()).unwrap()
    }
    #[test]
    fn actual_checked_service_selects_one_exact_native_card_and_same_residence() {
        let selected = service();
        assert!(selected.service);
        assert_eq!(selected.read_operations, 8192);
        assert_eq!(selected.residence.unwrap().prefix_bytes, 8192);
        assert_eq!(selected.residence.unwrap().slots, 1264);
        let description = selected
            .description(
                [service_contract::SERVICE_KIND].into_iter(),
                &"test/image".into(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(description.slot.base, "esp32/record-service");
        assert_eq!(description.slot.pool, "pool/esp32/record-service");
        assert_eq!(description.slot.implementation, SERVICE_IMPLEMENTATION);
        assert_eq!(description.class, native_append::APPEND_CLASS);
        assert_eq!(description.mechanism, native_append::APPEND_MECHANISM);
        assert_eq!(
            description.definition.kind_id.as_str(),
            service_contract::SERVICE_KIND
        );
        assert_eq!(
            description.definition.capability_id.as_str(),
            native_service::SERVICE_CAPABILITY
        );
        assert_eq!(description.definition.host_calls.len(), 2);
        assert_eq!(description.definition.resource_requirements.len(), 1);
        assert_eq!(description.definition.resource_requirements[0].units, 1);
    }
    #[test]
    fn legacy_checked_append_and_unused_optional_path_are_preserved() {
        let selected = Recording::checked(&checked(SOURCE).unwrap()).unwrap();
        assert!(!selected.service);
        assert_eq!(selected.read_operations, 0);
        let description = selected
            .description(
                [native_append::APPEND_KIND].into_iter(),
                &"test/image".into(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(description.slot.base, "esp32/record-append");
        assert_eq!(description.slot.implementation, APPEND_IMPLEMENTATION);
        assert_eq!(
            description.definition.capability_id.as_str(),
            native_append::APPEND_CAPABILITY
        );
        assert!(selected
            .description(core::iter::empty(), &"test/image".into())
            .unwrap()
            .is_none());
        let absent = SOURCE
            .lines()
            .filter(|line| !line.contains(source_build::recording::BASE_KIND))
            .collect::<Vec<_>>()
            .join("\n");
        let absent = Recording::checked(&checked(&absent).unwrap()).unwrap();
        assert!(absent.residence.is_none());
        assert!(!absent.service);
        assert_eq!(absent.read_operations, 0);
        assert!(absent
            .description(
                [native_append::APPEND_KIND].into_iter(),
                &"test/image".into()
            )
            .is_err());
    }
    #[test]
    fn mismatched_duplicate_mixed_and_missing_recorder_source_refuse() {
        let service = service();
        let append = Recording::checked(&checked(SOURCE).unwrap()).unwrap();
        let artifact = ArtifactId::from("test/image");
        assert!(service.description(core::iter::empty(), &artifact).is_err());
        assert!(service
            .description([native_append::APPEND_KIND].into_iter(), &artifact)
            .is_err());
        assert!(append
            .description([service_contract::SERVICE_KIND].into_iter(), &artifact)
            .is_err());
        for kinds in [
            [
                service_contract::SERVICE_KIND,
                service_contract::SERVICE_KIND,
            ],
            [native_append::APPEND_KIND, native_append::APPEND_KIND],
            [native_append::APPEND_KIND, service_contract::SERVICE_KIND],
        ] {
            assert!(service.description(kinds.into_iter(), &artifact).is_err());
            assert!(append.description(kinds.into_iter(), &artifact).is_err());
        }
    }
}
