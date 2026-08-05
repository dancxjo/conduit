use super::*;

pub fn parse_placements(source: &str) -> Result<PlacementChoices, PlannerError> {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    if lines.first().copied().unwrap_or("") != "placements 0" {
        return Err(PlannerError::InvalidPlacementSyntax(
            "expected first non-comment line to be 'placements 0'".to_string(),
        ));
    }

    let mut index = 1usize;
    let mut by_operation = BTreeMap::new();
    while index < lines.len() {
        let header = lines[index];
        let operation_name = header
            .strip_suffix(':')
            .ok_or_else(|| PlannerError::InvalidPlacementSyntax(header.to_string()))?;
        if by_operation.contains_key(&OperationId::from(operation_name)) {
            return Err(PlannerError::DuplicatePlacement(operation_name.to_string()));
        }
        let host_line = lines
            .get(index + 1)
            .ok_or_else(|| PlannerError::InvalidPlacementSyntax(operation_name.to_string()))?;
        let capability_line = lines
            .get(index + 2)
            .ok_or_else(|| PlannerError::InvalidPlacementSyntax(operation_name.to_string()))?;
        let host_id = parse_assignment(host_line, "host")?;
        let capability_id = parse_assignment(capability_line, "capability")?;
        by_operation.insert(
            OperationId::from(operation_name),
            PlacementChoice {
                host_id: HostId::from(host_id),
                capability_id: CapabilityId::from(capability_id),
            },
        );
        index += 3;
    }

    Ok(PlacementChoices { by_operation })
}

fn parse_assignment<'a>(line: &'a str, key: &str) -> Result<&'a str, PlannerError> {
    let (lhs, rhs) = line
        .split_once('=')
        .ok_or_else(|| PlannerError::InvalidPlacementSyntax(line.to_string()))?;
    if lhs.trim() != key {
        return Err(PlannerError::InvalidPlacementSyntax(line.to_string()));
    }
    let value = rhs.trim().trim_matches('"');
    if value.is_empty() {
        return Err(PlannerError::InvalidPlacementSyntax(line.to_string()));
    }
    Ok(value)
}

pub fn default_placements(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    let host = realm
        .first()
        .ok_or_else(|| PlannerError::UnknownHost("realm is empty".to_string()))?;
    let mut by_operation = BTreeMap::new();
    for operation in &form.operations {
        let offer = host
            .capabilities
            .iter()
            .find(|offer| {
                offer.kind_id == operation.kind_id
                    && offer.kind_contract_revision == operation.kind_contract_revision
                    && offer.inputs == operation.inputs
                    && offer.outputs == operation.outputs
            })
            .ok_or_else(|| {
                PlannerError::UnknownCapability(operation.kind_id.as_str().to_string())
            })?;
        by_operation.insert(
            operation.operation_id.clone(),
            PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: offer.capability_id.clone(),
            },
        );
    }
    Ok(PlacementChoices { by_operation })
}

pub fn plan(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
) -> Result<Plan, PlannerError> {
    plan_with_connection_limits(
        form,
        realm,
        placements,
        providers,
        DEFAULT_CONNECTION_ITEM_CAPACITY,
        DEFAULT_CONNECTION_BYTE_CAPACITY,
    )
}

pub fn plan_with_authority_grants(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    authority_grants: &[AuthorityGrant],
) -> Result<Plan, PlannerError> {
    plan_with_options(
        form,
        realm,
        placements,
        providers,
        PlanningOptions {
            connection_providers: &BTreeMap::new(),
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants,
            link_bindings: &[],
        },
    )
}

pub fn plan_with_link_bindings(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
    link_bindings: &[LinkBinding],
) -> Result<Plan, PlannerError> {
    plan_with_options(
        form,
        realm,
        placements,
        providers,
        PlanningOptions {
            connection_providers: &BTreeMap::new(),
            connection_item_capacity,
            connection_byte_capacity,
            authority_grants: &[],
            link_bindings,
        },
    )
}

pub fn plan_with_connection_limits(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<Plan, PlannerError> {
    plan_with_connection_limits_and_provider_overrides(
        form,
        realm,
        placements,
        providers,
        &BTreeMap::new(),
        connection_item_capacity,
        connection_byte_capacity,
    )
}

pub fn plan_with_connection_limits_and_provider_overrides(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    connection_providers: &BTreeMap<(OperationId, OperationId), ConnectionProvider>,
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<Plan, PlannerError> {
    plan_with_options(
        form,
        realm,
        placements,
        providers,
        PlanningOptions {
            connection_providers,
            connection_item_capacity,
            connection_byte_capacity,
            authority_grants: &[],
            link_bindings: &[],
        },
    )
}
