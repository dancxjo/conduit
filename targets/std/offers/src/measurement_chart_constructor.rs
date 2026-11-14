//! Pure bounded chart construction over the exact existing opaque tuple chain.
//! Preparation and execution allocate finite temporaries outside kernel Step.
use conduit_core::*;
use conduit_data::*;
pub const MEASUREMENT_CHART_CONSTRUCTOR_IMPLEMENTATION: &str =
    "std/measurement-chart-constructor@1";
pub const MEASUREMENT_CHART_CONSTRUCTOR_PROFILE: &str =
    "std/measurement-chart-constructor-hosted@1";
pub const MEASUREMENT_CHART_CONSTRUCTOR_CALL: &str = "conduit.host/measurement-chart-constructor@1";
pub fn measurement_chart_constructor_workspace(count: usize) -> Result<u32, &'static str> {
    let (_, input) = measurement_chart_constructor_input(count)?;
    let mut metadata = core::mem::size_of::<(
        Vec<Vec<u8>>,
        Vec<KindId>,
        Vec<u32>,
        Vec<String>,
        Vec<u8>,
        Vec<u8>,
        usize,
        u32,
    )>() + count
        * (core::mem::size_of::<Vec<u8>>()
            + core::mem::size_of::<KindId>()
            + core::mem::size_of::<u32>()
            + core::mem::size_of::<String>()
            + MAXIMUM_MEASUREMENT_CHART_LABEL_BYTES);
    for stage in 1..=count {
        let (ty, _) = measurement_chart_constructor_input(stage)?;
        metadata += ty.canonical_bytes().map_err(|_| "chart schema")?.len()
            + ty.profile()
                .map_err(|_| "chart profile")?
                .value_kind()
                .as_str()
                .len();
    }
    u32::try_from(
        input as usize
            + 2 * measurement_chart_canonical_bytes()
            + measurement_chart_decode_workspace_bytes()
            + metadata,
    )
    .map_err(|_| "chart workspace")
}

pub fn measurement_chart_constructor_std_offer(
    count: usize,
) -> Result<CapabilityOffer, &'static str> {
    let kind = measurement_chart_constructor_semantic_contract(count)?;
    let (_, input) = measurement_chart_constructor_input(count)?;
    let digest = semantic_digest(
        "std/chart-constructor@1",
        concat!(
            include_str!("measurement_chart_constructor.rs"),
            include_str!("../../src/hosted_measurement_chart_constructor.rs"),
            include_str!("../../src/installed_std/measurement_chart_constructor_back.rs"),
            include_str!("../../../../semantics/data/src/measurement_chart_constructor.rs"),
            include_str!("../../../../semantics/data/src/measurement_chart.rs"),
            include_str!("../../../../semantics/data/src/measurement_chart/wire.rs")
        )
        .as_bytes(),
    );
    let artifact = format!(
        "sha256:{}",
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    Ok(BackOfferBuilder::new(
        kind.clone(),
        Back {
            capability_id: format!("{MEASUREMENT_CHART_CONSTRUCTOR_IMPLEMENTATION}/{count}").into(),
            execution_profile_id: MEASUREMENT_CHART_CONSTRUCTOR_PROFILE.into(),
            implementation_id: format!("{MEASUREMENT_CHART_CONSTRUCTOR_IMPLEMENTATION}/{count}")
                .into(),
            artifact_id: artifact.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: MEASUREMENT_CHART_CONSTRUCTOR_CALL.into(),
                target_kind: Some(kind.kind_id),
                maximum_in_flight: 1,
                maximum_input_bytes: input,
                maximum_output_bytes: measurement_chart_canonical_bytes() as u32,
            }],
            resource_requirements: vec![resource_requirement(
                RUNTIME_MEMORY_RESOURCE_CLASS,
                measurement_chart_constructor_workspace(count)?,
            )],
            authority_requirements: vec![],
        },
    )
    .build())
}

/// Finite standard CPU profile inventory. Callers MUST filter against exact
/// checked Kind/revision/implementation selection before advertising any offer.
pub fn measurement_chart_cpu_offers() -> Result<Vec<CapabilityOffer>, &'static str> {
    let mut offers = vec![
        super::measurement_plot_std_offer(),
        super::measurement_chart_face_std_offer(super::measurement_chart_face_workspace_bytes())?,
        super::measurement_chart_flow_face_std_offer(
            super::measurement_chart_face_workspace_bytes(),
        )?,
    ];
    for count in 1..=4 {
        offers.push(measurement_chart_constructor_std_offer(count)?);
    }
    for count in 2..=4 {
        let (left, lb) = measurement_chart_constructor_input(count - 1)?;
        let (right, rb) = measurement_chart_constructor_input(1)?;
        let left = CheckedValueContract::new(
            left.profile()
                .map_err(|_| "latest profile")?
                .value_kind()
                .clone(),
            lb,
            vec![],
        )
        .map_err(|_| "latest contract")?;
        let right = CheckedValueContract::new(
            right
                .profile()
                .map_err(|_| "series profile")?
                .value_kind()
                .clone(),
            rb,
            vec![],
        )
        .map_err(|_| "series contract")?;
        offers.push(super::combine_latest_schema_offer(&left, &right)?);
        if count <= 3 {
            offers.push(super::combine_latest_schema_offer_with_input_terminals(
                &left,
                &right,
                if count == 2 {
                    [false, false]
                } else {
                    [true, false]
                },
            )?);
        }
    }
    Ok(offers)
}
