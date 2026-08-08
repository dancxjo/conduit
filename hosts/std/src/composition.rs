//! Explicit composition of optional operation families for the std reference host.
//!
//! A family controls which implementation code contributes offers to this host
//! image. The resulting `HostAdvertisement` remains the only runtime promise.

use crate::installed_std;
use crate::StdHostConfig;
use conduit_core::{
    kind_id, resource_offer, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    HostAdvertisement, HostProfileId, ImplementationId, PlannerCapabilityOffer, PlannerProfileId,
    PROTOCOL_VERSION,
};
use conduit_signal::{
    pulse_contract_revision, pulse_execution_profile, pulse_host_operation_requirements,
    pulse_outputs, pulse_resource_requirements, show_contract_revision, show_execution_profile,
    show_host_operation_requirements, show_inputs, show_resource_requirements,
    signal_resource_offers, PULSE_KIND, SHOW_KIND,
};

/// Compile/composition-time selection of implementation families included in a std host image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdHostComposition {
    pub signal: bool,
    pub time: bool,
    pub text: bool,
    pub state: bool,
    pub files: bool,
}

impl StdHostComposition {
    /// The broad reference composition. This is a host image, not the definition of a host.
    pub const fn reference() -> Self {
        Self {
            signal: true,
            time: true,
            text: true,
            state: true,
            files: true,
        }
    }

    /// A deliberately empty operation composition used to prove that optional families are not
    /// mandatory host-core methods.
    pub const fn minimal() -> Self {
        Self {
            signal: false,
            time: false,
            text: false,
            state: false,
            files: false,
        }
    }

    pub const fn with_signal(mut self) -> Self {
        self.signal = true;
        self
    }

    pub const fn with_time(mut self) -> Self {
        self.time = true;
        self
    }

    pub const fn with_text(mut self) -> Self {
        self.text = true;
        self
    }

    pub const fn with_state(mut self) -> Self {
        self.state = true;
        self
    }

    pub const fn with_files(mut self) -> Self {
        self.files = true;
        self
    }
}

impl Default for StdHostComposition {
    fn default() -> Self {
        Self::reference()
    }
}

pub(super) fn build_advertisement(
    config: StdHostConfig,
    composition: StdHostComposition,
) -> HostAdvertisement {
    let mut capabilities = Vec::new();
    if composition.signal {
        capabilities.extend(signal_offers());
    }
    if composition.time {
        capabilities.extend([
            installed_std::tick_offer(),
            installed_std::every_offer(),
            conduit_std_catalog::tick_presentation_offer(),
        ]);
    }
    if composition.text {
        capabilities.extend([
            conduit_std_catalog::text_literal_offer(),
            conduit_std_catalog::text_upper_offer(),
            conduit_std_catalog::text_join_offer(),
            installed_std::text_offer(),
        ]);
    }
    if composition.state {
        capabilities.extend([
            conduit_std_catalog::state_count_offer(),
            conduit_std_catalog::count_presentation_offer(),
        ]);
    }
    if composition.files {
        capabilities.push(conduit_std_catalog::copy_file_offer());
    }
    #[cfg(test)]
    {
        capabilities.push(installed_std::test_observer_offer());
        capabilities.push(installed_std::test_text_source_offer());
    }
    let mut resources = signal_resource_offers("std/timer", "std/presentation", 16);
    resources.retain(|offer| match offer.pool_id.as_str() {
        "std/timer" => composition.signal || composition.time,
        "std/presentation" => {
            composition.signal || composition.time || composition.text || composition.state
        }
        _ => false,
    });
    if composition.files {
        resources.push(resource_offer(
            "std/protected-file",
            conduit_std_catalog::PROTECTED_FILE_RESOURCE_CLASS,
            2,
        ));
        resources.sort();
    }

    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: config.host_id,
        boot_id: config.boot_id,
        offer_generation: config.offer_generation,
        profile: HostProfileId::from("rust-std"),
        resources,
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from(conduit_planner::FULL_PLANNER_PROFILE),
            limits: conduit_planner::FULL_PLANNER_LIMITS,
        }],
        capabilities,
    }
}

