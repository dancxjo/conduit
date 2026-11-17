//! Materialization of the same generic lowered Body kernel.
use super::*;
impl<'a> BodyKernel<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_with_live(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        fore_inputs: &'a [ExternalForeInput],
        sequential_fore: bool,
        has_fore_output: bool,
        live: Option<&BodyLiveForeQueue>,
        parsec_sources: Option<&std::sync::Arc<crate::flow_activation::ParsecSourcePlans>>,
        #[cfg(target_os = "linux")] selected_file: Option<
            &'a crate::text_file::SelectedTextFileResidence,
        >,
        funding: Option<&funded::TickPreparationLayout>,
    ) -> Result<Self, String> {
        let mut fragments = Vec::with_capacity(partitions.len());
        for fragment in partitions.iter().map(|partition| {
            partition
                .plan
                .fragments
                .first()
                .filter(|_| partition.plan.fragments.len() == 1)
                .ok_or("local body execution requires one local fragment per Plot")
        }) {
            fragments.push(fragment?);
        }
        // Activation coordinators belong to the whole sealed Plan. Bind each
        // entry to its exact local fragment before lowering ordinary nodes.
        let activations = bind_body_activations(partitions, &fragments)?;
        let mut scans = if parsec_sources.is_some() {
            scan_route::prepare_with_parsec_sources(partitions, parent_play, parsec_sources)?
        } else {
            scan_route::prepare(partitions, parent_play)?
        };
        let mut each = each_route::prepare(partitions, parent_play, fore_inputs.len())?;
        if scans.len() == 1 {
            let conduit_core::PlannedActivationEntry::Scan(planned) =
                &partitions[scans[0].partition].plan.activations[0]
            else {
                return Err("installed scan activation changed during preparation".into());
            };
            if fore_inputs.len() > usize::from(planned.limits.maximum_items) {
                return Err("preloaded Todo commands exceed the selected scan bound".into());
            }
        }
        let supported_pure_activation = scans.len() + each.len() == 1
            && partitions.len() == 1
            && fragments[0].placements.len() == 1
            && !has_keyboard
            && sequential_fore
            && has_fore_output
            && (live.is_some() || fore_inputs.len() <= 256)
            && fragments[0].placements[0].host_calls.is_empty();
        let supported_parsec_input_scan = has_keyboard
            && live.is_some()
            && scans.len() == 1
            && partitions.len() == 1
            && parsec_sources.is_some();
        let plans = partitions
            .iter()
            .map(|partition| &partition.plan)
            .collect::<Vec<_>>();
        let lowered = if let Some(funding) = funding {
            conduit_plan_lowering::fragment_set::lower_local_fragment_set_from_plans_with_limit(
                &plans,
                FIXED_KERNEL_STORAGE_PROFILE,
                funded::bounds(),
                funding.lowering_bytes,
            )
        } else {
            lower_local_fragment_set_from_plans(
                &plans,
                FIXED_KERNEL_STORAGE_PROFILE,
                funded::bounds(),
            )
        }
        .map_err(|error| format!("Body fragment lowering: {error:?}"))?;
        let fore = if let Some(queue) = live {
            BodyForeRoute::prepare_live(&lowered.partitions, queue)?
        } else {
            BodyForeRoute::prepare(
                &lowered.partitions,
                fore_inputs,
                sequential_fore,
                has_fore_output,
            )?
        };
        for operation in lowered.partitions.iter().flat_map(|part| &part.host_calls) {
            if keyboard(&operation.contract_id) || button(&operation.contract_id) {
                if !has_keyboard {
                    return Err("Body keyboard has no admitted adapter".into());
                }
            } else if !scalar_math::supports(operation)
                && !timer(&operation.contract_id)
                && !typed_record_codec(&operation.contract_id)
                && !image_text(&operation.contract_id)
                && !text_state(&operation.contract_id)
                && !data_text::is_file(&operation.contract_id)
                && !input_semantic(&operation.contract_id)
                && !todo_checkpoint(operation)
                && !todo_checkpoint_read(operation)
                && !face(operation)
                && operation.contract_id.as_str()
                    != conduit_body_make::deployment_runtime_catalog::CRECHE_HOST_CALL
            {
                return Err(format!(
                    "Body Host Call is unsupported: {}",
                    operation.contract_id.as_str()
                ));
            }
        }
        let mut items = 0_u16;
        let mut bytes = 0_u32;
        let mut maximum = 1_u32;
        let mut sign_items = 32_u16;
        let mut request_capacity = 0_usize;
        for (partition_index, fragment) in fragments.iter().enumerate() {
            for placement in &fragment.placements {
                let ordinary;
                let budget = if let Some(scan) = scans.iter().find(|scan| {
                    scan.partition == partition_index && scan.owner == placement.placement_id
                }) {
                    &scan.budget
                } else if let Some(each) = each.iter().find(|each| {
                    each.partition == partition_index && each.owner == placement.placement_id
                }) {
                    &each.budget
                } else {
                    // Ordinary Backs are budgeted only after exact activation
                    // owners have been consumed by the whole-Plan scan route.
                    ordinary = super::super::preparation::back_budget(placement)?;
                    &ordinary
                };
                items = items
                    .checked_add(budget.value_items)
                    .ok_or("Body value item overflow")?;
                bytes = bytes
                    .checked_add(budget.value_bytes)
                    .filter(|bytes| *bytes <= 16 * 1024 * 1024)
                    .ok_or("Body value byte capacity exceeded")?;
                maximum = maximum.max(budget.maximum_value_bytes);
                sign_items = sign_items
                    .checked_add(budget.sign_items)
                    .ok_or("Body Sign overflow")?;
                request_capacity = request_capacity
                    .checked_add(budget.host_requests)
                    .ok_or("Body request overflow")?;
            }
        }
        for port in fore.input_ports() {
            items = items
                .checked_add(port.item_capacity)
                .ok_or("Body Fore value item overflow")?;
            bytes = bytes
                .checked_add(port.byte_capacity)
                .filter(|bytes| *bytes <= 16 * 1024 * 1024)
                .ok_or("Body Fore value byte capacity exceeded")?;
            maximum = maximum.max(port.byte_capacity);
        }
        sign_items = sign_items
            .checked_add(fore.sign_items()?)
            .ok_or("Body Fore Sign overflow")?;
        let mut values = if let Some(funding) = funding {
            let mut capacities = Vec::new();
            capacities
                .try_reserve_exact(usize::from(items.max(1)))
                .map_err(|_| "funded value capacity preparation allocation failed")?;
            capacities.resize(usize::from(items.max(1)), maximum);
            HostedValueStore::try_new_with_slot_capacities(
                &capacities,
                bytes.max(1),
                funding.value_bytes,
            )
            .map_err(|error| format!("funded Body value store: {error:?}"))?
        } else {
            HostedValueStore::new(items.max(1), maximum, bytes.max(1))
                .map_err(|error| format!("Body value store: {error:?}"))?
        };
        let mut drivers = core::array::from_fn(|_| InstalledBack::inactive());
        for (partition_index, (fragment, part)) in
            fragments.iter().zip(&lowered.partitions).enumerate()
        {
            for node in &part.nodes {
                drivers[usize::from(node.node.0)] = if let Some(scan) =
                    scans.iter_mut().find(|scan| {
                        scan.partition == partition_index && scan.owner == node.placement_id
                    }) {
                    InstalledBack::BodyScan(Box::new(
                        scan.back
                            .take()
                            .ok_or("installed scan owner was prepared twice")?,
                    ))
                } else if let Some(each) = each.iter_mut().find(|each| {
                    each.partition == partition_index && each.owner == node.placement_id
                }) {
                    InstalledBack::BodyEach(Box::new(
                        each.back
                            .take()
                            .ok_or("installed Each owner was prepared twice")?,
                    ))
                } else {
                    super::super::preparation::prepare_ordinary_operation(
                        fragment,
                        &node.placement_id,
                        &mut values,
                    )?
                };
            }
        }
        if each.iter().any(|each| each.back.is_some()) {
            return Err("installed Each has no lowered owner node".into());
        }
        if scans.iter().any(|scan| scan.back.is_some()) {
            return Err("installed scan has no lowered owner node".into());
        }
        let mut tables = KernelTables::prepare(&lowered.partitions.iter().collect::<Vec<_>>())?;
        for (fragment, partition) in fragments.iter().zip(&lowered.partitions) {
            tables.bind_fusions(fragment, partition)?;
        }
        let signs = fore_sign_storage::prepare(sign_items, fore.has_ports())?;
        let mut typed_record_hosts = Vec::with_capacity(usize::from(lowered.nodes));
        if funding.is_some() {
            typed_record_hosts.extend((0..usize::from(lowered.nodes)).map(|_| None));
        } else {
            for fragment in &fragments {
                typed_record_hosts.extend(super::super::typed_record_back::prepare_hosts(fragment));
            }
        }
        let mut image_text_hosts = Vec::with_capacity(usize::from(lowered.nodes));
        if funding.is_some() {
            image_text_hosts.extend((0..usize::from(lowered.nodes)).map(|_| None));
        } else {
            for fragment in &fragments {
                image_text_hosts.extend(super::super::image_text_back::prepare_hosts(fragment));
            }
        }
        let mut image_text_record_hosts = Vec::with_capacity(usize::from(lowered.nodes));
        if funding.is_some() {
            image_text_record_hosts.extend((0..usize::from(lowered.nodes)).map(|_| None));
        } else {
            for fragment in &fragments {
                image_text_record_hosts.extend(
                    super::super::image_text_record_back::prepare_hosts(fragment),
                );
            }
        }
        let mut text_state_hosts = Vec::with_capacity(usize::from(lowered.nodes));
        for host in fragments.iter().flat_map(|fragment| {
            fragment
                .placements
                .iter()
                .map(super::super::text_state_back::TextStateHost::from_placement)
        }) {
            text_state_hosts.push(host?);
        }
        let data_text_hosts = data_text::prepare(
            &fragments,
            &lowered.partitions,
            #[cfg(target_os = "linux")]
            selected_file,
        )?;
        let mut face_labels = vec![None; MAX_NODES];
        for (fragment, partition) in fragments.iter().zip(&lowered.partitions) {
            for placement in &fragment.placements {
                if conduit_semantic_catalog::face_data_label_required(placement.kind_id.as_str()) {
                    let node = partition
                        .identity
                        .node_for_placement(&placement.placement_id)
                        .ok_or("named Face Gear was not lowered")?;
                    face_labels[usize::from(node.0)] = Some(
                        conduit_semantic_catalog::face_data_label(&placement.configuration)?.into(),
                    );
                }
            }
        }
        let input_keymaps = super::super::input_semantic_backs::prepare_keymaps::<MAX_NODES>(
            fragments
                .iter()
                .zip(&lowered.partitions)
                .flat_map(|(fragment, partition)| {
                    fragment.placements.iter().map(|placement| {
                        (
                            partition
                                .identity
                                .node_for_placement(&placement.placement_id)
                                .map(|node| usize::from(node.0)),
                            placement,
                        )
                    })
                }),
        )?;
        let mut operations = Vec::with_capacity(
            lowered
                .partitions
                .iter()
                .map(|part| part.host_calls.len())
                .sum(),
        );
        for part in &lowered.partitions {
            operations.extend(part.host_calls.iter().cloned());
        }
        let scheduler = tables.install(drivers, values, signs)?;
        Ok(Self {
            terminal_events: None,
            error_diagnostic: None,
            cleanup_diagnostic: None,
            creche: None,
            scheduler,
            face_labels,
            input_origin: None,
            fore,
            activations,
            operations,
            partitions: lowered
                .partitions
                .into_iter()
                .map(|part| part.identity)
                .collect(),
            typed_record_hosts,
            image_text_hosts,
            image_text_record_hosts,
            text_state_hosts,
            data_text_hosts,
            input_keymaps,
            requests: Vec::with_capacity(request_capacity),
            clock_observations: KernelClockObservations::with_capacity(usize::from(sign_items)),
            supported_pure_activation,
            supported_parsec_input_scan,
            parsec_map: None,
            parsec_terminal: None,
            todo_checkpoint: None,
            todo_checkpoint_read: None,
        })
    }
}
