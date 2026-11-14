//! Original Root control and complete native preparation, independent of the
//! board carrier. Structured external Fore/Mask custody is a separate required
//! service; until installed this entrance never admits or starts the workset.
pub use crate::neutral_preflight::{preflight, Runtime};
use crate::{
    neutral_authority::{Issuers, Operations},
    neutral_equipment::Equipment,
    neutral_workset::{layout, source},
};
use conduit_i2c_transaction::I2cProvider;
use conduit_mcu_host_runtime::{
    execution::Policy,
    root::{Refusal, Request, Root},
    transport::DocumentTransport,
};
use conduit_monotonic_wait::clock_at::owner::MonotonicDeadlineProvider;
#[allow(clippy::too_many_arguments)]
pub async fn serve<I: I2cProvider, D: MonotonicDeadlineProvider, T: DocumentTransport>(
    root: &mut Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    equipment: &mut Equipment<I, D>,
    residents: &mut crate::resident::generated::Initialized,
    link: &mut T,
    runtime: Runtime,
    maximum_bytes: usize,
) -> ! {
    let mut issuers = Issuers::new();
    let operations = Operations::selected().expect("checked finite native authority");
    let mut response = [0; 4096];
    let mut last = "not-prepared";
    loop {
        let result = if root.retained_input() {
            // Retained command backing is immutable; inspection uses separate
            // caller-funded bytes and cannot replace or replay that command.
            let mut request = [0; 128];
            match link.receive(&mut request).await {
                Ok(b"?") => inspect(root, equipment, link, last).await,
                Ok(_) => link
                    .send(b"ROOT_REFUSAL@1 structured-fore-mask-unavailable not-playing")
                    .await
                    .map_err(|_| Refusal::Carrier),
                Err(_) => Err(Refusal::Carrier),
            }
        } else {
            match root.receive_request(link).await {
                Ok(Request::Inspect) => inspect(root, equipment, link, last).await,
                Ok(Request::Spawn) => root.spawn(link, &mut response).await,
                Ok(Request::Return) => {
                    let policy = Policy {
                        preparation: root.configuration().preparation,
                        execution: root.configuration().execution,
                        active_worksets: 0,
                        maximum_retained_bytes: maximum_bytes,
                    };
                    let authorized = root.return_context((runtime.now)()).and_then(|context| {
                        issuers
                            .authorize(context, policy, equipment, &operations, maximum_bytes)
                            .map_err(|_| Refusal::Scope)
                    });
                    match authorized {
                        Err(e) => Err(e),
                        Ok(()) => {
                            let images = residents
                                .offers()
                                .into_iter()
                                .flatten()
                                .chain([
                                    crate::neutral_workset::native_images::I2C_IMAGE,
                                    crate::neutral_workset::native_images::CLOCK_AT_IMAGE,
                                ])
                                .chain(source::OFFER_IMAGES.iter().copied());
                            root.return_member(
                                link,
                                images,
                                || {
                                    issuers.copy_grants(maximum_bytes).map_err(|_| {
                                        conduit_mcu_host_runtime::member_return::Refusal::Scope
                                    })
                                },
                                &mut response,
                            )
                            .await
                        }
                    }
                }
                Ok(Request::Prepare) => {
                    let received = embassy_time::with_timeout(
                        embassy_time::Duration::from_secs(10),
                        root.receive_preparation(link, (runtime.challenge)(), runtime.now),
                    )
                    .await;
                    match received {
                        Ok(Ok(())) => {
                            last = match preflight(
                                root,
                                equipment,
                                residents,
                                &mut issuers,
                                &operations,
                                &runtime,
                                maximum_bytes,
                            ) {
                                Ok(()) => "prepared-dropped-structured-fore-mask-unavailable",
                                Err(reason) => reason,
                            };
                            inspect(root, equipment, link, last).await
                        }
                        Ok(Err(e)) => Err(e),
                        Err(_) => {
                            root.clear_unretained_input();
                            Err(Refusal::Preparation)
                        }
                    }
                }
                Ok(Request::InstallContinuation) => Err(Refusal::Preparation),
                Err(e) => Err(e),
            }
        };
        crate::membership::observe_receipt(root.receipt_digest());
        if result.is_err() {
            embassy_time::Timer::after_millis(10).await;
        }
    }
}
async fn inspect<I, D, T: DocumentTransport>(
    root: &Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    equipment: &Equipment<I, D>,
    link: &mut T,
    last: &str,
) -> Result<(), Refusal> {
    use core::fmt::Write;
    let mut status = heapless::String::<512>::new();
    let _=write!(status,"ROOT_INSPECTION@1 unsigned current_member={} i2c_ready={} clock_ready={} retained_input={} phase=not-started fore=unsupported mask=unsupported preparation={} source={}",
        root.has_current_member(),equipment.bus.is_some(),equipment.clock.is_some(),root.retained_input(),last,layout::SOURCE_DOCUMENT_ID);
    link.send(status.as_bytes())
        .await
        .map_err(|_| Refusal::Carrier)
}
