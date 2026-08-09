use super::*;

impl TripleSource {
    pub fn prepare() -> Result<Self, String> {
        let exact = triple::exact_plan()?;
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
            .cloned()
            .ok_or_else(|| "triple source fragment missing".to_owned())?;
        let lowered = lower_plan_fragment(&fragment).map_err(|error| format!("{error:?}"))?;
        if lowered.nodes.len() != 2
            || lowered.cords.len() != 3
            || lowered.routes.len() != 1
            || lowered.routes[0].targets.len() != 3
            || lowered.remote_endpoints.len() != 2
            || lowered.host_operations.len() != 2
            || lowered.cord_value_slots != 3
        {
            return Err("triple source did not lower to the sealed fan-out profile".to_owned());
        }
        let pulse_node = node_for_kind(&fragment, &lowered, PULSE_KIND)?;
        let show_node = node_for_kind(&fragment, &lowered, SHOW_KIND)?;
        let show_placement = fragment.placements[usize::from(show_node.0)]
            .placement_id
            .clone();
        let configuration = parse_pulse_configuration(
            &fragment.placements[usize::from(pulse_node.0)].configuration,
        )
        .map_err(|error| error.to_string())?;
        if configuration.count != VALUES as u64 || configuration.period_ms != 250 {
            return Err("triple form is not the accepted sixteen-value Signal vector".to_owned());
        }

        let mut values = HostedValueStore::new(STORED_ITEMS, SIGNAL_ENCODED_LEN, STORED_BYTES)
            .map_err(|error| format!("{error:?}"))?;
        let mut signals = Vec::with_capacity(VALUES);
        for sequence in 0..configuration.count {
            let signal = Signal {
                sequence,
                level: conduit_signal::signal_level_for_sequence(
                    sequence,
                    configuration.initial_level,
                ),
            };
            signals.push(
                values
                    .store(&encode_signal(&signal).encoded)
                    .map_err(|error| format!("{error:?}"))?,
            );
        }
        let mut waits = Vec::with_capacity(WAITS);
        for _ in 0..WAITS {
            waits.push(
                values
                    .store(&configuration.period_ms.to_le_bytes())
                    .map_err(|error| format!("{error:?}"))?,
            );
        }

        let mut routes = FixedRoutes::<{ 2 * PORTS }, 3>::new(PORTS as u16);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(|error| format!("{error:?}"))?;
        }
        routes.seal().map_err(|error| format!("{error:?}"))?;
        let mut host_bindings = FixedHostOperationBindings::<2>::new(1);
        for operation in &lowered.host_operations {
            host_bindings
                .install(operation.node, operation.binding)
                .map_err(|error| format!("{error:?}"))?;
        }
        host_bindings.seal().map_err(|error| format!("{error:?}"))?;
        let mut operations = [None, None];
        operations[usize::from(pulse_node.0)] =
            Some(TripleOperation::pulse(signals.clone(), waits));
        operations[usize::from(show_node.0)] = Some(TripleOperation::show(signals));
        let drivers = operations
            .map(|operation| {
                OperationDriver::new(
                    operation.ok_or_else(|| "missing triple operation".to_owned())?,
                )
                .map_err(|error| format!("{error:?}"))
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "triple driver width".to_owned())?;
        let sign_bytes = u32::from(SIGN_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "triple sign bytes overflow".to_owned())?;
        let sign =
            HostedSignLog::new(SIGN_ITEMS, sign_bytes).map_err(|error| format!("{error:?}"))?;
        let scheduler = TripleScheduler::new_with_host_operations(
            lowered
                .node_specs
                .clone()
                .try_into()
                .map_err(|_| "triple node width".to_owned())?,
            lowered
                .cords
                .iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "triple cord width".to_owned())?,
            routes,
            host_bindings,
            drivers,
            values,
            sign,
        )
        .map_err(|error| format!("{error:?}"))?;
        let active_play =
            bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let identity = KernelExecutionIdentityMap::new(&lowered.identity, &active_play, 31, 16, 17)
            .map_err(|error| format!("{error:?}"))?;
        let browser = remote_branch(&fragment, &lowered, ConnectionBase::WebSocket)?;
        let pico = remote_branch(&fragment, &lowered, ConnectionBase::UsbCdc)?;
        if browser.binding.source_active_play_id != active_play.active_play_id
            || pico.binding.source_active_play_id != active_play.active_play_id
        {
            return Err("triple sessions disagree with the one source play".to_owned());
        }
        let receipts = Vec::with_capacity(VALUES);
        let mut source = Self {
            scheduler,
            fragment,
            lowered,
            identity,
            pulse_node,
            show_node,
            show_placement,
            active_play_id: active_play.active_play_id,
            browser,
            pico,
            receipts,
            seal: CapacitySeal {
                values: (0, 0),
                sign: 0,
                drivers: 0,
                identity: (0, 0, 0),
                receipts: 0,
            },
        };
        source.seal = source.capacity_seal();
        Ok(source)
    }
}
