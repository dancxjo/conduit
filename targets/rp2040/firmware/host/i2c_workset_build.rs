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
    let Some(mut source) = workset::from_environment(None)? else {
        return Ok(None);
    };
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
    let resident_count = source
        .entries
        .iter()
        .flat_map(|entry| &entry.plot.expanded.gears)
        .filter(|gear| gear.configuration.iter().any(|v| v.key == "program"))
        .count();
    // Uniform construction requirements stay within the original shared pool,
    // including the resident commit allowance; actual code cost may refuse.
    let per_resident = if resident_count == 0 {
        4
    } else {
        work.capacity_units
            .checked_div(u32::try_from(resident_count).map_err(|_| "resident count width")?)
            .ok_or("resident work division")?
    };
    let private_work = per_resident
        .checked_sub(3)
        .filter(|n| *n != 0)
        .ok_or("original work pool cannot fund resident requirements")?;
    let mut resident_bounds = conduit_host_rp2040::source_build::resident_expression_bounds(
        u16::try_from(private_work.min(u32::from(u16::MAX - 3)))
            .map_err(|_| "private work width")?,
    );
    resident_bounds.native.canonical_bytes =
        usize::try_from(checked.configuration().limits.buffered_bytes)
            .map_err(|_| "canonical buffer width")?;
    let resident = source.prepare_selected_residents(resident_bounds)?;
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
    native_build::calibrate_and_emit_neutral(
        &mut source,
        None,
        out,
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
    Ok(Some(resident))
}
