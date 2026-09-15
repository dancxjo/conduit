use super::*;
use conduit_audio::{PcmChannelLayout, PcmFrameHeader};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex, MutexGuard,
};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static PROVIDER_PROCESS: Mutex<()> = Mutex::new(());

fn provider_process() -> MutexGuard<'static, ()> {
    PROVIDER_PROCESS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Fixture {
    root: PathBuf,
    executable: PathBuf,
    model: PathBuf,
    config: PathBuf,
}

impl Fixture {
    fn new(script: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "conduit-piper-provider-{}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test"),
            FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        let executable = root.join("piper-fixture");
        let model = root.join("voice.onnx");
        let config = root.join("voice.onnx.json");
        fs::write(&executable, script).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(&model, b"bounded model fixture").unwrap();
        fs::write(&config, br#"{"audio":{"sample_rate":22050}}"#).unwrap();
        Self {
            root,
            executable,
            model,
            config,
        }
    }

    fn adapter(&self, maximum_frames: u32) -> PiperSpeechAdapter {
        PiperDiscovery::inspect(&self.executable, &self.model, &self.config, None)
            .unwrap()
            .initialize(PiperLimits {
                maximum_text_bytes: 256,
                maximum_frames,
                maximum_blocks: 16,
                timeout: Duration::from_secs(2),
            })
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn bounded_process_output_becomes_ordered_canonical_pcm_blocks() {
    let _provider_process = provider_process();
    let fixture = Fixture::new(
        "#!/bin/sh\ncat >/dev/null\nprintf '\\001\\000\\002\\000\\003\\000'\nprintf 'fixture diagnostic' >&2\n",
    );
    let discovery =
        PiperDiscovery::inspect(&fixture.executable, &fixture.model, &fixture.config, None)
            .unwrap();
    assert_eq!(discovery.sample_rate_hz, 22_050);
    assert_eq!(discovery.model_bytes, 21);
    assert_eq!(discovery.executable_sha256.len(), 64);
    let mut adapter = discovery
        .initialize(PiperLimits {
            maximum_text_bytes: 256,
            maximum_frames: 8,
            maximum_blocks: 2,
            timeout: Duration::from_secs(2),
        })
        .unwrap();
    let mut observed = Vec::new();
    let receipt = adapter
        .synthesize(
            "Rosehip House",
            || false,
            |block| {
                let (header, payload) = PcmFrameHeader::decode_frame(block).unwrap();
                observed.push((header, payload.to_vec()));
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(receipt.frames, 3);
    assert_eq!(receipt.blocks, 1);
    assert_eq!(receipt.diagnostic_bytes, 18);
    assert_eq!(observed[0].0.sample_rate_hz, 22_050);
    assert_eq!(observed[0].0.layout, PcmChannelLayout::Mono);
    assert_eq!(observed[0].0.start_frame, 0);
    assert_eq!(observed[0].1, [1, 0, 2, 0, 3, 0]);
}

#[test]
fn provider_receives_end_of_text_before_output_is_polled() {
    let _provider_process = provider_process();
    let fixture = Fixture::new("#!/bin/sh\ncat >/dev/null\nprintf '\\001\\000'\n");
    let mut adapter = fixture.adapter(8);
    let receipt = adapter.synthesize("Rosehip", || false, |_| Ok(())).unwrap();
    assert_eq!(receipt.frames, 1);
}

#[test]
fn resumable_session_exposes_one_block_per_pull_and_can_abort() {
    let _provider_process = provider_process();
    let fixture =
        Fixture::new("#!/bin/sh\ncat >/dev/null\ndd if=/dev/zero bs=1 count=100 2>/dev/null\n");
    let mut adapter = fixture.adapter(50);

    adapter.begin("Rosehip").unwrap();
    assert_eq!(
        adapter.begin("another utterance"),
        Err(PiperFailure::ProviderBusy)
    );
    let first = match adapter.next(|| false).unwrap() {
        PiperSynthesisStep::Block(block) => {
            let (header, payload) = PcmFrameHeader::decode_frame(block).unwrap();
            assert_eq!(header.start_frame, 0);
            assert_eq!(header.frame_count, 25);
            payload.len()
        }
        PiperSynthesisStep::Complete(_) => panic!("completed before yielding the first block"),
    };
    assert_eq!(first, 50);
    match adapter.next(|| false).unwrap() {
        PiperSynthesisStep::Block(block) => {
            let (header, payload) = PcmFrameHeader::decode_frame(block).unwrap();
            assert_eq!(header.start_frame, 25);
            assert_eq!(payload.len(), 50);
        }
        PiperSynthesisStep::Complete(_) => panic!("completed before yielding the second block"),
    }
    let receipt = match adapter.next(|| false).unwrap() {
        PiperSynthesisStep::Complete(receipt) => receipt,
        PiperSynthesisStep::Block(_) => panic!("yielded an unexpected third block"),
    };
    assert_eq!((receipt.frames, receipt.blocks), (50, 2));

    adapter.begin("restart").unwrap();
    adapter.abort();
    assert_eq!(adapter.next(|| false), Err(PiperFailure::NoActiveSynthesis));
    adapter.begin("after abort").unwrap();
    adapter.abort();
}

#[test]
fn text_output_and_consumer_bounds_fail_distinctly() {
    let _provider_process = provider_process();
    let fixture =
        Fixture::new("#!/bin/sh\ncat >/dev/null\nprintf '\\001\\000\\002\\000\\003\\000'\n");
    let mut adapter = fixture.adapter(2);
    assert_eq!(
        adapter.synthesize("Rosehip", || false, |_| Ok(())),
        Err(PiperFailure::OutputOverflow)
    );
    let mut adapter = fixture.adapter(8);
    assert_eq!(
        adapter.synthesize("Rosehip", || false, |_| Err(())),
        Err(PiperFailure::ConsumerPressure)
    );
    assert_eq!(
        adapter.synthesize("", || false, |_| Ok(())),
        Err(PiperFailure::EmptyText)
    );
}

#[test]
fn cancellation_and_provider_failure_are_not_completion() {
    let _provider_process = provider_process();
    let fixture = Fixture::new("#!/bin/sh\nsleep 2\n");
    let mut adapter = fixture.adapter(8);
    assert_eq!(
        adapter.synthesize("Rosehip", || true, |_| Ok(())),
        Err(PiperFailure::Cancelled)
    );
    let fixture = Fixture::new("#!/bin/sh\nexit 7\n");
    let mut adapter = fixture.adapter(8);
    assert_eq!(
        adapter.synthesize("Rosehip", || false, |_| Ok(())),
        Err(PiperFailure::ProviderLost)
    );
}

#[test]
fn timeout_and_partial_sample_are_distinct_failures() {
    let _provider_process = provider_process();
    let fixture = Fixture::new("#!/bin/sh\nsleep 2\n");
    let mut adapter =
        PiperDiscovery::inspect(&fixture.executable, &fixture.model, &fixture.config, None)
            .unwrap()
            .initialize(PiperLimits {
                maximum_text_bytes: 256,
                maximum_frames: 8,
                maximum_blocks: 8,
                timeout: Duration::from_millis(500),
            })
            .unwrap();
    assert_eq!(
        adapter.synthesize("Rosehip", || false, |_| Ok(())),
        Err(PiperFailure::Timeout)
    );

    let fixture = Fixture::new("#!/bin/sh\ncat >/dev/null\nprintf '\\001'\n");
    let mut adapter = fixture.adapter(8);
    assert_eq!(
        adapter.synthesize("Rosehip", || false, |_| Ok(())),
        Err(PiperFailure::MalformedPcm)
    );
}
