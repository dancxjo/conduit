//! Mutable finite software initialization; Source binding is checked before entry.
use super::*;
impl<V: ValueStorage> PreparedResidentSoftware<V> {
    pub(super) fn initialize_with_retained<'a>(
        construction: Construction,
        text: impl Fn(
            &c::AssignedComputationRequirement,
        ) -> Result<(&'a str, Option<&'a [u8]>), NativePreparationRefusal>,
        schemas: Option<&[ResidentSelectorSchemas]>,
        linked: Option<&[ResidentExpressionMaterial]>,
        mut values: V,
    ) -> Result<Self, (NativePreparationRefusal, V)> {
        let parts = (|| {
            use NativePreparationRefusal as E;
            let Construction {
                roles,
                computations,
                count,
                period_ms,
                ..
            } = &construction;
            let (roles, computations, count, period_ms) =
                (*roles, computations, *count, *period_ms);
            let maximum = u32::try_from(count).map_err(|_| E::Configuration)?;
            // Fixed preparation backing lives in the admitted heap, not beneath the
            // recursive program/type decoder on a constrained target's stack.
            let mut drivers = empty_slots::<ApplianceBack, NODES>();
            let mut selectors = empty_slots::<PreparedCaptureSelector, NODES>();
            let program_key = c::AssignedIdentity::from_text("program");
            let selector_key = c::AssignedIdentity::from_text("selector");
            // Prepare the largest temporary evaluator first, while other retained
            // codecs/publications do not yet occupy the admission arena. This is
            // preparation ordering only; runtime order remains the admitted tables.
            let mut initialized = [false; NODES];
            for _ in 0..computations.len() {
                let computation = computations
                    .iter()
                    .filter(|r| !initialized[usize::from(r.node)])
                    .max_by_key(|r| r.text_bytes)
                    .ok_or(E::Roles)?;
                let node = usize::from(computation.node);
                initialized[node] = true;
                let (text, canonical_bytes) = text(computation)?;
                let driver = if computation.key == program_key {
                    if let Some(materials) = linked {
                        let material = materials
                            .iter()
                            .find(|m| m.node == computation.node && m.key == computation.key)
                            .ok_or(E::Configuration)?;
                        // SAFETY: only the explicitly unsafe trusted-Make entrance
                        // supplies this optional set, after exact binding checks.
                        unsafe {
                            CaptureBack::expression_from_linked_make(
                                material.bytes,
                                material.program_digest,
                                (material.maximum_input_bytes, material.maximum_output_bytes),
                                c::PortTemporal::Flow { closes: true },
                            )
                        }
                        .map_err(init)?
                    } else {
                        let program = match canonical_bytes {
                            Some(bytes) => {
                                conduit_plot::PortableExpressionProgram::from_canonical_bytes(bytes)
                            }
                            None => {
                                conduit_plot::PortableExpressionProgram::from_canonical_hex(text)
                            }
                        }
                        .map_err(init)?;
                        CaptureBack::expression(&program, c::PortTemporal::Flow { closes: true })
                            .map_err(init)?
                    }
                } else if computation.key == selector_key {
                    let selector = c::StructuredSelector::from_canonical_hex(text).map_err(init)?;
                    selectors[node] = Some(
                        match schemas {
                            Some(schemas) => {
                                let schema = schemas
                                    .iter()
                                    .find(|schema| {
                                        schema.node == computation.node
                                            && schema.key == computation.key
                                    })
                                    .ok_or(E::Configuration)?;
                                PreparedCaptureSelector::with_static_schema(
                                    selector,
                                    schema.input_schema,
                                    schema.output_schema,
                                    1024,
                                    1024,
                                )
                            }
                            None => PreparedCaptureSelector::prepare(selector, 1024, 1024),
                        }
                        .map_err(init)?,
                    );
                    CaptureBack::selector(1024)
                } else {
                    return Err(E::Configuration);
                };
                drivers[node] = Some(ApplianceBack::Native(driver));
            }
            drivers[usize::from(roles.pulse.0)] =
                Some(ApplianceBack::Native(CaptureBack::pulse(1024)));
            drivers[usize::from(roles.trigger.0)] = Some(ApplianceBack::Trigger(
                conduit_time::TickTriggerBack::default(),
            ));
            drivers[usize::from(roles.observation.0)] = Some(ApplianceBack::Native(
                CaptureBack::observation(&ObservationContract::prepare().map_err(init)?)
                    .map_err(init)?,
            ));
            let order = PublicationSequence::prepare(count);
            for (node, channel) in roles.publications.into_iter().zip(PublicationChannel::ALL) {
                drivers[usize::from(node.0)] = Some(
                    ApplianceBack::source_publication(
                        &PublicationContract::prepare(channel).map_err(init)?,
                        maximum,
                        order.clone(),
                    )
                    .map_err(init)?,
                );
            }
            if drivers
                .iter()
                .enumerate()
                .any(|(node, driver)| node != usize::from(roles.tick.0) && driver.is_none())
            {
                return Err(E::Roles);
            }
            // Allocate the only stored value after all fallible codec preparation.
            // Codec refusals leave storage untouched; storage refusal returns
            // its actual state. No wait reference is orphaned and no generation
            // is reset during a retry.
            let wait = values
                .store(&c::encode_monotonic_duration(period_ms))
                .map_err(E::Storage)?;
            drivers[usize::from(roles.tick.0)] = Some(ApplianceBack::Tick(
                conduit_time::TickBack::counted(wait, count)
                    .expect("Construction checked the finite tick count"),
            ));
            Ok((drivers, selectors))
        })();
        match parts {
            Ok((drivers, selectors)) => Ok(Self {
                construction,
                drivers,
                selectors,
                values,
            }),
            Err(error) => Err((error, values)),
        }
    }
}
