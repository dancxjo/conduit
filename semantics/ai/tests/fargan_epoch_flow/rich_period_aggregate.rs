//! Finite allocating numerical projector. Upstream opaque common custody remains
//! required; byte quotas below do not claim a whole evaluator/Source heap bound.
use super::*;

const MAX_EPOCHS: usize = 65535;
const MAX_FRAME: usize = 16384;
const MAX_Q8: usize = 1024;
const MAX_RETAINED: usize = 256 * 1024 * 1024;
// Conservative canonical output allowance for eligible/arithmetic/raw/period/span.
const OUTPUT_PER_EPOCH: usize = 8192;

struct Basis {
    original: Rc<[u8]>,
    /// Complete original trajectory material, never replaced by held targets.
    trajectory: Rc<[u8]>,
    /// Full original revision/effect material retained outside bounded Core facts.
    revision_effect: Rc<[u8]>,
}
struct Query<'a> {
    basis: &'a Rc<Basis>,
    frame: &'a [u8],
    q8: &'a [u8],
    upstream: &'a [u8],
}
struct Receipt {
    basis: Rc<Basis>,
    numerical: PreparedRichPeriod,
    position: Vec<u8>,
    span: Vec<u8>,
}
struct Aggregate {
    basis: Rc<Basis>,
    projector: Rc<RichPeriodProjector>,
    hold_program: String,
    receipts: Vec<Receipt>,
    canonical_retained_bound: usize,
}
enum DeclaredHold {
    Onset10ms,
    Unsupported,
}
impl Aggregate {
    fn prepare(
        basis: &Rc<Basis>,
        queries: &[Query<'_>],
        budget: usize,
        policy: DeclaredHold,
    ) -> Result<Self, String> {
        Self::prepare_window(basis, queries, budget, policy, 0, queries.len())
    }
    /// Explicit slice of one complete admitted finite session; legacy preparation
    /// remains zero-origin. Global frame/order is retained without rebasing.
    fn prepare_window(
        basis: &Rc<Basis>,
        queries: &[Query<'_>],
        budget: usize,
        policy: DeclaredHold,
        first_epoch: usize,
        total_epochs: usize,
    ) -> Result<Self, String> {
        if total_epochs > MAX_EPOCHS
            || first_epoch
                .checked_add(queries.len())
                .is_none_or(|end| end > total_epochs)
        {
            return Err("finite session window".into());
        }
        // Inspect ALL resource/custody/order bounds before parsing Source or
        // allocating per-query receipts. This quota covers retained canonical
        // materials, not AST/Native expansion/evaluator scratch or session heap.
        if !(2..=MAX_EPOCHS).contains(&queries.len())
            || basis.original.is_empty()
            || basis.trajectory.is_empty()
            || basis.revision_effect.is_empty()
            || budget > MAX_RETAINED
        {
            return Err("finite resource profile".into());
        }
        let mut bound = basis
            .original
            .len()
            .checked_add(basis.trajectory.len())
            .and_then(|v| v.checked_add(basis.revision_effect.len()))
            .ok_or("canonical byte overflow")?;
        // Reserve fixed Source program material (actual measured below).
        bound = bound
            .checked_add(128 * 1024)
            .ok_or("program byte overflow")?;
        for query in queries {
            if !Rc::ptr_eq(basis, query.basis)
                || query.frame.len() > MAX_FRAME
                || query.q8.len() > MAX_Q8
                || query.upstream.is_empty()
            {
                return Err("foreign basis or frame resource profile".into());
            }
            for bytes in [
                query.frame.len(),
                query.q8.len(),
                query.upstream.len(),
                OUTPUT_PER_EPOCH,
            ] {
                bound = bound.checked_add(bytes).ok_or("canonical byte overflow")?;
            }
        }
        if bound > budget {
            return Err("canonical retained byte budget".into());
        }
        for (ordinal, query) in queries.iter().enumerate() {
            let frame = StructuredInfoValue::from_canonical_bytes(query.frame)
                .map_err(|e| format!("frame: {e:?}"))?;
            if frame.value_type() != scalar(0).value_type()
                || number(&frame) != (first_epoch + ordinal) as u64 * 160
            {
                return Err("foreign frame/order".into());
            }
        }
        let projector = RichPeriodProjector::prepare(true)?;
        let d = &projector.document;
        interface::admit_retained_session_native(
            d,
            "SpeechLinearPitchTrajectory",
            &basis.trajectory,
        )?;
        let graph = expand_canonical_plot_for_authoring(
            d,
            "ai/fargan-rich-hold-span",
            &ProfileCatalog::new(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let ConfigurationValue::Text(hold_program) =
            &graph.expanded.gears[0].configuration[0].value
        else {
            return Err("hold program".into());
        };
        if hold_program.len() + projector.program.len() > 128 * 1024 {
            return Err("program material quota".into());
        }
        let parsed = PortableExpressionProgram::from_canonical_hex(hold_program)
            .map_err(|e| format!("{e:?}"))?;
        let mut receipts = Vec::with_capacity(queries.len());
        for (ordinal, query) in queries.iter().enumerate() {
            let position = record(
                ty(d, "FarganRichHoldPosition"),
                &[
                    ("epoch_count", scalar(total_epochs as u64)),
                    ("ordinal", scalar((first_epoch + ordinal) as u64)),
                    (
                        "original_query_frame",
                        scalar((first_epoch + ordinal) as u64 * 160),
                    ),
                    (
                        "policy",
                        named(
                            ty(d, "FarganRichHoldPolicy"),
                            match policy {
                                DeclaredHold::Onset10ms => "onset_hold_10ms",
                                DeclaredHold::Unsupported => "unsupported_hold",
                            },
                        ),
                    ),
                ],
            )
            .canonical_bytes()
            .map_err(|e| format!("{e:?}"))?;
            interface::admit_retained_session_native(d, "FarganRichHoldPosition", &position)?;
            let span = parsed.evaluate(&position).map_err(|e| format!("{e:?}"))?;
            interface::admit_retained_session_native(d, "FarganRichHoldSpan", &span)?;
            let numerical = PreparedRichPeriod::prepare_shared(
                &projector,
                query.q8,
                query.frame,
                Rc::clone(&basis.original),
                query.upstream,
                (first_epoch + ordinal) as u64,
                (
                    named(
                        ty(d, "FarganRichPeriodPolicy"),
                        "nearest_whole_sample_ties_up",
                    ),
                    named(
                        ty(d, "FarganRichPeriodCadence"),
                        "epoch_onset_160_frames_at_16000_hz",
                    ),
                ),
            )?;
            let actual = numerical.eligible.len()
                + numerical.arithmetic.len()
                + numerical.raw.len()
                + numerical.period.canonical_bytes().unwrap().len()
                + position.len()
                + span.len();
            if actual > OUTPUT_PER_EPOCH {
                return Err("canonical output allowance".into());
            }
            receipts.push(Receipt {
                basis: Rc::clone(basis),
                numerical,
                position,
                span,
            });
        }
        Ok(Self {
            basis: Rc::clone(basis),
            projector,
            hold_program: hold_program.clone(),
            receipts,
            canonical_retained_bound: bound,
        })
    }
}

#[test]
fn finite_shared_period_aggregate_preserves_and_refuses() {
    let d =
        check_syntax_document(&parse_syntax_document(&source()), &StartupCatalog::new()).unwrap();
    let basis = Rc::new(Basis {
        original: Rc::from(&b"full original common owner material"[..]),
        trajectory: Rc::from(
            record(
                ty(&d, "SpeechLinearPitchTrajectory"),
                &[
                    (
                        "duration",
                        record(
                            ty(&d, "SpeechExactDuration"),
                            &[("numerator_seconds", scalar(1)), ("denominator", scalar(1))],
                        ),
                    ),
                    (
                        "start",
                        record(
                            ty(&d, "SpeechFundamentalCycle"),
                            &[
                                ("numerator_seconds", scalar(1)),
                                ("denominator", scalar(200)),
                            ],
                        ),
                    ),
                    (
                        "end",
                        record(
                            ty(&d, "SpeechFundamentalCycle"),
                            &[
                                ("numerator_seconds", scalar(1)),
                                ("denominator", scalar(300)),
                            ],
                        ),
                    ),
                ],
            )
            .canonical_bytes()
            .unwrap(),
        ),
        revision_effect: Rc::from(&b"complete upstream revision and effect material"[..]),
    });
    let q8: Vec<_> = [(1, 200), (1, 300), (1, 250)]
        .into_iter()
        .map(|(n, den)| {
            let num = n * 16000 * 256;
            fixture(&d, n, den, 16000, num / den, num % den)
                .canonical_bytes()
                .unwrap()
        })
        .collect();
    let frames: Vec<_> = (0..3)
        .map(|i| scalar(i * 160).canonical_bytes().unwrap())
        .collect();
    let queries: Vec<_> = (0..3)
        .map(|i| Query {
            basis: &basis,
            frame: &frames[i],
            q8: &q8[i],
            upstream: b"full upstream receipt",
        })
        .collect();
    assert!(Aggregate::prepare(&basis, &queries, MAX_RETAINED, DeclaredHold::Unsupported).is_err());
    let mut a =
        Aggregate::prepare(&basis, &queries, MAX_RETAINED, DeclaredHold::Onset10ms).unwrap();
    assert_eq!(a.receipts.len(), 3);
    for (i, r) in a.receipts.iter().enumerate() {
        assert!(Rc::ptr_eq(&a.basis, &r.basis));
        assert!(Rc::ptr_eq(
            &a.basis.original,
            &r.numerical.original_rich_basis
        ));
        assert!(Rc::ptr_eq(&a.projector.program, &r.numerical.program));
        assert_eq!(r.numerical.original_q8(), q8[i]);
        temporal::verify_report(&a, i).unwrap();
        let material = temporal::material(&a, i).unwrap();
        if let Ok(path) = std::env::var("CONDUIT_FARGAN_TEMPORAL_RECEIPT") {
            use std::io::Write;
            let mut out = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            writeln!(out, "{}", material).unwrap();
        }
        let span = StructuredInfoValue::from_canonical_bytes(&r.span).unwrap();
        assert_eq!(number(field(&span, "start_frame")), i as u64 * 160);
        assert_eq!(
            number(field(&span, "end_frame_exclusive")),
            (i as u64 + 1) * 160
        );
        let replay = PortableExpressionProgram::from_canonical_hex(&a.hold_program)
            .unwrap()
            .evaluate(&r.position)
            .unwrap();
        assert_eq!(replay, r.span);
        let expected = [80, 53, 64][i];
        assert_eq!(number(field(r.numerical.result(), "period")), expected);
    }
    assert!(
        Aggregate::prepare(
            &basis,
            &queries,
            a.canonical_retained_bound - 1,
            DeclaredHold::Onset10ms
        )
        .is_err()
    );
    let mut invalid = StructuredInfoValue::from_canonical_bytes(&basis.trajectory).unwrap();
    let zero_duration = record(
        ty(&d, "SpeechExactDuration"),
        &[("numerator_seconds", scalar(0)), ("denominator", scalar(1))],
    );
    invalid = record(
        ty(&d, "SpeechLinearPitchTrajectory"),
        &[
            ("duration", zero_duration),
            ("start", field(&invalid, "start").clone()),
            ("end", field(&invalid, "end").clone()),
        ],
    );
    let invalid_basis = Rc::new(Basis {
        original: Rc::clone(&basis.original),
        trajectory: Rc::from(invalid.canonical_bytes().unwrap()),
        revision_effect: Rc::clone(&basis.revision_effect),
    });
    let invalid_queries: Vec<_> = (0..3)
        .map(|i| Query {
            basis: &invalid_basis,
            frame: &frames[i],
            q8: &q8[i],
            upstream: b"full upstream receipt",
        })
        .collect();
    assert!(
        Aggregate::prepare(
            &invalid_basis,
            &invalid_queries,
            MAX_RETAINED,
            DeclaredHold::Onset10ms
        )
        .is_err()
    );
    let foreign = Rc::new(Basis {
        original: Rc::clone(&basis.original),
        trajectory: Rc::clone(&basis.trajectory),
        revision_effect: Rc::clone(&basis.revision_effect),
    });
    let saved = a.receipts[0].span.clone();
    a.receipts[0].span = record(
        ty(&d, "FarganRichHoldSpan"),
        &[
            ("start_frame", scalar(1)),
            ("end_frame_exclusive", scalar(161)),
        ],
    )
    .canonical_bytes()
    .unwrap();
    assert!(temporal::verify_report(&a, 0).is_err());
    a.receipts[0].span = saved;
    a.receipts[0].basis = Rc::clone(&foreign);
    assert!(temporal::verify_report(&a, 0).is_err());
    a.receipts[0].basis = Rc::clone(&basis);
    let bad = [
        Query {
            basis: &foreign,
            frame: &frames[0],
            q8: &q8[0],
            upstream: b"receipt",
        },
        Query {
            basis: &basis,
            frame: &frames[1],
            q8: &q8[1],
            upstream: b"receipt",
        },
    ];
    assert!(Aggregate::prepare(&basis, &bad, MAX_RETAINED, DeclaredHold::Onset10ms).is_err());
    let swapped = [
        Query {
            basis: &basis,
            frame: &frames[1],
            q8: &q8[0],
            upstream: b"receipt",
        },
        Query {
            basis: &basis,
            frame: &frames[0],
            q8: &q8[1],
            upstream: b"receipt",
        },
    ];
    assert!(Aggregate::prepare(&basis, &swapped, MAX_RETAINED, DeclaredHold::Onset10ms).is_err());
    for (count, ordinal, frame, policy) in [
        (3, 3, 480, "onset_hold_10ms"),
        (3, 1, 0, "onset_hold_10ms"),
        (3, 1, 160, "unsupported_hold"),
        (65536, 1, 160, "onset_hold_10ms"),
    ] {
        let forged = record(
            ty(&d, "FarganRichHoldPosition"),
            &[
                ("epoch_count", scalar(count)),
                ("ordinal", scalar(ordinal)),
                ("original_query_frame", scalar(frame)),
                ("policy", named(ty(&d, "FarganRichHoldPolicy"), policy)),
            ],
        )
        .canonical_bytes()
        .unwrap();
        assert!(
            interface::admit_retained_session_native(&d, "FarganRichHoldPosition", &forged)
                .is_err()
        );
    }
    assert!(
        Aggregate::prepare(&basis, &queries[..1], MAX_RETAINED, DeclaredHold::Onset10ms).is_err()
    );
}

#[path = "rich_period_temporal.rs"]
mod temporal;

#[test]
#[ignore = "requires verified complete continuous-word carrier"]
fn verified_full200_varying_windows_hold_actual_trajectories() {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(std::env::var("CONDUIT_FARGAN_FULL200_QUERIES").unwrap()).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "3c698c51a560bf3fad61740101decff6cf66957ebf60a1567e4380e130a66a2f"
    );
    let carrier: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let shared:Rc<[u8]>=Rc::from(serde_json::to_vec(&serde_json::json!({"original_intent":carrier["original_intent"],"realized_intent":carrier["realized_intent"],"rich_accepted":carrier["rich_accepted"],"word_admissions":carrier["word_admissions"],"initializations":carrier["initializations"],"files":carrier["files"]})).unwrap());
    let revisions:Rc<[u8]>=Rc::from(serde_json::to_vec(&serde_json::json!({"rich_accepted":carrier["rich_accepted"],"word_admissions":carrier["word_admissions"]})).unwrap());
    let all = carrier["queries"].as_array().unwrap();
    assert_eq!(all.len(), 200);
    let mut reports = Vec::new();
    let mut trajectories = Vec::new();
    let mut shared_hold_program = None;
    let mut totals = 0;
    for segment in 4..10 {
        let first = segment * 20;
        let admission: Vec<u8> =
            serde_json::from_value(all[first]["pitch_admission"].clone()).unwrap();
        let admitted = StructuredInfoValue::from_canonical_bytes(&admission).unwrap();
        let trajectory = field(
            field(field(field(&admitted, "original"), "pitch"), "admission"),
            "trajectory",
        )
        .canonical_bytes()
        .unwrap();
        let basis = Rc::new(Basis {
            original: Rc::clone(&shared),
            trajectory: Rc::from(trajectory),
            revision_effect: Rc::clone(&revisions),
        });
        let mut materials = Vec::new();
        for (offset, query) in all[first..first + 20].iter().enumerate() {
            assert_eq!(query["basis"], "continuous_word_pitch_frame");
            assert_eq!(query["segment"], segment);
            let raw: Vec<u8> = serde_json::from_value(query["pitch_admission"].clone()).unwrap();
            let admission = StructuredInfoValue::from_canonical_bytes(&raw).unwrap();
            let original = field(&admission, "original");
            assert_eq!(
                field(field(field(original, "pitch"), "admission"), "trajectory")
                    .canonical_bytes()
                    .unwrap(),
                basis.trajectory.as_ref()
            );
            assert_eq!(
                number(field(original, "global_frame")),
                ((first + offset) * 160) as u64
            );
            materials.push((
                field(original, "global_frame").canonical_bytes().unwrap(),
                field(&admission, "projected").canonical_bytes().unwrap(),
                serde_json::to_vec(query).unwrap(),
            ));
        }
        let queries = materials
            .iter()
            .map(|(frame, q8, upstream)| Query {
                basis: &basis,
                frame,
                q8,
                upstream,
            })
            .collect::<Vec<_>>();
        assert!(
            Aggregate::prepare_window(
                &basis,
                &queries,
                MAX_RETAINED,
                DeclaredHold::Onset10ms,
                first + 1,
                200
            )
            .is_err()
        );
        assert!(
            Aggregate::prepare_window(
                &basis,
                &queries,
                MAX_RETAINED,
                DeclaredHold::Unsupported,
                first,
                200
            )
            .is_err()
        );
        let aggregate = Aggregate::prepare_window(
            &basis,
            &queries,
            MAX_RETAINED,
            DeclaredHold::Onset10ms,
            first,
            200,
        )
        .unwrap();
        assert!(Rc::ptr_eq(&aggregate.basis.original, &shared));
        for ordinal in 0..20 {
            temporal::verify_report(&aggregate, ordinal).unwrap();
            let mut material = temporal::material(&aggregate, ordinal).unwrap();
            let object = material.as_object_mut().unwrap();
            let program = object.remove("source_program").unwrap();
            if let Some(retained) = &shared_hold_program {
                assert_eq!(retained, &program);
            } else {
                shared_hold_program = Some(program);
            }
            assert_eq!(
                object.remove("original_trajectory").unwrap(),
                serde_json::json!(basis.trajectory.as_ref())
            );
            assert_eq!(
                object.remove("original_revision_effect_material").unwrap(),
                serde_json::json!(revisions.as_ref())
            );
            object.insert(
                "original_trajectory_index".into(),
                serde_json::json!(segment - 4),
            );
            object.insert(
                "shared_revision_effect_basis".into(),
                serde_json::json!(true),
            );
            object.insert(
                "shared_source_hold_program_index".into(),
                serde_json::json!(0),
            );
            reports.push(material);
        }
        trajectories.push(serde_json::json!(basis.trajectory.as_ref()));
        totals += aggregate.canonical_retained_bound;
    }
    assert_eq!(reports.len(), 120);
    if let Ok(path) = std::env::var("CONDUIT_FARGAN_FULL200_HOLD_OUTPUT") {
        std::fs::write(path,serde_json::to_vec(&serde_json::json!({"original_artifact_sha256":"3c698c51a560bf3fad61740101decff6cf66957ebf60a1567e4380e130a66a2f","shared_original_basis":shared.as_ref(),"shared_revision_effect_basis":revisions.as_ref(),"original_trajectories":trajectories,"shared_source_hold_program":shared_hold_program,"reports":reports,"conservative_window_canonical_bounds_sum":totals,"unchanged_Hello_queries_without_temporal_approximation":80,"scope":"120 exact original varying trajectory Source hold receipts + Core Approximation; numerical Precision remains separate; canonical bounds are not complete runtime heap admission"})).unwrap()).unwrap();
    }
}
