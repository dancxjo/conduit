//! Standing timer/observer/facets run in one admitted IMAGE past finite defaults.
use super::*;
use std::sync::atomic::Ordering;
struct ResidentCapture {
    current: [Option<CommittedFaceContribution>; 2],
    counts: [usize; 2],
    control: RunControl,
    pressure: [bool; 2],
    stopped: bool,
    pressures: usize,
}
impl BodyFaceContributionObserver for ResidentCapture {
    fn contribute(
        &mut self,
        frame: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        let index = match frame.fragment().subjects[0].name.as_str() {
            "Wallpaper" => 0,
            "Clock and weather" => 1,
            _ => panic!("unexpected contributor"),
        };
        if self.counts[index].is_multiple_of(64) && !self.pressure[index] {
            self.pressure[index] = true;
            self.pressures += 1;
            return Err(ReturnedFaceContribution {
                contribution: frame,
                refusal: crate::body_execution::FaceContributionRefusal::Pressure,
            });
        }
        self.pressure[index] = false;
        if let Some(old) = &self.current[index] {
            assert_eq!(old.fragment().subjects, frame.fragment().subjects);
            assert_eq!(old.fragment().basis, frame.fragment().basis);
        }
        assert!(!frame.fragment().subjects.iter().any(|s| s.name == "Home"));
        self.current[index] = Some(frame);
        self.counts[index] += 1;
        if !self.stopped && self.counts.iter().all(|count| *count >= 1442) {
            self.stopped = true;
            self.control
                .request_stop(crate::RunControlRequestId::new("resident/explicit-stop").unwrap())
                .unwrap();
        }
        Ok(())
    }
}
#[test]
fn resident_two_facets_keep_one_play_and_bounded_storage_past_1440_observations() {
    let mut host = common::resident_host(64 * 1024 * 1024);
    let (plan, wake) = common::planned_resident(&host);
    let placements = plan
        .plots
        .iter()
        .flat_map(|p| &p.plan.fragments)
        .flat_map(|f| &f.placements)
        .collect::<Vec<_>>();
    assert_eq!(placements.len(), 4);
    assert!(placements.iter().all(|p| {
        !p.configuration
            .iter()
            .any(|c| c.key == "maximum-updates" || c.key == "count")
    }));
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let control = RunControl::default();
    let starts = Cell::new(0);
    let mut capture = ResidentCapture {
        current: [None, None],
        counts: [0, 0],
        control: control.clone(),
        pressure: [false; 2],
        stopped: false,
        pressures: 0,
    };
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &[],
            false,
            &mut NoOutput,
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                starts.set(starts.get() + 1);
                Ok(())
            },
        )
        .unwrap();
    assert!(
        result.report().failure.is_none(),
        "{:?}",
        result.report().failure
    );
    assert!(matches!(
        result.report().terminal,
        conduit_core::TerminalDisposition::Cancelled { .. }
    ));
    assert_eq!(starts.get(), 1);
    assert!(capture.counts.iter().all(|count| *count >= 1442));
    assert!(capture.pressures > 2);
    let first = capture.current[0].as_ref().unwrap();
    let second = capture.current[1].as_ref().unwrap();
    assert_eq!(first.fragment().basis, second.fragment().basis);
    assert_ne!(
        first.fragment().subjects[0].identity,
        second.fragment().subjects[0].identity
    );
    drop(result);
    assert!(memory.load(Ordering::Acquire) > baseline);
    drop(capture);
    assert_eq!(memory.load(Ordering::Acquire), baseline);
}
