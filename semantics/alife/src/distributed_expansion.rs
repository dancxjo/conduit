//! Canonical recursive realization of portable Lenia across three workers.

use alloc::{format, string::ToString};
use conduit_form::{
    check_syntax_document, expand_canonical_form_with_backs, parse_syntax_document,
    CanonicalBackCatalog, ExpandedCanonicalForm, ProfileCatalog, StartupCatalog,
};

use crate::{
    install_distributed_lenia_catalogs, install_lenia_catalogs, LENIA_JOIN_KIND,
    LENIA_PARTITION_KIND, LENIA_REGION_RESULT_INFO_ID, LENIA_REGION_STEP_KIND,
    LENIA_REGION_WORK_INFO_ID, LENIA_STEP_KIND, SCALAR_FIELD_GRAY8_KIND,
    SCALAR_FIELD_PRESENTATION_KIND,
};

pub fn expanded_three_region_lenia() -> Result<ExpandedCanonicalForm, alloc::string::String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_lenia_catalogs(&mut startup, &mut profile)?;
    conduit_time::install_time_every_catalog(&mut startup, &mut profile)?;
    conduit_presentation::install_bitmap_presentation_catalog(&mut startup, &mut profile)?;
    install_distributed_lenia_catalogs(&mut startup, &mut profile)?;

    let source = include_str!("../../../examples/lenia-orbium.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup)
        .map_err(|diagnostics| format!("portable Lenia source: {diagnostics:?}"))?;
    let back_source = format!(
        "form alife/lenia-step (\n > initial: {}\n > tick: {}...|\n field: {}...| >\n) {{\n partition: {LENIA_PARTITION_KIND}\n region0: {LENIA_REGION_STEP_KIND}\n region1: {LENIA_REGION_STEP_KIND}\n region2: {LENIA_REGION_STEP_KIND}\n join: {LENIA_JOIN_KIND}\n initial > partition.initial\n tick > partition.tick\n partition.work0 > region0.work\n partition.work1 > region1.work\n partition.work2 > region2.work\n region0.result > join.result0\n region1.result > join.result1\n region2.result > join.result2\n join.field > field\n}}\n",
        crate::SCALAR_FIELD2_INFO_ID,
        conduit_time::TICK_VALUE_KIND,
        crate::SCALAR_FIELD2_INFO_ID,
    );
    let back = check_syntax_document(&parse_syntax_document(&back_source), &startup)
        .map_err(|diagnostics| format!("distributed Lenia Back: {diagnostics:?}"))?;
    let definition = profile
        .get(&conduit_core::kind_id(LENIA_STEP_KIND))
        .ok_or_else(|| "Lenia profile lacks step definition".to_string())?
        .clone();
    let mut backs = CanonicalBackCatalog::new();
    backs
        .insert(&definition, &back, LENIA_STEP_KIND)
        .map_err(|error| format!("distributed Lenia Back: {error:?}"))?;
    let presentation_back_source = format!(
        "form presentation/scalar-field (\n > field: {}...|\n) {{\n bitmap: {SCALAR_FIELD_GRAY8_KIND}\n manifest: {}\n field > bitmap.field\n bitmap.bitmap > manifest.bitmap\n}}\n",
        crate::SCALAR_FIELD2_INFO_ID,
        conduit_presentation::BITMAP_PRESENTATION_KIND,
    );
    let presentation_back =
        check_syntax_document(&parse_syntax_document(&presentation_back_source), &startup)
            .map_err(|diagnostics| format!("Lenia bitmap presentation Back: {diagnostics:?}"))?;
    let presentation_definition = profile
        .get(&conduit_core::kind_id(SCALAR_FIELD_PRESENTATION_KIND))
        .ok_or_else(|| "Lenia profile lacks scalar-field presentation definition".to_string())?
        .clone();
    backs
        .insert(
            &presentation_definition,
            &presentation_back,
            SCALAR_FIELD_PRESENTATION_KIND,
        )
        .map_err(|error| format!("Lenia bitmap presentation Back: {error:?}"))?;
    let expanded =
        expand_canonical_form_with_backs(&checked, "lenia-orbium-demo", &profile, &backs)
            .map_err(|error| error.to_string())?;
    if expanded
        .gears
        .iter()
        .filter(|gear| gear.kind_id.as_str() == LENIA_REGION_STEP_KIND)
        .count()
        != 3
        || expanded
            .connections
            .iter()
            .filter(|connection| {
                matches!(
                    connection.value_kind.as_str(),
                    LENIA_REGION_WORK_INFO_ID | LENIA_REGION_RESULT_INFO_ID
                )
            })
            .count()
            != 6
        || expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == SCALAR_FIELD_GRAY8_KIND)
            .count()
            != 1
        || expanded
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == conduit_presentation::BITMAP_PRESENTATION_KIND)
            .count()
            != 1
    {
        return Err(
            "distributed Lenia expansion lacks workers, exact Cords, or bitmap presentation"
                .to_string(),
        );
    }
    expanded
        .validate_expansion()
        .map_err(|error| error.to_string())?;
    Ok(expanded)
}
