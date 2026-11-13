//! Host-side checked protocol compilation shared by MCU fabrication targets.
//! No GPIO, sensor result, runtime readiness, Host identity or grant is invented.
use conduit_plot as p;
pub mod clock;
pub mod composite;
pub mod fore_carrier;
pub mod image_input;
pub mod native_build;
pub mod native_calibration;
pub mod protocol_catalog;
pub mod recording;
pub mod renderer;
pub mod software;
pub mod source_composition;
pub mod template;
pub mod template_calibration;
pub mod value_profiles;
pub mod workset;
/// Maximum measured private computation fuel of this exact authored closure.
/// The emitted-code test prepares every actual Back and verifies this ceiling.
/// Scheduling/commit fuel remains separately accounted by the resident provider.
pub const DHT11_TM1637_PRIVATE_WORK_UNITS: u16 = 1113;
/// Bounded host-side resident compiler envelope, independent of MCU queue RAM.
pub const MAXIMUM_RESIDENT_PROGRAM_BYTES: u32 = 512 * 1024;
pub const DHT11_TM1637_ENTRIES: &[&str] = &[
    "dht11-capture-request",
    "dht11-pulse-frame",
    "dht11-decode",
    "tm1637-packets",
];
/// These are the ordinary checked authored files. The existing native pulse
/// contract installs only its typed Fore while Source retains protocol meaning.
pub fn checked_dht11_tm1637_source() -> Result<p::CheckedSyntaxDocument, String> {
    let mut startup = p::StartupCatalog::new();
    let mut profiles = p::ProfileCatalog::new();
    conduit_pulse_capture::PulseReadContract::prepare()
        .map_err(|e| format!("pulse contract: {e:?}"))?
        .install(&mut startup, &mut profiles)?;
    let source = [
        include_str!("../../../../plots/device-protocols/pulse-types.conduit"),
        include_str!("../../../../plots/device-protocols/dht11-capture.conduit"),
        include_str!("../../../../plots/device-protocols/dht11.conduit"),
        include_str!("../../../../plots/device-protocols/dht11-read.conduit"),
        include_str!("../../../../plots/device-protocols/tm1637.conduit"),
    ]
    .join("\n");
    let syntax = p::parse_syntax_document(&source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("protocol Source: {:?}", syntax.diagnostics));
    }
    p::check_syntax_document(&syntax, &startup)
        .map_err(|e| format!("checked protocol Source: {e:?}"))
}
pub fn generate_dht11_tm1637_inventory(
    bounds: conduit_embedded_build::ResidentExpressionBounds,
    maximum_generated_bytes: u32,
) -> Result<String, String> {
    let checked = checked_dht11_tm1637_source()?;
    conduit_embedded_build::generate_source_resident_library(
        &checked,
        DHT11_TM1637_ENTRIES,
        bounds,
        maximum_generated_bytes,
    )
}
/// Emit canonical selector material from an already checked expansion. Runtime
/// prepares each selected pure implementation; it need not retain a rich graph.
/// Indices/order are preserved for the caller's exact Source identity mapping.
pub fn selector_material(
    source: &p::ExpandedCanonicalPlot,
) -> Result<Vec<(conduit_core::GearId, Vec<u8>)>, String> {
    source
        .validate_expansion()
        .map_err(|e| format!("expansion: {e:?}"))?;
    let mut material = Vec::new();
    for gear in &source.gears {
        if let Some(configuration) = gear
            .configuration
            .iter()
            .find(|entry| entry.key == "selector")
        {
            let conduit_core::ConfigurationValue::Text(text) = &configuration.value else {
                return Err("selector is not canonical text".into());
            };
            let selector = conduit_core::StructuredSelector::from_canonical_hex(text)
                .map_err(|e| format!("checked selector: {e:?}"))?;
            material.push((
                gear.gear_id.clone(),
                selector
                    .canonical_bytes()
                    .map_err(|e| format!("selector material: {e:?}"))?,
            ));
        }
    }
    Ok(material)
}

/// Reviewed implementation-generation envelope; execution fuel is independently
/// admitted through the original private work resource.
pub fn dht11_tm1637_resident_bounds() -> conduit_embedded_build::ResidentExpressionBounds {
    resident_expression_bounds(DHT11_TM1637_PRIVATE_WORK_UNITS)
}
/// Shared static compilation envelope. The caller supplies the explicit private
/// work requirement; this construction metadata grants no execution work.
pub fn resident_expression_bounds(
    private_work_units: u16,
) -> conduit_embedded_build::ResidentExpressionBounds {
    conduit_embedded_build::ResidentExpressionBounds {
        native: p::rust_binding::PreparedNativeBounds {
            canonical_bytes: 4096,
            scopes: 64,
            invariants: 64,
            contracts: 64,
        },
        maximum_occurrences: 4096,
        maximum_program_bytes: MAXIMUM_RESIDENT_PROGRAM_BYTES,
        maximum_generated_bytes: 1024 * 1024,
        private_work_units,
    }
}

