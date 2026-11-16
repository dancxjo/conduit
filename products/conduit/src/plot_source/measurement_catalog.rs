//! Finite Linux authoring definitions shared by ordinary and bounded checking.
//! No provider, Host offer, resource grant or executable preparation is installed.
use conduit_core::CheckedValueContract;
use conduit_plot::{ProfileCatalog, StartupCatalog};

fn latest_inputs(count: usize) -> Result<(CheckedValueContract, CheckedValueContract), String> {
    let contract = |arity| {
        let (ty, bytes) = conduit_data::measurement_chart_constructor_input(arity)?;
        CheckedValueContract::new(
            ty.profile()
                .map_err(|_| "measurement tuple profile")?
                .value_kind()
                .clone(),
            bytes,
            vec![],
        )
        .map_err(|_| "measurement tuple contract")
    };
    Ok((
        contract(count - 1).map_err(String::from)?,
        contract(1).map_err(String::from)?,
    ))
}

pub(super) fn install(
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
) -> Result<(), String> {
    // Exact finite family, matching installed Std/CPU capabilities. All entries
    // are semantic definitions; Make still advertises only selected kinds.
    conduit_data::install_measurement_window_catalog(startup, profiles)?;
    let (series, bytes) = conduit_data::measurement_chart_constructor_input(1)?;
    let contract = CheckedValueContract::new(
        series
            .profile()
            .map_err(|_| "measurement series profile")?
            .value_kind()
            .clone(),
        bytes,
        vec![],
    )
    .map_err(|_| "measurement series contract")?;
    // Only this explicitly installed Series profile is available here; the
    // generic semantic constructor does not install arbitrary typed providers.
    conduit_semantic_catalog::install_flow_singleton_kind(&contract, &series, startup, profiles)?;
    for count in 1..=4 {
        conduit_data::install_measurement_chart_constructor(count, startup, profiles)?;
    }
    conduit_data::install_measurement_chart_flow_catalog(startup, profiles)?;
    for count in 2..=4 {
        let (left, right) = latest_inputs(count)?;
        conduit_semantic_catalog::install_combine_latest_schema_kind(
            &left, &right, startup, profiles,
        )?;
        if count <= 3 {
            conduit_semantic_catalog::install_combine_latest_schema_with_input_terminals(
                &left,
                &right,
                if count == 2 {
                    [false, false]
                } else {
                    [true, false]
                },
                startup,
                profiles,
            )?;
        }
    }
    Ok(())
}
