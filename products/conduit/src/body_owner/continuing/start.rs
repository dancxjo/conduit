//! Prepare and launch the single affine owner worker with its durable start handshake.
use super::*;
impl Owner {
    pub(crate) fn start_service_run(
        &mut self,
        root: &Path,
        maximum_millis: u64,
    ) -> Result<RunWorker, String> {
        self.start_service_run_inner(
            root,
            maximum_millis,
            #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
            &mut None,
            #[cfg(all(
                feature = "installed-environmental-sources",
                feature = "installed-body-storage",
                target_os = "linux"
            ))]
            None,
        )
    }

    #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
    pub(crate) fn start_service_native_run(
        &mut self,
        root: &Path,
        maximum_millis: u64,
        equipment: &mut Option<
            crate::durable_host::selected_environmental_sources::AttachedEquipment,
        >,
    ) -> Result<RunWorker, String> {
        if equipment.is_none() {
            return Err("selected native equipment is absent".into());
        }
        self.start_service_run_inner(
            root,
            maximum_millis,
            equipment,
            #[cfg(all(
                feature = "installed-environmental-sources",
                feature = "installed-body-storage",
                target_os = "linux"
            ))]
            None,
        )
    }

    #[cfg(all(
        feature = "installed-environmental-sources",
        feature = "installed-body-storage",
        target_os = "linux"
    ))]
    pub(crate) fn start_service_source_publication(
        &mut self,
        root: &Path,
        maximum_millis: u64,
        authority: &conduit_core::AuthorityGrant,
        preparation_limit_bytes: u64,
    ) -> Result<RunWorker, String> {
        if !(1..=MAXIMUM_SERVICE_RUN_MILLIS).contains(&maximum_millis) {
            return Err("service run duration must be 1..900000 milliseconds".into());
        }
        let input =
            self.prepare_source_recording_publication(root, authority, preparation_limit_bytes)?;
        self.start_service_run_inner(
            root,
            maximum_millis,
            &mut None,
            Some(recording_run::RecordingRun::new(input)),
        )
    }

    #[cfg(all(
        feature = "installed-environmental-sources",
        feature = "installed-body-storage",
        target_os = "linux"
    ))]
    pub(crate) fn start_service_source_readback(
        &mut self,
        root: &Path,
        maximum_millis: u64,
        authority: &conduit_core::AuthorityGrant,
        preparation_limit_bytes: u64,
    ) -> Result<RunWorker, String> {
        if !(1..=MAXIMUM_SERVICE_RUN_MILLIS).contains(&maximum_millis) {
            return Err("service run duration must be 1..900000 milliseconds".into());
        }
        let input =
            self.prepare_source_recording_readback(root, authority, preparation_limit_bytes)?;
        self.start_service_run_inner(
            root,
            maximum_millis,
            &mut None,
            Some(recording_run::RecordingRun::new(input)),
        )
    }

    #[cfg(all(
        feature = "installed-environmental-sources",
        feature = "installed-body-storage",
        target_os = "linux"
    ))]
    pub(crate) fn start_service_source_catalog(
        &mut self,
        root: &Path,
        maximum_millis: u64,
        authority: &conduit_core::AuthorityGrant,
        preparation_limit_bytes: u64,
    ) -> Result<RunWorker, String> {
        if !(1..=MAXIMUM_SERVICE_RUN_MILLIS).contains(&maximum_millis) {
            return Err("service run duration must be 1..900000 milliseconds".into());
        }
        let input =
            self.prepare_source_recording_catalog(root, authority, preparation_limit_bytes)?;
        self.start_service_run_inner(
            root,
            maximum_millis,
            &mut None,
            Some(recording_run::RecordingRun::catalog(input)?),
        )
    }

    #[cfg(all(
        feature = "installed-environmental-sources",
        feature = "installed-body-storage",
        target_os = "linux"
    ))]
    pub(crate) fn start_service_source_catalog_recovery(
        &mut self,
        root: &Path,
        maximum_millis: u64,
        preparation_limit_bytes: u64,
    ) -> Result<RunWorker, String> {
        if !(1..=MAXIMUM_SERVICE_RUN_MILLIS).contains(&maximum_millis) {
            return Err("service run duration must be 1..900000 milliseconds".into());
        }
        let input = self.prepare_source_catalog_recovery(root, preparation_limit_bytes)?;
        self.recovered_source_catalog = None;
        self.start_service_run_inner(
            root,
            maximum_millis,
            &mut None,
            Some(recording_run::RecordingRun::recovery(input)?),
        )
    }

    fn start_service_run_inner(
        &mut self,
        root: &Path,
        maximum_millis: u64,
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        equipment: &mut Option<
            crate::durable_host::selected_environmental_sources::AttachedEquipment,
        >,
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        publication: Option<recording_run::RecordingRun>,
    ) -> Result<RunWorker, String> {
        if !(1..=MAXIMUM_SERVICE_RUN_MILLIS).contains(&maximum_millis) {
            return Err("service run duration must be 1..900000 milliseconds".into());
        }
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        let has_publication = publication.is_some();
        #[cfg(not(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        )))]
        let has_publication = false;
        if self.host.is_playing() || (self.session.realization().is_some() && !has_publication) {
            return Err("Body already has a current proposal or Play".into());
        }
        self.ensure_execution_retained(root)?;
        let checked = super::super::super::checked_retained_source(root)?
            .ok_or("retained source is unavailable for Body Play")?;
        let resident = ResidentPlot::new(
            checked.expanded.source_document_id.clone(),
            checked.expanded.checked_plot_id.clone(),
        );
        if self.resident.as_ref() != Some(&resident) {
            return Err("retained source differs from the Body's resident Plot".into());
        }
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        let mut native = match equipment.as_ref() {
            Some(equipment) => native::NativeRun::prepare(self, &checked, equipment)?,
            None => None,
        };
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        if native.is_none() && !has_publication {
            self.plan(&checked)?;
        }
        #[cfg(not(all(feature = "installed-environmental-sources", target_os = "linux")))]
        self.plan(&checked)?;
        self.persist(root)?;
        state::require_execution_capacity(root)?;
        let proposed = self
            .session
            .realization()
            .expect("Plan just committed")
            .clone();
        let host = self.host.take_for_play()?;
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        if let Some(native) = &mut native {
            native.equipment = equipment.take();
        }
        #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
        let live_fore = native
            .as_mut()
            .and_then(|run| run.live.as_mut())
            .and_then(|live| live.receiver.take());
        #[cfg(all(
            feature = "installed-environmental-sources",
            feature = "installed-body-storage",
            target_os = "linux"
        ))]
        let mut recording = publication;
        let control = RunControl::default();
        let worker_control = control.clone();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (ack_tx, ack_rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("conduit-body-play".into())
            .stack_size(BODY_PLAY_STACK_BYTES)
            .spawn(move || {
                let mut host = host;
                let mut output = BoundedOutput(Vec::with_capacity(32 * 1024));
                let mut timer = DeadlineTimer {
                    until: Instant::now() + Duration::from_millis(maximum_millis),
                    control: worker_control.clone(),
                };
                let (host, result) = thread::scope(|scope| {
                    let (finished, receiver) = mpsc::sync_channel::<()>(1);
                    let deadline_control = worker_control.clone();
                    scope.spawn(move || {
                        if receiver
                            .recv_timeout(Duration::from_millis(maximum_millis))
                            .is_err()
                        {
                            let _ = deadline_control.request_stop(
                                RunControlRequestId::new("owner/service-deadline")
                                    .expect("bounded request"),
                            );
                        }
                    });
                    let request = BodyRunRequest {
                        wake: &proposed.wake,
                        plan: &proposed.plan,
                        control: &worker_control,
                        keyboard: None,
                    };
                    let started = |play: &BodyPlayIdentity, wake: &Wake| {
                        started_tx
                            .send((play.clone(), wake.clone()))
                            .map_err(|_| "Body owner start observer disconnected".to_string())?;
                        ack_rx
                            .recv()
                            .map_err(|_| "Body owner start acknowledgement lost".to_string())?
                    };
                    #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
                    let (host, result) = {
                        #[cfg(all(
                            feature = "installed-environmental-sources",
                            feature = "installed-body-storage",
                            target_os = "linux"
                        ))]
                        if let Some(recording) = &mut recording {
                            recording.execute(host, request, &mut output, &mut timer, started)
                        } else {
                            if let Some(native) = &mut native {
                                native.execute(host, request, &mut output, &mut timer, started)
                            } else {
                                let result = host.run_body_plan_to_with_start(
                                    request,
                                    &mut output,
                                    &mut timer,
                                    started,
                                );
                                (host, result)
                            }
                        }
                        #[cfg(not(feature = "installed-body-storage"))]
                        if let Some(native) = &mut native {
                            native.execute(host, request, &mut output, &mut timer, started)
                        } else {
                            let result = host.run_body_plan_to_with_start(
                                request,
                                &mut output,
                                &mut timer,
                                started,
                            );
                            (host, result)
                        }
                    };
                    #[cfg(not(all(
                        feature = "installed-environmental-sources",
                        target_os = "linux"
                    )))]
                    let (host, result) = {
                        let result = host.run_body_plan_to_with_start(
                            request,
                            &mut output,
                            &mut timer,
                            started,
                        );
                        (host, result)
                    };
                    let _ = finished.send(());
                    (host, result)
                });
                WorkerOutcome {
                    #[cfg(all(
                        feature = "installed-environmental-sources",
                        feature = "installed-body-storage",
                        target_os = "linux"
                    ))]
                    recording,
                    host,
                    result,
                    output: output.0,
                    #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
                    native,
                }
            })
            .expect("failure to launch the only Body Play worker stops the owner service");
        Ok(RunWorker {
            control,
            started: started_rx,
            acknowledge: Some(ack_tx),
            thread: Some(thread),
            #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
            returned_equipment: None,
            #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
            live_fore,
            #[cfg(all(feature = "installed-environmental-sources", target_os = "linux"))]
            fore_refusal: None,
        })
    }
}
