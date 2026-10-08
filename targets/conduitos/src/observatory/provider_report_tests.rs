use super::*;
use crate::{identity::BootIdentities, offer::CpuFeatures};

fn fixture() -> (HostOffer<'static>, HostAdvertisement) {
    let ids = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &ids,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        1_048_576,
    );
    let advertisement = crate::ordinary_plan::advertisement(&ids, &offer, "build").unwrap();
    (offer, advertisement)
}

#[test]
fn current_timer_clock_pair_enriches_one_report_without_duplicate_provider_truth() {
    let (offer, advertisement) = fixture();
    let pair = crate::ordinary_base::timer_clock_provider(&offer).unwrap();
    let mut reports = fixed_base_reports(&offer, &advertisement).unwrap();
    append_advertised_bases(&mut reports, &advertisement);
    let timers: Vec<_> = reports
        .iter()
        .filter(|report| report.base_id.as_str() == hex(&pair.base_id))
        .collect();
    assert_eq!(timers.len(), 1);
    assert_eq!(
        timers[0].provider_instance_id.as_str(),
        hex(&pair.provider_instance_id)
    );
    assert_eq!(timers[0].provider_generation, pair.provider_generation);
    assert_eq!(
        timers[0].implementation_id.as_ref().unwrap().as_str(),
        crate::ordinary_base::TIMER_PROVIDER_IMPLEMENTATION
    );
}

#[test]
fn timer_report_refuses_stale_clock_timer_and_forged_advertised_instances() {
    for kind in [BaseKind::Clock, BaseKind::Timer] {
        let (mut offer, advertisement) = fixture();
        offer
            .bases
            .iter_mut()
            .find(|base| base.kind == kind)
            .unwrap()
            .provider_generation += 1;
        assert!(matches!(
            fixed_base_reports(&offer, &advertisement),
            Err(ExportError::InvalidSnapshot)
        ));
    }
    let (offer, mut advertisement) = fixture();
    advertisement
        .bases
        .iter_mut()
        .find(|base| {
            base.implementation_id.as_str() == crate::ordinary_base::TIMER_PROVIDER_IMPLEMENTATION
        })
        .unwrap()
        .provider_instance_id = "forged-provider".into();
    assert!(matches!(
        fixed_base_reports(&offer, &advertisement),
        Err(ExportError::InvalidSnapshot)
    ));
}

#[test]
fn contradictory_serial_provider_remains_visible_for_snapshot_validation_to_refuse() {
    let (offer, mut advertisement) = fixture();
    let serial = advertisement
        .bases
        .iter_mut()
        .find(|base| {
            base.implementation_id.as_str() == crate::ordinary_base::SERIAL_PROVIDER_IMPLEMENTATION
        })
        .unwrap();
    let id = serial.base_id.clone();
    serial.provider_instance_id = "forged-serial".into();
    let mut reports = fixed_base_reports(&offer, &advertisement).unwrap();
    append_advertised_bases(&mut reports, &advertisement);
    assert_eq!(
        reports.iter().filter(|report| report.base_id == id).count(),
        2
    );
}
