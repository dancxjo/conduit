use super::*;
use crate::ssd1306_frame::tests::presentation;
use conduit_ssd1306::{I2cBaseAvailability, I2cProviderFailure};

struct Provider {
    available: bool,
    fail_at: Option<usize>,
    writes: usize,
}

impl Ssd1306I2cProvider for Provider {
    fn availability(&self) -> I2cBaseAvailability {
        if self.available {
            I2cBaseAvailability::Available
        } else {
            I2cBaseAvailability::Unavailable
        }
    }
    fn write(&mut self, _address: u8, _bytes: &[u8]) -> Result<(), I2cProviderFailure> {
        if self.fail_at == Some(self.writes) {
            return Err(I2cProviderFailure::Write);
        }
        self.writes += 1;
        Ok(())
    }
}

fn evidence() -> Ssd1306PresenterEvidence {
    Ssd1306PresenterEvidence {
        host_id: HostId::from("host/pico"),
        boot_id: BootId::from("boot/pico"),
        offer_generation: OfferGeneration(1),
        i2c_base_id: "base/i2c".into(),
        attachment_id: "attachment/oled".into(),
        framebuffer_resource_id: "framebuffer/oled".into(),
        address: 0x3c,
        observed_at_tick: 10,
        maximum_age_ticks: 100,
    }
}

fn provider() -> Provider {
    Provider {
        available: true,
        fail_at: None,
        writes: 0,
    }
}

#[test]
fn exact_plan_and_successful_manifestation_retain_portable_identity() {
    let evidence = evidence();
    let mut presenter = Ssd1306Presenter::prepare(evidence.clone(), 10).unwrap();
    validate_ssd1306_plan(presenter.plan(), &evidence).unwrap();
    let source = presentation(1);
    let receipt = presenter.present(&source, &mut provider()).unwrap();
    assert_eq!(receipt.presentation_id, source.identity);
    assert_eq!(receipt.lifecycle, ManifestationLifecycle::Available);
    assert_eq!(receipt.device_failure, None);
    assert!(!receipt.motion_safety_mutated);
    assert!(receipt.host_remains_available);
    assert!(!receipt.physical_hil_claimed);
    assert_eq!(
        presenter.present(&source, &mut provider()),
        Err(Ssd1306PresenterError::StaleRevision)
    );
}

#[test]
fn missing_display_is_failed_manifestation_but_not_host_or_safety_failure() {
    let mut presenter = Ssd1306Presenter::prepare(evidence(), 10).unwrap();
    let source = presentation(1);
    let mut missing = provider();
    missing.available = false;
    let receipt = presenter.present(&source, &mut missing).unwrap();
    assert_eq!(receipt.lifecycle, ManifestationLifecycle::Failed);
    assert_eq!(
        receipt.manifestation_failure,
        Some(ManifestationFailure::DeliveryFailed)
    );
    assert_eq!(
        receipt.device_failure,
        Some(Ssd1306Failure::I2cBaseUnavailable)
    );
    assert!(receipt.host_remains_available);
    assert!(!receipt.motion_safety_mutated);
    assert_eq!(source, presentation(1));
}

#[test]
fn stale_attachment_and_wrong_plan_refuse_before_device_io() {
    let evidence = evidence();
    assert!(matches!(
        Ssd1306Presenter::prepare(evidence.clone(), 111),
        Err(Ssd1306PresenterError::StaleEvidence)
    ));
    let presenter = Ssd1306Presenter::prepare(evidence.clone(), 10).unwrap();
    let mut wrong = presenter.plan().clone();
    wrong.fragments[0].placements[0].boot_id = BootId::from("boot/wrong");
    assert_eq!(
        validate_ssd1306_plan(&wrong, &evidence),
        Err(Ssd1306PresenterError::WrongPlan)
    );
}
