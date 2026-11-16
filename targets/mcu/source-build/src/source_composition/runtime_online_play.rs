// One lexically scoped online preparation and its continuing admitted Play.
// ONLY an initial preparation Err with no prepared owner returns a reason.
// Renderer/admission/Start refusal and every successful execution diverge while
// retaining their existing owners. The caller retains Root/arena/Face/backing.
{
            // Caller-owned affine slots live in the original future storage.
            // Poll boundaries return only status, never another fat workset copy.
            #[cfg(fore_carrier)]
            let mut fore_hot = None;
            #[cfg(fore_carrier)]
            let mut fore_cold = None;
            #[cfg(fore_carrier)]
            let mut fore_buffers =
                crate::play::fore::buffers().expect("funded installed Fore buffers");
            // Retained loans drop before their caller-owned rows and backing.
            let mut prepared_slot = None;

            crate::runtime::stack_checkpoint(3);
            let prepared_status = include!(concat!(
                env!("OUT_DIR"),
                "/source_runtime_online_prepare.rs"
            ));
            crate::runtime::stack_checkpoint(4);
            if let Err(reason) = prepared_status {
                // The original preparation returned Err before publishing any
                // prepared owner. Its partial tickets already dropped there.
                if prepared_slot.is_none() {
                    return reason;
                }
                // An inconsistent/unknown owner state never becomes permission
                // to release its loan or claim a pre-Start diagnostic outcome.
                loop {
                    core::hint::black_box(&prepared_slot);
                    core::hint::black_box(&arena);
                    Timer::after_secs(1).await;
                }
            }

            // Successive polls retire preparation scratch before admission and
            // provider binding need their own stack. Custody moves once; no
            // command, resource ledger, or execution grant is reconstructed.
            crate::runtime::stack_checkpoint(5);
            let mut presentation_slot = None;
            let prepared_status = match prepared_status {
                Err(reason) => Err(reason),
                Ok(()) => {
                    let future = crate::play_service::prepare_online_in_slot(
                        &prepared_slot,
                        &mut presentation_slot,
                        &mut equipment.renderer,
                        &mut issuers.renderer,
                        &operations.renderer,
                        reader.take().expect("original unclaimed Reader"),
                        &face,
                    );
                    conduit_mcu_host_runtime::poll_boundary::separate(core::pin::pin!(future))
                        .await
                        .map_err(|error| {
                            use core::fmt::Write;
                            let _ = write!(&mut refusal_detail, "{error:?}");
                            "renderer-preparation"
                        })
                }
            };
            crate::runtime::stack_checkpoint(6);
            let mut started_slot = None;
            crate::runtime::stack_checkpoint(7);
            let mut scopes_slot = None;
            let outcome = match prepared_status {
                Err(reason) => Err(reason),
                Ok(()) => {
                    let future = async {
                        let (
                            prepared,
                            workspace,
                            bootstrap,
                            member,
                            configuration,
                            resources,
                            policy,
                            held_pools,
                        ) = prepared_slot.take().ok_or("prepared-slot-empty")?;
                        crate::play_service::start_prepared_into(
                            prepared,
                            workspace,
                            bootstrap,
                            member,
                            configuration,
                            resources,
                            policy,
                            presentation_slot.take().ok_or("presentation-slot-empty")?,
                            &face,
                            &held_pools,
                            append_output.len() + core::mem::size_of::<conduit_mcu_host_runtime::execution_inspection::PreparedAppendStatus>() + {
                                #[cfg(fore_carrier)] { core::mem::size_of::<Option<crate::play::fore::installation::Hot>>() + core::mem::size_of::<crate::play::fore::Buffers>() }
                                #[cfg(not(fore_carrier))] { 0 }
                            },
                            &mut started_slot,
                            &mut scopes_slot,
                        )
                        .map_err(|error| {
                            use core::fmt::Write;
                            let _ = write!(&mut refusal_detail, "{error:?}");
                            "complete-admission-start"
                        })?;
                        Ok::<(), &'static str>(())
                    };
                    let mut future = core::pin::pin!(future);
                    conduit_mcu_host_runtime::poll_boundary::separate(future.as_mut()).await
                }
            };
            crate::runtime::stack_checkpoint(8);
            match outcome {
                Ok(()) => {
                    // Destructure affine Started ownership synchronously before
                    // constructing the run future; no duplicate whole owner is
                    // retained beside its executing children across suspension.
                    let future = crate::play::fore::run_started(
                        started_slot.take().expect("original Started output slot"),
                        optional.take().expect("original optional tracking"),
                        scopes_slot.take().expect("original Scopes output slot"),
                        arena.take().expect("original retained IMAGE lease"),
                        &face,
                        {
                            #[cfg(fore_carrier)] { None }
                            #[cfg(not(fore_carrier))] { Some(link) }
                        },
                        &equipment.activity,
                        &mut append_output,
                        append_status.take().expect("original prepared status custody"),
                    );
                    let mut future = core::pin::pin!(future);
                    match conduit_mcu_host_runtime::poll_boundary::separate(future.as_mut()).await {}

                }
                Err(reason) => {
                    // No partial execution: retain the original signed input and
                    // any fenced reservations. Read-only refusal stays observable
                    // on the same carrier; recovery remains the original reboot.
                    #[cfg(fore_carrier)]
                    loop {
                        core::hint::black_box(&arena);
                        core::hint::black_box(&prepared_slot);
                        core::hint::black_box(reason);
                        Timer::after_secs(1).await;
                    }
                    #[cfg(not(fore_carrier))]
                    loop {
                        core::hint::black_box(&arena);
                        link.connected().await;
                        let mut input = [0; 128];
                        while link.receive(&mut input).await.is_ok() {
                            #[derive(serde::Serialize)]
                            struct Status<'a> {
                                phase: &'a str,
                                reason: &'a str,
                                detail: &'a str,
                                heap_used_bytes: usize,
                                heap_peak_bytes: Option<usize>,
                                heap_limit_bytes: usize,
                            }
                            let mut response = [0u8; 512];
                            let count = serde_json_core::to_slice(
                                &Status {
                                    phase: "refused-before-play",
                                    reason,
                                    detail: refusal_detail.as_str(),
                                    heap_used_bytes: allocator_used(),
                                    heap_peak_bytes: crate::runtime::allocator_peak(),
                                    heap_limit_bytes: crate::config::HEAP_BYTES,
                                },
                                &mut response,
                            )
                            .expect("bounded static refusal");
                            if link.send(&response[..count]).await.is_err() {
                                break;
                            }
                        }
                        link.reset();
                        Timer::after_millis(10).await;
                    }
                }
            }
}
