use conduit_core::{
    BodyClockCorrelation, BodyClockRateEstimator, BodyTimeQuality, BodyTimeRelation,
    BodyTimeRequirement, BodyTimeTolerance, BodyTimeTracker, BootId, ClockProvenance, HostId,
    MonotonicClockIdentity, MonotonicDuration, MonotonicInstant, PeerClockExchange,
    PeerClockPolicy, TemporalScale,
};
use std::io::{self, BufRead, Write};
use std::time::Instant;

fn sample(host: &str, boot: &str, basis: &str, ticks: u64) -> MonotonicInstant {
    MonotonicInstant::new(
        ticks,
        MonotonicClockIdentity::new(
            host.into(),
            boot.into(),
            basis.into(),
            TemporalScale::Microseconds,
            1,
            1,
        )
        .unwrap(),
    )
    .unwrap()
}

fn elapsed_micros(epoch: Instant) -> u64 {
    u64::try_from(epoch.elapsed().as_micros()).unwrap()
}

fn main() {
    let epoch = Instant::now();
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    let mut line = String::new();
    let mut tracker: Option<BodyTimeTracker> = None;
    let mut estimator: Option<BodyClockRateEstimator> = None;
    writeln!(output, "READY").unwrap();
    output.flush().unwrap();
    for round in 0..2 {
        line.clear();
        input.read_line(&mut line).unwrap();
        let mut fields = line.split_ascii_whitespace();
        assert_eq!(fields.next(), Some("PROBE"));
        let browser_send = fields.next().unwrap().parse::<u64>().unwrap();
        let browser_basis = fields.next().unwrap().to_string();
        assert_eq!(fields.next(), None);
        let native_receive = elapsed_micros(epoch);
        let native_send = elapsed_micros(epoch);
        writeln!(output, "SAMPLES {native_receive} {native_send}").unwrap();
        output.flush().unwrap();

        line.clear();
        input.read_line(&mut line).unwrap();
        let mut fields = line.split_ascii_whitespace();
        assert_eq!(fields.next(), Some("COMPLETE"));
        let browser_receive = fields.next().unwrap().parse::<u64>().unwrap();
        assert_eq!(fields.next(), None);
        let peer_receive = sample("host/std", "boot/std", "std/process-epoch", native_receive);
        let peer_send = sample("host/std", "boot/std", "std/process-epoch", native_send);
        let peer_correlation = BodyClockCorrelation::new(
            "body/heterogeneous".into(),
            TemporalScale::Microseconds,
            1,
            peer_receive.clone(),
            1_000_000 + native_receive,
            0,
            100,
            10_000,
            5_000_000,
            ClockProvenance::External {
                provider_id: "std/process-epoch".into(),
                admission_reference: "fixture/native-process".into(),
                policy_id: "fixture/browser-native".into(),
            },
        )
        .unwrap();
        let local_send = sample("host/browser", "boot/browser", &browser_basis, browser_send);
        let local_receive = sample(
            "host/browser",
            "boot/browser",
            &browser_basis,
            browser_receive,
        );
        let exchange = PeerClockExchange {
            local_send,
            peer_receive,
            peer_send,
            local_receive: local_receive.clone(),
        };
        let policy = PeerClockPolicy {
            peer_host: HostId::from("host/std"),
            peer_boot: BootId::from("boot/std"),
            body_basis: "body/heterogeneous".into(),
            minimum_generation: 1,
            maximum_round_trip: MonotonicDuration::new(2_000_000, TemporalScale::Microseconds),
            maximum_local_rate_error_ppm: 1_000,
            correlation_horizon: MonotonicDuration::new(5_000_000, TemporalScale::Microseconds),
            admission_id: "fixture/native-process".into(),
            policy_id: "fixture/browser-native".into(),
            membership_revision: 1,
            observation_sequence: round + 1,
        };
        let candidate = exchange
            .derive_correlation(&peer_correlation, &policy, round + 2)
            .unwrap();
        let estimate = if let (Some(tracker), Some(estimator)) = (&mut tracker, &mut estimator) {
            let refined = estimator.refine(&candidate, 1_000).unwrap();
            let estimate = tracker
                .reconcile(refined.clone(), &local_receive)
                .unwrap()
                .clone();
            estimator.record(refined).unwrap();
            estimate
        } else {
            let estimate = candidate.project(&local_receive).unwrap();
            tracker = Some(BodyTimeTracker::new(candidate.clone(), &local_receive).unwrap());
            estimator = Some(BodyClockRateEstimator::new(candidate).unwrap());
            estimate
        };
        assert!(matches!(
            &estimate.provenance,
            ClockProvenance::Peer { host_id, boot_id, .. }
                if host_id.as_str() == "host/std" && boot_id.as_str() == "boot/std"
        ));
        let native_estimate = peer_correlation.project(&exchange.peer_send).unwrap();
        assert_eq!(
            estimate.physical_relation(&native_estimate),
            Ok(BodyTimeRelation::Indeterminate)
        );
        let requirement = BodyTimeRequirement::new(
            "body/heterogeneous".into(),
            BodyTimeTolerance::new(500_000, TemporalScale::Microseconds),
            MonotonicDuration::new(1_000_000, TemporalScale::Microseconds),
        )
        .unwrap();
        assert!(matches!(
            requirement.assess(tracker.as_ref().unwrap().correlation(), &local_receive),
            BodyTimeQuality::Ready { .. }
        ));
        writeln!(
            output,
            "ESTIMATE {} {} {} {} {} {} {} {}",
            estimate.generation,
            estimate.center_ticks,
            estimate.earliest_ticks,
            estimate.latest_ticks,
            estimate.correlation_age_ticks,
            estimate.local_sample.clock().host_id().as_str(),
            estimate.local_sample.clock().boot_id().as_str(),
            estimate.local_sample.clock().basis_id(),
        )
        .unwrap();
        output.flush().unwrap();
    }
}
