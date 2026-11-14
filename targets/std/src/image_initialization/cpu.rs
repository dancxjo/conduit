//! Native CPU-only operations selected before the one IMAGE registry/ledger.
//! This provider entry confers no hardware or operation authority.
use super::*;
use conduit_host_hosted::{CPU_BASE_KIND, CPU_IMPLEMENTATION, CheckedCpuInitialization};
pub(crate) struct InitialCpu {
    pub(super) entry: BaseProviderEntry,
}
pub(crate) fn prepare(
    config: &StdHostConfig,
    image: &HostImage,
    checked: &conduit_host_make::CheckedHostConfiguration,
) -> Result<Option<InitialCpu>, String> {
    if !checked
        .configuration()
        .bases
        .iter()
        .any(|base| base.kind == CPU_BASE_KIND)
    {
        return Ok(None);
    }
    let parameters = CheckedCpuInitialization::from_configuration(checked)?;
    if !cfg!(target_os = "linux")
        || usize::BITS != 64
        || checked.configuration().target.architecture != std::env::consts::ARCH
    {
        return Err(
            "selected CPU IMAGE differs from actual native 64-bit Linux architecture".into(),
        );
    }
    if image.manifest.source_configuration_id.as_deref()
        != Some(parameters.configuration_id.as_str())
    {
        return Err("selected CPU parameters differ from IMAGE".into());
    }
    let mut bases = image
        .manifest
        .base_selections
        .iter()
        .filter(|base| base.kind == CPU_BASE_KIND);
    let base = bases.next().ok_or("IMAGE lacks selected CPU Base")?;
    if bases.next().is_some() || base.driver != CPU_IMPLEMENTATION {
        return Err("IMAGE CPU Base differs from installed native provider".into());
    }
    let budgets = image
        .manifest
        .resource_budgets
        .iter()
        .filter(|budget| budget.id == parameters.memory_pool)
        .collect::<Vec<_>>();
    if budgets.len() != 1
        || budgets[0].class != RUNTIME_MEMORY_RESOURCE_CLASS
        || budgets[0].slots != parameters.memory_bytes
        || budgets[0].bytes != u64::from(parameters.memory_bytes)
    {
        return Err("IMAGE CPU budget differs from checked initialization".into());
    }
    let mut capabilities = Vec::new();
    let mut workspace = 0u32;
    for offer in conduit_std_offers::measurement_chart_cpu_offers()? {
        let selected = checked.profile().capabilities.iter().any(|selection| {
            selection.kind == offer.kind_id.as_str()
                && selection.contract_revision == offer.kind_contract_revision.as_str()
                && selection.implementation == offer.implementation.implementation_id.as_str()
        });
        if selected
            && image.manifest.implementations.iter().any(|implementation| {
                implementation == offer.implementation.implementation_id.as_str()
            })
        {
            for requirement in &offer.resource_requirements {
                if requirement.class_id.as_str() != RUNTIME_MEMORY_RESOURCE_CLASS {
                    return Err("CPU profile requests unsupported resource class".into());
                }
                workspace = workspace
                    .checked_add(requirement.units)
                    .ok_or("selected CPU workspace overflow")?;
            }
            capabilities.push(offer);
        }
    }
    if capabilities.is_empty() || workspace > parameters.memory_bytes {
        return Err(
            "selected CPU requires authored operations and sufficient explicit profile workspace"
                .into(),
        );
    }
    Ok(Some(InitialCpu {
        entry: BaseProviderEntry {
            base_id: base.id.as_str().into(),
            provider_instance_id: format!(
                "{}/{}/cpu64",
                config.host_id.as_str(),
                config.boot_id.as_str()
            )
            .into(),
            provider_generation: config.offer_generation.0,
            implementation_id: CPU_IMPLEMENTATION.into(),
            mechanism_family: CPU_BASE_KIND.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities,
            resources: vec![resource_offer(
                &parameters.memory_pool,
                RUNTIME_MEMORY_RESOURCE_CLASS,
                parameters.memory_bytes,
            )],
        },
    }))
}
impl StdHost {
    /// Construct only the checked native CPU operations. Exact Text residence
    /// requires the separate factory's actual affine initialization. Runtime
    /// profile workspace and escaped IMAGE result custody
    /// remain independent gates; no declaration increases either capacity.
    pub fn from_selected_timer_and_cpu_image(
        config: StdHostConfig,
        image: &HostImage,
        catalog: &MakeCatalog,
        checked: &conduit_host_make::CheckedHostConfiguration,
    ) -> Result<Self, String> {
        conduit_host_make::verify_configuration_image(checked, image, catalog)?;
        let cpu =
            prepare(&config, image, checked)?.ok_or("checked Host has no CPU initialization")?;
        Self::from_selected_initial_bases(
            config,
            image,
            catalog,
            InitialBases {
                cpu: Some(cpu),
                ..InitialBases::default()
            },
        )
    }
}

