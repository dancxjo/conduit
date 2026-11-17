//! Equipment-neutral checked Source fabrication for the actual configured I2C
//! controller and one calibrated clock description. No provider is initialized,
//! no diagnostics are substituted, and no Root execution is installed here.
use conduit_host_rp2040::source_build::{
    image_input, native_build, native_calibration as calibration, workset,
};
use std::path::Path;

pub fn prepare(
    checked: &conduit_host_make::CheckedHostConfiguration,
    catalog: &conduit_host_make::MakeCatalog,
    image_id: &str,
    source_identity: &str,
    out: &Path,
) -> Result<Option<Vec<conduit_embedded_build::PreparedResidentExpression>>, String> {
    with_inputs(
        checked,
        catalog,
        image_id,
        source_identity,
        out,
        None,
        |source, input, physical| {
            native_build::calibrate_and_emit_neutral(source, None, out, input, physical)
        },
    )
    .map(|result| result.map(|(_, residents)| residents))
}

/// Host-only schema inspection. This creates no assigned document or qualified
/// artifact: HOST_TOOLING limits bound metadata generation, not target resources.
#[allow(dead_code)] // Used by host-only inspection; ordinary firmware preparation stays Owned.
pub fn inspect_schema(
    checked: &conduit_host_make::CheckedHostConfiguration,
    catalog: &conduit_host_make::MakeCatalog,
    image_id: &str,
    source_identity: &str,
    out: &Path,
) -> Result<Option<Vec<conduit_host_rp2040::source_build::template_calibration::InertEntry>>, String>
{
    with_inputs(
        checked,
        catalog,
        image_id,
        source_identity,
        out,
        Some(conduit_embedded_build::ResidentExecutionStorage::SharedWorkspaceSchema),
        |source, mut input, physical| {
            input.bounds.image = conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING;
            calibration::inspect_with_physical_bases(source, input, physical)
        },
    )
    .map(|result| result.map(|(inspection, _)| inspection))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn with_inputs<T>(
    checked: &conduit_host_make::CheckedHostConfiguration,
    catalog: &conduit_host_make::MakeCatalog,
    image_id: &str,
    source_identity: &str,
    out: &Path,
    inspection_storage: Option<conduit_embedded_build::ResidentExecutionStorage>,
    finish: impl FnOnce(
        &mut workset::Workset,
        calibration::Input<'_>,
        &[calibration::NativeBaseSelection<'_>],
    ) -> Result<T, String>,
) -> Result<Option<(T, Vec<conduit_embedded_build::PreparedResidentExpression>)>, String> {
    let Some(mut source) = workset::from_environment(None)? else {
        return Ok(None);
    };
    let storage = inspection_storage.unwrap_or_else(|| source.resident_execution_storage());
    if inspection_storage.is_none()
        && storage == conduit_embedded_build::ResidentExecutionStorage::SharedWorkspace
        && !cfg!(feature = "execution-workspace")
    {
        return Err(
            "captured shared workspace requires matching target execution-workspace support".into(),
        );
    }
    let image = image_input::from_environment(checked, catalog, image_id, source_identity)?;
    let bus = conduit_host_rp2040::CheckedI2cConstruction::from_configuration(checked)?;
    if bus.maximum_call_operations == 0 || bus.call_address_range.is_none() {
        return Err("selected Source requires explicitly enabled original I2C calls".into());
    }
    let clock = conduit_host_rp2040::CheckedClockConstruction::from_configuration(
        checked,
        conduit_host_rp2040::CLOCK_IMPLEMENTATION,
    )?;
    if clock.slots == 0 {
        return Err("selected Source requires an original clock slot".into());
    }
    let count = |kind: &str| {
        source
            .entries
            .iter()
            .flat_map(|entry| &entry.plot.expanded.gears)
            .filter(|gear| gear.kind_id.as_str() == kind)
            .count()
    };
    if count("machine/i2c/transact") != 1 || count("machine/clock/at") != 1 {
        return Err(
            "this finite construction requires exactly one selected I2C and deadline clock owner"
                .into(),
        );
    }
    let work = conduit_host_rp2040::CheckedResidentWorkPool::from_configuration(checked)?;
    // Each Source-selected owner retains its exact compiler requirement. A
    // shared pool is an aggregate limit, not an equal per-owner allowance.
    // Captured storage is sealed into each original resident artifact.
    // Inspection overrides are host-only and never qualify a target artifact.
    let mut resident_bounds = conduit_host_rp2040::source_build::resident_expression_bounds(
        u16::try_from(work.capacity_units.min(u32::from(u16::MAX - 3)))
            .map_err(|_| "private work width")?,
    );
    resident_bounds.native.canonical_bytes =
        usize::try_from(checked.configuration().limits.buffered_bytes)
            .map_err(|_| "canonical buffer width")?;
    let resident = source.prepare_selected_residents_with_exact_work(
        resident_bounds,
        storage,
        work.capacity_units,
    )?;
    let artifact = conduit_core::ArtifactId::from(image_id);
    let i2c = conduit_host_rp2040::checked_i2c_offer_definition(checked, image_id)?
        .ok_or("I2C definition unselected")?;
    let clock_contract =
        conduit_monotonic_wait::clock_at::contract::MonotonicClockContract::prepare()
            .map_err(|e| format!("clock contract: {e:?}"))?;
    let clock_offer = conduit_monotonic_wait::clock_at::installation::capability_offer(
        &clock_contract,
        &artifact,
    )
    .map_err(|e| format!("clock definition: {e:?}"))?;
    let (base, pool) = match bus.controller {
        0 => ("rp2040/i2c/0", "pool/rp2040-i2c-0"),
        1 => ("rp2040/i2c/1", "pool/rp2040-i2c-1"),
        _ => return Err("checked I2C controller".into()),
    };
    let descriptions = [
        calibration::NativeDescription {
            slot: calibration::Slot {
                base,
                pool,
                implementation: i2c.implementation.implementation_id.as_str(),
            },
            class: "machine/i2c-attachment",
            mechanism: "machine/i2c-attachment",
            definition: i2c.clone(),
        },
        calibration::NativeDescription {
            slot: calibration::Slot {
                base: "rp2040/clock-at/0",
                pool: "pool/rp2040/clock-at/0",
                implementation: clock_offer.implementation.implementation_id.as_str(),
            },
            class: conduit_monotonic_wait::clock_at::installation::CLOCK_RESOURCE_CLASS,
            mechanism: conduit_monotonic_wait::clock_at::installation::CLOCK_RESOURCE_CLASS,
            definition: clock_offer.clone(),
        },
    ];
    let physical = [
        calibration::NativeBaseSelection {
            base,
            implementation: conduit_host_rp2040::I2C_IMPLEMENTATION,
        },
        calibration::NativeBaseSelection {
            base: "rp2040/clock-at/0",
            implementation: conduit_host_rp2040::CLOCK_IMPLEMENTATION,
        },
    ];
    native_build::emit_capability_images(
        out,
        &[("I2C_IMAGE", &i2c), ("CLOCK_AT_IMAGE", &clock_offer)],
        None,
        24 * 1024,
    )?;
    let shared = [conduit_core::resource_offer(
        &work.id,
        conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID,
        work.capacity_units,
    )];
    let bounds = calibration::Bounds::for_workset(
        checked,
        &source,
        conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: resident_bounds.maximum_program_bytes,
            maximum_total_program_bytes: resident_bounds.maximum_program_bytes,
        },
    )?;
    let inspection = finish(
        &mut source,
        calibration::Input {
            checked,
            image: &image,
            catalog,
            shared_resources: &shared,
            native: &descriptions,
            other: &[],
            resident: &resident,
            bounds,
        },
        &physical,
    )?;
    Ok(Some((inspection, resident)))
}
