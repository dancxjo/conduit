//! Finite local Body kernel preparation and partition binding.
use super::super as installed;
use super::*;

impl<'a> BodyKernel<'a> {
    #[cfg(test)]
    pub(crate) fn prepare(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        fore_inputs: &'a [ExternalForeInput],
        sequential_fore: bool,
        has_fore_output: bool,
    ) -> Result<Self, String> {
        Self::prepare_with_live(
            partitions,
            has_keyboard,
            parent_play,
            fore_inputs,
            sequential_fore,
            has_fore_output,
            None,
            None,
        )
    }

    pub(crate) fn prepare_on_host(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        fore_inputs: &'a [ExternalForeInput],
        sequential_fore: bool,
        has_fore_output: bool,
        host: &conduit_core::HostAdvertisement,
    ) -> Result<Self, String> {
        let sources = parsec_sources::prepare(partitions, host)?;
        Self::prepare_with_live(
            partitions,
            has_keyboard,
            parent_play,
            fore_inputs,
            sequential_fore,
            has_fore_output,
            None,
            sources.as_ref(),
        )
    }

    pub(crate) fn prepare_live_on_host(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        queue: &BodyLiveForeQueue,
        control: &RunControl,
        host: &conduit_core::HostAdvertisement,
    ) -> Result<Self, String> {
        if partitions.len() != 1 || !queue.matches_plan(&partitions[0].plan, control) {
            return Err("live Fore queue differs from the exact Body Plan or stop control".into());
        }
        if has_keyboard {
            BodyLiveForeQueue::validate_parsec_input_plan(&partitions[0].plan, host)?;
        }
        let sources = parsec_sources::prepare(partitions, host)?;
        Self::prepare_with_live(
            partitions,
            has_keyboard,
            parent_play,
            &[],
            true,
            true,
            Some(queue),
            sources.as_ref(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_with_live(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        fore_inputs: &'a [ExternalForeInput],
        sequential_fore: bool,
        has_fore_output: bool,
        live: Option<&BodyLiveForeQueue>,
        parsec_sources: Option<&std::sync::Arc<crate::flow_activation::ParsecSourcePlans>>,
    ) -> Result<Self, String> {
        let fragments = partitions
            .iter()
            .map(|partition| {
                partition
                    .plan
                    .fragments
                    .first()
                    .filter(|_| partition.plan.fragments.len() == 1)
                    .ok_or("local body execution requires one local fragment per Plot")
            })
            .collect::<Result<Vec<_>, _>>()?;
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
        let lowered = lower_local_fragment_set_from_plans(
            &plans,
            FIXED_KERNEL_STORAGE_PROFILE,
            FragmentSetBounds {
                fragments: conduit_body::MAX_BODY_PLOTS as u16,
                nodes: MAX_NODES as u16,
                cords: MAX_CORDS as u16,
                queue_slots: MAX_QUEUE_SLOTS as u16,
                value_bytes: 16 * 1024 * 1024,
                sign_items: 4096,
                sign_bytes: 4 * 1024 * 1024,
            },
        )
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
            } else if !timer(&operation.contract_id)
                && !typed_record_codec(&operation.contract_id)
                && !image_text(&operation.contract_id)
                && !text_state(&operation.contract_id)
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
                    ordinary = preparation::back_budget(placement)?;
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
        let mut values = HostedValueStore::new(items.max(1), maximum, bytes.max(1))
            .map_err(|error| format!("Body value store: {error:?}"))?;
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
                    preparation::prepare_ordinary_operation(
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
        let typed_record_hosts = fragments
            .iter()
            .flat_map(|fragment| installed::typed_record_back::prepare_hosts(fragment))
            .collect();
        let image_text_hosts = fragments
            .iter()
            .flat_map(|fragment| installed::image_text_back::prepare_hosts(fragment))
            .collect();
        let image_text_record_hosts = fragments
            .iter()
            .flat_map(|fragment| installed::image_text_record_back::prepare_hosts(fragment))
            .collect();
        let text_state_hosts = fragments
            .iter()
            .flat_map(|fragment| {
                fragment
                    .placements
                    .iter()
                    .map(installed::text_state_back::TextStateHost::from_placement)
            })
            .collect::<Result<Vec<_>, String>>()?;
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
        let input_keymaps = installed::input_semantic_backs::prepare_keymaps::<MAX_NODES>(
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
        let scheduler = tables.install(drivers, values, signs)?;
        Ok(Self {
            scheduler,
            face_labels,
            input_origin: None,
            fore,
            activations,
            operations: lowered
                .partitions
                .iter()
                .flat_map(|part| part.host_calls.clone())
                .collect(),
            partitions: lowered
                .partitions
                .into_iter()
                .map(|part| part.identity)
                .collect(),
            typed_record_hosts,
            image_text_hosts,
            image_text_record_hosts,
            text_state_hosts,
            input_keymaps,
            requests: Vec::with_capacity(request_capacity),
            clock_observations: KernelClockObservations::with_capacity(usize::from(sign_items)),
            supported_pure_activation,
            supported_parsec_input_scan,
            parsec_map: None,
            parsec_terminal: None,
            todo_checkpoint: None,
            todo_checkpoint_read: None,
            creche: None,
        })
    }
}
