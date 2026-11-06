use super::*;
use crate::{machine::BaseError, pcm_output::PcmProvider};

struct Output {
    calls: usize,
    fail: bool,
    generation: u64,
}
impl PcmOutput for Output {
    fn provider(&self) -> PcmProvider {
        PcmProvider {
            base_id: "test/pcm",
            provider_generation: self.generation,
            sample_rate: 48_000,
            maximum_frames: 57_600,
        }
    }
    fn play(&mut self, samples: &[i16]) -> Result<(), BaseError> {
        self.calls += 1;
        assert_eq!(samples.len(), 57_600);
        assert!(samples.iter().any(|sample| *sample != 0));
        if self.fail {
            Err(BaseError::Unavailable)
        } else {
            Ok(())
        }
    }
}

#[test]
fn shipped_plot_runs_once_through_a_started_body_and_the_pcm_provider() {
    let (ids, host) = crate::native_workset::tests::fixture();
    let mut output = Output {
        calls: 0,
        fail: false,
        generation: 1,
    };
    let mut plot = StartupPlot::prepare(&ids, &host, "build", Some(output.provider())).unwrap();
    assert!(plot.biography().validate().is_ok());
    let allocations = crate::test_allocations::allocations(|| {
        let receipt = plot.run(Some(&mut output)).unwrap();
        assert_eq!(receipt.outcome, "completed");
        assert_eq!(receipt.frames, 57_600);
    });
    assert_eq!(allocations, 0);
    assert_eq!(output.calls, 1);
    assert_eq!(
        plot.run(Some(&mut output)).err(),
        Some("startup-play-already-finished")
    );
    assert_eq!(output.calls, 1);
}

#[test]
fn missing_output_failure_and_stale_provider_complete_the_optional_sink_without_replay() {
    let (ids, host) = crate::native_workset::tests::fixture();
    let mut absent = StartupPlot::prepare(&ids, &host, "build", None).unwrap();
    assert_eq!(absent.run(None).unwrap().outcome, "denied");
    let mut output = Output {
        calls: 0,
        fail: true,
        generation: 1,
    };
    let mut failed = StartupPlot::prepare(&ids, &host, "build", Some(output.provider())).unwrap();
    assert_eq!(failed.run(Some(&mut output)).unwrap().outcome, "failed");
    assert_eq!(output.calls, 1);
    let mut stale = StartupPlot::prepare(&ids, &host, "build", Some(output.provider())).unwrap();
    output.generation = 2;
    assert_eq!(stale.run(Some(&mut output)).unwrap().outcome, "denied");
    assert_eq!(output.calls, 1);
}
