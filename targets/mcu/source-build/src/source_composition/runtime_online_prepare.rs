// Lexically included original online preparation expression. It keeps the
// SAME caller-owned slots and poll boundary; no fat workset return/extra owner
// frame, alternate executor or new authority enters through this extraction.
{
    let future = async {
        let parts = root.execution_parts().map_err(|_| "execution-custody")?;
        let policy = Policy {
            preparation: crate::config::PREPARATION_DELEGATION.as_ref(),
            execution: crate::config::EXECUTION_DELEGATION.as_ref(),
            active_worksets: 0,
            // A logical complete-owner receipt is bounded separately
            // from ASG representation. Physical backing is exclusively
            // held by the original whole-arena lease.
            maximum_retained_bytes: arena.bytes(),
        };
        let identity = conduit_core::AssignedIdentity::from_text;
        let mut held_pools = heapless::Vec::<_, 6>::from_slice(&[
            identity(crate::config::RESIDENT_WORK_POOL),
            identity(
                equipment
                    .pulse
                    .as_ref()
                    .ok_or("pulse-busy")?
                    .identity()
                    .resource_pool_id
                    .as_str(),
            ),
            identity(
                equipment
                    .display
                    .as_ref()
                    .ok_or("display-busy")?
                    .native_identity()
                    .resource_pool_id
                    .as_str(),
            ),
            identity(
                equipment.waits[0]
                    .as_ref()
                    .ok_or("wait-busy")?
                    .native_identity()
                    .resource_pool_id
                    .as_str(),
            ),
            identity(
                equipment
                    .renderer
                    .native_identity()
                    .resource_pool_id
                    .as_str(),
            ),
        ])
        .expect("five original held pools");
        if let Some(pool) = equipment.recording.held_pool() {
            held_pools
                .push(identity(pool.as_str()))
                .map_err(|_| "held-pools")?;
        }
        let context = conduit_body::NativePreparationContext {
            bootstrap: parts.bootstrap,
            member: parts.member,
            actual: parts.resources.host(),
            image_id: parts.configuration.image_id,
            configuration_id: parts.configuration.configuration_id,
            now_millis: now_ms(),
        };
        #[cfg(record_service)]
        {
            recording_resource = equipment
                .recording
                .resource_binding(remaining())
                .map_err(|_| "recording-resource")?;
        }
        #[cfg(fore_carrier)]
        {
            let (hot, cold) = crate::play::fore::installation::prepare(
                context,
                crate::config::FORE_CARRIER_IMPLEMENTATION,
                crate::config::FORE_CARRIER_BOUNDS.ok_or("missing-installed-fore")?,
                &source::sampling_layout::fore(),
            )?;
            fore_hot = Some(hot);
            fore_cold = Some(cold);
        }
        #[cfg(fore_carrier)]
        let hot = fore_hot.as_ref().ok_or("missing-fore-hot")?;
        // The cold callback transfers these original exclusive loans once;
        // it never returns a reborrow of its own FnMut capture storage.
        #[cfg(fore_carrier)]
        let mut carrier_inputs = Some((&mut *link, &mut fore_buffers));
        let prepared = crate::play::prepare(
            &*parts.workspace,
            context.into(),
            policy,
            &mut residents,
            &mut *software,
            &mut *output_software,
            &mut routing,
            &mut equipment.pulse,
            &mut equipment.display,
            &mut equipment.waits,
            equipment.recording.ready_slot(),
            #[cfg(record_service)]
            recording_resource.as_ref(),
            {
                #[cfg(not(record_service))]
                {
                    &mut issuers.recording
                }
                #[cfg(record_service)]
                {
                    [&mut issuers.recording, &mut issuers.recording_read]
                }
            },
            recording_operation.as_ref(),
            append_output.len(),
            &mut issuers.pulse,
            &mut issuers.write,
            &mut issuers.waits,
            &operations,
            sampling_values.take().ok_or("sampling-values-busy")?,
            output_values.take().ok_or("output-values-busy")?,
            &mut contracts,
            &mut refusal_detail,
            {
                #[cfg(fore_carrier)]
                {
                    Some(crate::play::fore::Selection {
                        profile: hot.profile(),
                        bindings: &hot.bindings,
                    })
                }
                #[cfg(not(fore_carrier))]
                {
                    None
                }
            },
            &mut |approved, fore, profile| {
                #[cfg(not(fore_carrier))]
                {
                    let _ = (approved, fore, profile);
                    Ok(None)
                }
                #[cfg(fore_carrier)]
                {
                    let cold = fore_cold.take().ok_or("missing-fore-cold")?;
                    let (carrier_link, carrier_buffers) = carrier_inputs.take().ok_or("fore-carrier-already-transferred")?;
                    crate::play::fore::prepare_components(
                        &parts,
                        context,
                        hot,
                        cold,
                        carrier_link,
                        carrier_buffers,
                        approved,
                        fore.ok_or("missing-opaque-fore")?,
                        profile,
                    )
                    .map(Some)
                }
            },
        )
        .map_err(|error| {
            use core::fmt::Write;
            let _ = write!(&mut refusal_detail, "{error:?}");
            "complete-source-preparation"
        })?;
        prepared_slot = Some((
            prepared,
            &*parts.workspace,
            parts.bootstrap,
            parts.member,
            parts.configuration,
            parts.resources,
            policy,
            held_pools,
        ));
        Ok::<(), &'static str>(())
    };
    let mut future = core::pin::pin!(future);
    conduit_mcu_host_runtime::poll_boundary::separate(future.as_mut()).await
}