#[cfg(all(test, target_os = "linux", target_pointer_width = "64"))]
pub(crate) fn test_fixture(heap_bytes: u64) -> StdHost {
    let source = include_str!("../../profiles/raspberry-pi-5-measurement-cpu.host.conduit");
    let mut configuration = conduit_host_make::parse_host_configuration_conduit(source).unwrap();
    if std::env::consts::ARCH == "x86_64" {
        configuration.target = conduit_host_make::ConfigurationTarget {
            architecture: "x86_64".into(),
            machine: "computer".into(),
            board: None,
            os: Some("linux".into()),
            make_descriptor: None,
        };
    }
    // Select an explicit finite pool before checking the fixture IMAGE. The
    // deployed Pi5 example keeps its separately authored 2MiB capacity.
    let workspace = conduit_std_offers::measurement_plot_workspace_bytes()
        .checked_add(conduit_std_offers::measurement_chart_face_workspace_bytes())
        .unwrap();
    configuration
        .bases
        .iter_mut()
        .find(|base| base.kind == CPU_BASE_KIND)
        .unwrap()
        .initialization
        .as_mut()
        .unwrap()
        .values
        .insert("memory_bytes".into(), serde_json::json!(workspace));
    let budget = configuration
        .resources
        .iter_mut()
        .find(|budget| budget.id == "memory/measurement")
        .unwrap();
    budget.slots = workspace;
    budget.bytes = u64::from(workspace);
    configuration.limits.heap_arena_bytes = heap_bytes;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked =
        conduit_host_make::check_host_configuration(configuration, &catalog, &packages).unwrap();
    let image = conduit_host_make::build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &conduit_host_make::BuildInputs {
            source_identity: "fixture/selected-cpu".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    StdHost::from_selected_timer_and_cpu_image(
        StdHostConfig {
            host_id: "fixture/cpu".into(),
            boot_id: "boot/cpu".into(),
            offer_generation: OfferGeneration(1),
        },
        &image,
        &catalog,
        &checked,
    )
    .unwrap()
}
#[cfg(all(test, target_os = "linux", target_pointer_width = "64"))]
mod tests {
    use super::*;
    #[test]
    fn explicit_cpu_image_admits_only_authored_real_profiles() {
        let host = test_fixture(64 * 1024 * 1024);
        let ad = host.advertisement.clone();
        assert_eq!(ad.capabilities.len(), 3);
        assert_eq!(ad.resources.len(), 2);
        assert_eq!(ad.bases.len(), 2);
        assert!(
            ad.capabilities
                .iter()
                .any(|offer| offer.implementation.implementation_id.as_str()
                    == conduit_std_offers::MEASUREMENT_PLOT_IMPLEMENTATION)
        );
        assert!(
            ad.capabilities
                .iter()
                .any(|offer| offer.implementation.implementation_id.as_str()
                    == conduit_std_offers::MEASUREMENT_CHART_FACE_IMPLEMENTATION)
        );
        assert!(
            !ad.capabilities
                .iter()
                .any(|offer| offer.kind_id.as_str().starts_with("observation/"))
        );
        assert!(
            host.kernel_resources
                .require_reference_reconfiguration()
                .is_err()
        );
        assert_eq!(host.advertisement, ad);
    }
    #[test]
    fn cross_target_static_check_does_not_depend_on_fabricator_architecture() {
        let source = include_str!("../../profiles/raspberry-pi-5-measurement-cpu.host.conduit");
        let checked = conduit_host_make::check_host_configuration(
            conduit_host_make::parse_host_configuration_conduit(source).unwrap(),
            &conduit_workspace_make::catalog(),
            &conduit_workspace_make::package_set(),
        )
        .unwrap();
        CheckedCpuInitialization::from_configuration(&checked).unwrap();
        let mut configuration = checked.configuration().clone();
        configuration
            .bases
            .iter_mut()
            .find(|base| base.kind == CPU_BASE_KIND)
            .unwrap()
            .initialization
            .as_mut()
            .unwrap()
            .values
            .insert("pointer_bits".into(), serde_json::json!(32));
        assert!(
            conduit_host_make::check_host_configuration(
                configuration,
                &conduit_workspace_make::catalog(),
                &conduit_workspace_make::package_set()
            )
            .is_err()
        );
    }
    #[test]
    fn selected_cpu_refuses_a_pool_below_the_actual_profile_workspace() {
        let source = include_str!("../../profiles/raspberry-pi-5-measurement-cpu.host.conduit")
            .replace("2097152", "1");
        let mut configuration =
            conduit_host_make::parse_host_configuration_conduit(&source).unwrap();
        if std::env::consts::ARCH == "x86_64" {
            configuration.target = conduit_host_make::ConfigurationTarget {
                architecture: "x86_64".into(),
                machine: "computer".into(),
                board: None,
                os: Some("linux".into()),
                make_descriptor: None,
            };
        }
        let catalog = conduit_workspace_make::catalog();
        let packages = conduit_workspace_make::package_set();
        let checked =
            conduit_host_make::check_host_configuration(configuration, &catalog, &packages)
                .unwrap();
        let image = conduit_host_make::build_default_host_image(
            checked.profile().clone(),
            &catalog,
            &packages,
            &conduit_host_make::BuildInputs {
                source_identity: "fixture/insufficient-cpu".into(),
                toolchain_available: true,
            },
        )
        .unwrap()
        .0;
        let error = StdHost::from_selected_timer_and_cpu_image(
            StdHostConfig {
                host_id: "fixture/cpu".into(),
                boot_id: "boot/cpu".into(),
                offer_generation: OfferGeneration(1),
            },
            &image,
            &catalog,
            &checked,
        )
        .err()
        .unwrap();
        assert!(error.contains("profile workspace"), "{error}");
    }
}
