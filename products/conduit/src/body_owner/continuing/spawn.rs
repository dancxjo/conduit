//! The single bounded installed Body worker and its admitted return routes.
use super::*;
impl Owner {
    pub(super) fn spawn_prepared_worker(
        &mut self,
        root: &Path,
        maximum_millis: u64,
        has_fore: bool,
        creche_binding: Option<conduit::creche_encounter::CrecheBodyBinding>,
        ingress: Option<ImageIngress>,
    ) -> Result<RunWorker, String> {
        self.persist(root)?;
        state::require_execution_capacity(root)?;
        let proposed = self
            .session
            .realization()
            .expect("Plan just committed")
            .clone();
        let has_runtime_face =
            proposed
                .plan
                .plots
                .iter()
                .flat_map(|plot| &plot.plan.fragments)
                .flat_map(|fragment| &fragment.placements)
                .any(|placement| {
                    matches!(placement.implementation_id.as_str(),
                    conduit_std_host::measurement_chart_offer::MEASUREMENT_CHART_FACE_IMPLEMENTATION
                    | conduit_std_offers::MEASUREMENT_CHART_FLOW_FACE_IMPLEMENTATION)
                });
        let (mut face_sender, face_receiver) = super::super::runtime_face::channel();
        let mut creche = self.take_creche_execution(creche_binding);
        let creche_custody = creche.as_ref().map(|run| run.custody.clone());
        let (creche_tx, creche_rx) = mpsc::sync_channel(1);
        let host = self.host.take_for_play()?;
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
                let mut fore = ReceiptFore::new();
                let mut timer = DeadlineTimer {
                    until: Instant::now() + Duration::from_millis(maximum_millis),
                    control: worker_control.clone(),
                };
                let result = thread::scope(|scope| {
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
                    let result = match creche.as_mut() {
                        Some(run) => run
                            .run(
                                &mut host,
                                request,
                                &mut output,
                                &mut timer,
                                &creche_tx,
                                started,
                            ),
                        None if has_runtime_face && host.image_identity().is_some() => {
                            if let Some(ingress) = &ingress {
                                host.verify_selected_image_fore_input(&ingress.input)?;
                            }
                            let inputs = ingress.as_ref().map(|i| std::slice::from_ref(i.input.input())).unwrap_or(&[]);
                            host.run_selected_image_body_plan_with_face_to_with_start(
                                request, inputs, false, &mut fore, &mut face_sender,
                                &mut output, &mut timer, started,
                            ).map(RunReport::Image)
                        }
                        None if has_runtime_face => host.run_body_plan_with_fore_and_face_to_with_start(
                            request, &[], false, &mut fore, &mut face_sender,
                            &mut output, &mut timer, started,
                        ).map(RunReport::Reference),
                        None if ingress.is_some() => host
                            .run_selected_image_body_plan_with_retained_input_to_with_start(
                                request, &ingress.as_ref().expect("selected ingress").input,
                                &mut output, &mut timer, &mut fore, started,
                            ).map(RunReport::Image),
                        None if host.image_identity().is_some() => host
                            .run_selected_image_body_plan_with_output_to_with_start(
                                request,
                                &mut output,
                                &mut timer,
                                has_fore.then_some(&mut fore as &mut dyn conduit_std_host::body_execution::BodyForeOutputAdapter),
                                started,
                            )
                            .map(RunReport::Image),
                        None if has_fore => host.run_body_plan_with_fore_to_with_start(
                            request, &[], false, &mut fore, &mut output, &mut timer, started,
                        ).map(RunReport::Reference),
                        None => host
                            .run_body_plan_to_with_start(request, &mut output, &mut timer, started)
                            .map(RunReport::Reference),
                    };
                    let _ = finished.send(());
                    result
                });
                (host, result, output.0, fore, creche.map(|run| run.factory), ingress)
            })
            .expect("failure to launch the only Body Play worker stops the owner service");
        Ok(RunWorker {
            control,
            runtime_faces: face_receiver,
            started: started_rx,
            acknowledge: Some(ack_tx),
            thread: Some(thread),
            creche_controller: creche_rx,
            creche_custody,
        })
    }
}