fn signal_offers() -> [CapabilityOffer; 2] {
    [
        CapabilityOffer {
            startup_parameters: conduit_signal::pulse_face_startup_parameters(),
            shorthand: None,
            capability_id: CapabilityId::from("pulse-1"),
            kind_id: kind_id(PULSE_KIND),
            kind_contract_revision: pulse_contract_revision(),
            execution_profile_id: pulse_execution_profile(),
            implementation_id: ImplementationId::from("std/pulse-v1"),
            artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
            inputs: vec![],
            outputs: pulse_outputs(),
            host_operations: pulse_host_operation_requirements(),
            resource_requirements: pulse_resource_requirements(),
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 16,
                max_queue_items: 4,
                max_queue_bytes: 64,
            },
        },
        CapabilityOffer {
            startup_parameters: vec![],
            shorthand: None,
            capability_id: CapabilityId::from("stdout-show-1"),
            kind_id: kind_id(SHOW_KIND),
            kind_contract_revision: show_contract_revision(),
            execution_profile_id: show_execution_profile(),
            implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
            artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
            inputs: show_inputs(),
            outputs: vec![],
            host_operations: show_host_operation_requirements(),
            resource_requirements: show_resource_requirements(),
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 16,
                max_queue_items: 4,
                max_queue_bytes: 64,
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::StdHostComposition;
    use crate::{StdHost, StdHostConfig};
    use conduit_core::{BootId, HostId, OfferGeneration};

    fn host(composition: StdHostComposition) -> StdHost {
        StdHost::new_with_composition(
            StdHostConfig {
                host_id: HostId::from("composition-test"),
                boot_id: BootId::from("composition-boot"),
                offer_generation: OfferGeneration(1),
            },
            composition,
        )
    }

    fn offered(host: &StdHost, kind: &str) -> bool {
        host.advertisement()
            .capabilities
            .iter()
            .any(|offer| offer.kind_id.as_str() == kind)
    }

    #[test]
    fn a_selected_family_contributes_only_its_exact_operation_offers() {
        let host = host(StdHostComposition::minimal().with_text());

        assert!(offered(&host, "text/literal"));
        assert!(offered(&host, "text/upper"));
        assert!(offered(&host, "text/join"));
        assert!(offered(&host, "presentation/text"));
        assert!(!offered(&host, "flow/pulse"));
        assert!(!offered(&host, "time/every"));
        assert!(!offered(&host, "state/count"));
        assert_eq!(host.advertisement().resources.len(), 1);
        assert_eq!(
            host.advertisement().resources[0].pool_id.as_str(),
            "std/presentation"
        );
    }

    #[test]
    fn compiled_families_are_not_ambient_runtime_promises() {
        let minimal = host(StdHostComposition::minimal());
        let reference = host(StdHostComposition::reference());

        for kind in [
            "flow/pulse",
            "presentation/show",
            "time/tick",
            "time/every",
            "text/literal",
            "text/upper",
            "text/join",
            "presentation/text",
            "state/count",
            "presentation/count",
            "file/copy",
        ] {
            assert!(!offered(&minimal, kind), "minimal host offered {kind}");
            assert!(offered(&reference, kind), "reference host omitted {kind}");
        }
        assert!(minimal.advertisement().resources.is_empty());
    }

    #[test]
    fn planner_cannot_obtain_an_unselected_family_from_a_category_prefix() {
        let host = host(StdHostComposition::minimal().with_text());
        let form = conduit_form::parse(
            include_str!("../../../examples/signal-demo.form"),
            &conduit_signal::signal_profile_catalog(),
        )
        .expect("Signal form checks independently of host composition");

        assert!(host.plan_local(&form, None).is_err());
    }

    #[test]
    fn reference_host_browser_and_pico_have_different_exact_offer_sets() {
        let std = host(StdHostComposition::reference());
        let browser = conduit_signal::distributed_browser_sink_advertisement();
        let pico = conduit_signal::pico_local_advertisement();

        let kinds = |advertisement: &conduit_core::HostAdvertisement| {
            advertisement
                .capabilities
                .iter()
                .map(|offer| offer.kind_id.as_str().to_owned())
                .collect::<std::collections::BTreeSet<_>>()
        };

        assert_ne!(kinds(std.advertisement()), kinds(&browser));
        assert_ne!(kinds(std.advertisement()), kinds(&pico));
        assert_ne!(kinds(&browser), kinds(&pico));
    }

    #[test]
    fn reference_host_advertises_every_supported_std_revision_and_no_legacy_revision() {
        let host = host(StdHostComposition::reference());
        let advertised = host
            .advertisement()
            .capabilities
            .iter()
            .filter(|offer| {
                offer
                    .kind_contract_revision
                    .as_str()
                    .starts_with("conduit.std/")
            })
            .cloned()
            .collect::<Vec<_>>();
        let supported = conduit_std_catalog::supported_nucleus_offers();

        assert_eq!(advertised, supported);
    }
}
