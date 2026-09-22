//! Synchronous value transformations for admitted browser Host requests.
//! This dispatcher never advances the scheduler or owns pending platform effects.
use super::{debug_error, TourScheduler};
use conduit_core::{HostCallRequirement, PlannedGear};
use conduit_kernel::scheduler::HostCallRequest;
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};

#[path = "engine_transforms/measurement_window.rs"]
mod measurement_window;
#[path = "engine_transforms/stroke_capture.rs"]
mod stroke_capture;

pub(in crate::form_runner) fn complete_transform(
    scheduler: &mut TourScheduler,
    placement: &PlannedGear,
    operation: &HostCallRequirement,
    request: HostCallRequest,
) -> Result<bool, String> {
    if operation.contract_id.as_str() == crate::installed_browser::application::STATE_OPERATION {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?
            .to_vec();
        let index = usize::from(request.node.0);
        let result = scheduler.applications[index]
            .as_mut()
            .ok_or("browser application state was not prepared before Play")?
            .execute(&input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler
                            .kernel
                            .store_host_value(&bytes)
                            .map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(_) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::InvalidInput,
                    detail: 70,
                }),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::text_state::HOST_CALL {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.text_states[usize::from(request.node.0)]
            .as_mut()
            .ok_or("browser text state was not prepared before Play")?
            .execute(input);
        let outcome = match result {
            Ok(output) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: output
                    .map(|bytes| {
                        let value = scheduler
                            .kernel
                            .store_host_value(bytes)
                            .map_err(debug_error)?;
                        BoundedValueRef::new(value, operation.maximum_output_bytes)
                            .map_err(debug_error)
                    })
                    .transpose()?,
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::keymap::HOST_CALL {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.keymaps[usize::from(request.node.0)]
            .as_mut()
            .ok_or("browser keymap was not prepared before Play")?
            .execute(input);
        let outcome = match result {
            Ok(output) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: output
                    .map(|bytes| {
                        let value = scheduler
                            .kernel
                            .store_host_value(bytes.as_slice())
                            .map_err(debug_error)?;
                        BoundedValueRef::new(value, operation.maximum_output_bytes)
                            .map_err(debug_error)
                    })
                    .transpose()?,
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::structured_selector::HOST_CALL {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.structured_selectors[usize::from(request.node.0)]
            .as_mut()
            .ok_or("structured selector was not prepared before Play")?
            .execute(input);
        let outcome = match result {
            Ok(output) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: output
                    .map(|bytes| {
                        let value = scheduler
                            .kernel
                            .store_host_value(bytes)
                            .map_err(debug_error)?;
                        BoundedValueRef::new(value, operation.maximum_output_bytes)
                            .map_err(debug_error)
                    })
                    .transpose()?,
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::template_storage::HOST_CALL {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.template_stores[usize::from(request.node.0)]
            .as_mut()
            .ok_or("template store was not prepared before Play")?
            .execute(input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler
                            .kernel
                            .store_host_value(bytes)
                            .map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::replay_control::HOST_CALL {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.replay_controls[usize::from(request.node.0)]
            .as_mut()
            .ok_or("replay control was not prepared before Play")?
            .execute(operation.contract_id.as_str(), input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: bytes
                    .map(|bytes| {
                        let value = scheduler
                            .kernel
                            .store_host_value(bytes)
                            .map_err(debug_error)?;
                        BoundedValueRef::new(value, operation.maximum_output_bytes)
                            .map_err(debug_error)
                    })
                    .transpose()?,
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::replay_source::HOST_CALL {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.replay_sources[usize::from(request.node.0)]
            .as_mut()
            .ok_or("replay source was not prepared before Play")?
            .execute(input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler
                            .kernel
                            .store_host_value(bytes)
                            .map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::historical::HOST_CALL {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.histories[usize::from(request.node.0)]
            .as_mut()
            .ok_or("bounded history was not prepared before Play")?
            .execute(input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler
                            .kernel
                            .store_host_value(bytes)
                            .map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::record_delivery::HOST_CALL {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.deliveries[usize::from(request.node.0)]
            .as_mut()
            .ok_or("delivery codec was not prepared before Play")?
            .execute(input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler
                            .kernel
                            .store_host_value(bytes)
                            .map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(refusal) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(crate::installed_browser::record_delivery::failure(refusal)),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if let Some(port) =
        crate::installed_browser::pattern_comparison::input_port(operation.contract_id.as_str())
    {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.comparisons[usize::from(request.node.0)]
            .as_mut()
            .ok_or("comparison codec was not prepared before Play")?
            .execute(port, input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: bytes
                    .map(|bytes| {
                        let value = scheduler
                            .kernel
                            .store_host_value(bytes)
                            .map_err(debug_error)?;
                        BoundedValueRef::new(value, operation.maximum_output_bytes)
                            .map_err(debug_error)
                    })
                    .transpose()?,
                failure: None,
            },
            Err(refusal) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(crate::installed_browser::pattern_comparison::failure(
                    refusal,
                )),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if crate::installed_browser::timing::OPERATIONS.contains(&operation.contract_id.as_str()) {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.timing[usize::from(request.node.0)]
            .as_mut()
            .ok_or("timing codec was not prepared before Play")?
            .execute(operation.contract_id.as_str(), input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler
                            .kernel
                            .store_host_value(bytes)
                            .map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if crate::installed_browser::json::OPERATIONS.contains(&operation.contract_id.as_str()) {
        let result = crate::installed_browser::json::execute(
            placement,
            operation.contract_id.as_str(),
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        );
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler.store_host_value(&bytes).map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::measurement_plot::HOST_CALL {
        let result = crate::installed_browser::measurement_plot::execute(
            placement,
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        );
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler.store_host_value(&bytes).map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::measurement_summary::HOST_CALL {
        let result = crate::installed_browser::measurement_summary::execute(
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        );
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler.store_host_value(&bytes).map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if crate::installed_browser::measurement_window::OPERATIONS
        .contains(&operation.contract_id.as_str())
    {
        measurement_window::complete(scheduler, operation, request)?;
        return Ok(true);
    }
    if crate::installed_browser::garden_step::OPERATIONS.contains(&operation.contract_id.as_str()) {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.garden_steps[usize::from(request.node.0)]
            .as_mut()
            .ok_or("Garden step was not prepared before Play")?
            .execute(operation.contract_id.as_str(), input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: bytes
                    .map(|bytes| {
                        let value = scheduler
                            .kernel
                            .store_host_value(&bytes)
                            .map_err(debug_error)?;
                        BoundedValueRef::new(value, operation.maximum_output_bytes)
                            .map_err(debug_error)
                    })
                    .transpose()?,
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if crate::installed_browser::stroke_capture::OPERATIONS
        .contains(&operation.contract_id.as_str())
    {
        stroke_capture::complete(scheduler, operation, request)?;
        return Ok(true);
    }
    if crate::installed_browser::measurement_hysteresis::OPERATIONS
        .contains(&operation.contract_id.as_str())
    {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let result = scheduler.measurement_hysteresis[usize::from(request.node.0)]
            .as_mut()
            .ok_or("measurement hysteresis was not prepared before Play")?
            .execute(operation.contract_id.as_str(), input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: bytes
                    .map(|bytes| {
                        let value = scheduler
                            .kernel
                            .store_host_value(&bytes)
                            .map_err(debug_error)?;
                        BoundedValueRef::new(value, operation.maximum_output_bytes)
                            .map_err(debug_error)
                    })
                    .transpose()?,
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if crate::installed_browser::typed_record::OPERATIONS.contains(&operation.contract_id.as_str())
    {
        let result = crate::installed_browser::typed_record::execute(
            operation.contract_id.as_str(),
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        );
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler.store_host_value(&bytes).map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::record_queue::HOST_CALL {
        let result = crate::installed_browser::record_queue::execute(
            placement,
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        );
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler.store_host_value(&bytes).map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::pointer_selector::HOST_CALL {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let output = scheduler.selectors[usize::from(request.node.0)]
            .as_mut()
            .ok_or("selector was not prepared before Play")?
            .execute(input)?;
        let value = scheduler
            .kernel
            .store_host_value(output)
            .map_err(debug_error)?;
        scheduler
            .kernel
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, operation.maximum_output_bytes)
                            .map_err(debug_error)?,
                    ),
                    failure: None,
                },
            )
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::QUANTITY_HOST_CALL {
        let encoded = crate::installed_browser::transform_quantity(
            scheduler.mappings[usize::from(request.node.0)]
                .ok_or("quantity mapping was not prepared before Play")?,
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        )?;
        let outcome = match encoded {
            Ok(bytes) => {
                let value = scheduler.store_host_value(&bytes).map_err(debug_error)?;
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, conduit_core::QUANTITY_ENCODED_LEN as u32)
                            .map_err(debug_error)?,
                    ),
                    failure: None,
                }
            }
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::NORMALIZE_QUANTITY_OPERATION {
        let converted = crate::installed_browser::normalize_quantity(
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        );
        let outcome = match converted {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler.store_host_value(&bytes).map_err(debug_error)?,
                        conduit_core::SCALAR_ENCODED_LEN as u32,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(failure) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(failure),
            },
        };
        scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    if operation.contract_id.as_str() == crate::installed_browser::QUANTITY_WRAP_OPERATION {
        let (encoded, length) = crate::installed_browser::wrap_quantity(
            scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?,
        )?;
        let value = scheduler
            .store_host_value(&encoded[..length])
            .map_err(debug_error)?;
        scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, operation.maximum_output_bytes)
                            .map_err(debug_error)?,
                    ),
                    failure: None,
                },
            )
            .map_err(debug_error)?;
        return Ok(true);
    }
    Ok(false)
}
