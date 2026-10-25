//! CPU diagnostic: real checked Mask, compositor, typed native input and adapter
//! custody. The pixel target is explicitly software; no SDL/DRM/physical proof.
use super::*;
use crate::creche_encounter::CrecheEncounterAdapter;
use conduit_human::{KeyModifiers, KeyTransition};
use conduit_std_host::hosted_creche::{CrecheHost, CrecheLimits};
use conduitos::display::{DisplayError, DisplayFormat};

struct CpuOutput {
    writes: u64,
    commits: u32,
    revoked: bool,
}
impl PixelTarget for CpuOutput {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 640,
            height: 480,
            pitch: 2560,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
        if self.revoked {
            return Err(DisplayError::Lost);
        }
        self.writes += 1;
        Ok(())
    }
    fn commit_frame(&mut self) -> Result<(), DisplayError> {
        if self.revoked {
            return Err(DisplayError::Lost);
        }
        self.commits += 1;
        Ok(())
    }
}
impl LocalCrecheScanout for CpuOutput {
    fn revoke_and_drain(&mut self) -> Result<(), String> {
        self.revoked = true;
        Ok(())
    }
}
#[test]
fn real_native_owner_redraws_acknowledges_and_returns_exact_input_to_encounter() {
    let host = conduit_std_host::StdHost::new().advertisement().clone();
    // Explicit diagnostic CPU surface observation. This is not an SDL provider
    // or a statement that this host owns firmware-mapped physical scanout.
    let basis = conduit_observatory::FramebufferBasis {
        base_id: "diagnostic/cpu-output".into(),
        width: 640,
        height: 480,
        pitch_bytes: 2560,
        bits_per_pixel: 32,
    };
    let bases = conduitos::product_bases::NativeProductBases::observe_framebuffer(&basis).unwrap();
    let provider = bases.framebuffer_provider([17; 32]).unwrap();
    let mask = NativeFaceMask::prepare(
        host.host_id.clone(),
        host.boot_id.clone(),
        host.offer_generation,
        "diagnostic",
        provider.entry.base_id.clone(),
        "diagnostic/creche",
        &provider,
    )
    .unwrap();
    let (mut native, custody) = LocalCrecheNativeOwner::new(
        mask,
        CpuOutput {
            writes: 0,
            commits: 0,
            revoked: false,
        },
        LocalCrecheNativeLimits {
            maximum_presentations: 16,
            maximum_input_events: 1024,
            retirement_timeout: Duration::from_millis(100),
        },
    )
    .unwrap();
    let draft = crate::creche_draft_fixture::encounter("Garden");
    let profiles = draft
        .construction()
        .hosts()
        .iter()
        .map(|h| h.configuration.clone())
        .collect();
    let (mut adapter, controller) = CrecheEncounterAdapter::new(
        draft,
        profiles,
        "diagnostic/native-creche".into(),
        "diagnostic/creche-slot".into(),
        CrecheLimits {
            maximum_evidence_bytes: 4096,
            maximum_polls: 128,
            minimum_poll_interval_ms: 1,
        },
        32,
        custody,
    )
    .unwrap();
    let (host, plan, play) = crate::creche_encounter::tests::plan_on_host(&adapter, host);
    let mut owner = CrecheHost::prepare(&mut adapter, &host, &plan, &play).unwrap();
    owner.confirm_started(&host, &play).unwrap();
    native.bind_execution(owner.context(), &play).unwrap();
    owner.begin(owner.request()).unwrap();
    assert!(native.present_current(&controller).unwrap());
    let original = controller.face().unwrap().unwrap();
    assert!(native.display.writes > 0 && native.display.commits > 0);
    let commits = native.display.commits;
    let tab = KeyEvent::new(43, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    let enter = KeyEvent::new(40, KeyTransition::Pressed, KeyModifiers::NONE).unwrap();
    let mut reached = false;
    for _ in 0..64 {
        assert!(!native.key(&controller, tab).unwrap());
        let scene = native.mask.scene().unwrap();
        if scene
            .focused()
            .is_some_and(|control| original.actions[control.action].identity == "creche.suggest")
        {
            reached = true;
            break;
        }
    }
    assert!(
        reached,
        "bounded native navigation reaches the suggestion action"
    );
    assert!(
        native.display.commits > commits,
        "focus reading crossed actual commit hook"
    );
    assert!(
        native.key(&controller, enter).unwrap(),
        "Mask typed return must match controller acknowledged Show after redraw"
    );
    assert!(native.shared.has_returned().unwrap());
    assert!(
        native.key(&controller, tab).is_err(),
        "one queued returned occurrence retains input pressure"
    );
    std::thread::sleep(owner.poll_wait());
    owner.poll().unwrap();
    assert_ne!(
        controller.face().unwrap().unwrap().identity,
        original.identity
    );
    assert!(native.present_current(&controller).unwrap());
    assert!(
        !native.shared.has_returned().unwrap(),
        "replacement Face retires the consumed occurrence"
    );
    native.retire().unwrap();
    owner.retire().unwrap();
    assert!(native.display.revoked);
    assert!(native.mask.show().is_none() && native.mask.render_receipt().is_none());
}