#[cfg(test)]
#[path = "../../../../architecture/plot/tests/common/compiled_rust.rs"]
mod compiled_rust;
#[cfg(test)]
mod tests {
    #[test]
    fn real_sampling_source_emits_exact_finite_software_factories_without_native_effect_offers() {
        let source = conduit_environment_runtime::construction::compile_default().unwrap();
        let material = super::software::compile(&source.expanded, 24 * 1024).unwrap();
        assert_eq!(material.node_count, source.expanded.gears.len());
        assert_eq!(material.cord_count, source.expanded.connections.len());
        let clock = material
            .nodes
            .iter()
            .find(|n| matches!(n.role, super::software::SoftwareRole::Tick { .. }))
            .unwrap();
        assert!(
            clock.offer_image.is_none(),
            "actual native timer must publish its own readiness"
        );
        assert!(material
            .nodes
            .iter()
            .all(|n| !["machine/pulse/read", "machine/tm1637/write"].contains(&n.kind.as_str())));
        let first = super::software::emit_rust(&source.expanded, 24 * 1024, 1024 * 1024).unwrap();
        assert_eq!(
            first,
            super::software::emit_rust(&source.expanded, 24 * 1024, 1024 * 1024).unwrap()
        );
        assert!(first.contains("missing-clock-owner"));
        let mapping = (0..material.node_count)
            .rev()
            .map(|n| u16::try_from(n).unwrap())
            .collect::<Vec<_>>();
        let remapped = super::software::emit_rust_for_layout(
            &source.expanded,
            &mapping,
            24 * 1024,
            1024 * 1024,
        )
        .unwrap();
        assert_ne!(first, remapped);
        assert!(remapped.contains(&format!(
            "result[{}]=Some(D::Tick",
            mapping[usize::from(clock.node)]
        )));
        assert!(super::software::emit_rust_for_layout(
            &source.expanded,
            &vec![0; material.node_count],
            24 * 1024,
            1024 * 1024
        )
        .is_err());

        assert!(super::software::emit_rust(&source.expanded, 24 * 1024, first.len() - 1).is_err());
    }
    #[test]
    fn authored_dht_and_display_entries_emit_one_resident_inventory() {
        let generated = super::generate_dht11_tm1637_inventory(
            conduit_embedded_build::ResidentExpressionBounds {
                native: conduit_plot::rust_binding::PreparedNativeBounds {
                    canonical_bytes: 4096,
                    scopes: 64,
                    invariants: 64,
                    contracts: 64,
                },
                maximum_occurrences: 4096,
                maximum_program_bytes: crate::MAXIMUM_RESIDENT_PROGRAM_BYTES,
                maximum_generated_bytes: 1024 * 1024,
                private_work_units: super::DHT11_TM1637_PRIVATE_WORK_UNITS,
            },
            2_000_000,
        )
        .expect("ordinary authored protocols must lower");
        assert!(generated.contains("PreparedResidentLoans"));
        assert!(generated.contains("ENTRY_COUNT"));
        crate::compiled_rust::verify_with_libraries(
            &format!("mod image{{{generated}}}"),
            r#"fn main(){
                for (entry,fuel) in [
                    (0,image::entry_0::prepare_back().unwrap().computation_fuel()),
                    (1,image::entry_1::prepare_back().unwrap().computation_fuel()),
                    (2,image::entry_2::prepare_back().unwrap().computation_fuel()),
                    (3,image::entry_3::prepare_back().unwrap().computation_fuel())] {
                    println!("resident_entry={entry} computation_fuel={fuel}");
                }
                let mut owner=image::Initialized::prepare().unwrap();
                assert_eq!(owner.offers().iter().flatten().count(),4);
                {let ticket=owner.prepare_selected_array(core::array::from_fn(|_|None)).unwrap();
                 assert!(ticket.nodes().iter().all(Option::is_none));
                 let split=ticket.split();assert!(split.iter().all(Option::is_none));}
                assert_eq!(owner.offers().iter().flatten().count(),4);
                println!("resident_inventory_static_bytes={}",core::mem::size_of::<image::Initialized>());
            }"#,
            "mcu-complete-resident-inventory",
            &[
                "conduit_core",
                "conduit_plot",
                "conduit_kernel",
                "conduit_plan_lowering",
                "conduit_semantic_catalog",
            ],
        );
    }
}
