//! Separately admitted exact-cycle → nearest model period capability.
//! Root's prepared common owner must supply the actual frame/rich-basis custody;
//! this numerical component does not establish that provenance or clock mapping.
use super::*;
use std::rc::Rc;

const SOURCE: &str = include_str!("../../fargan_rich_period.conduit");
fn source() -> String {
    String::from(include_str!("../../../speech/timing.conduit"))
        + "\n"
        + include_str!("../../../speech/pitch_trajectory.conduit")
            .split("type SpeechSegmentPitchAdmission")
            .next()
            .unwrap()
        + "\n"
        + SOURCE
        + "\ntype FarganPeriod = U16 in 32..=255\n"
}
fn ty<'a>(d: &'a CheckedSyntaxDocument, name: &str) -> &'a StructuredInfoType {
    &d.native_types
        .iter()
        .find(|t| t.name == name)
        .unwrap()
        .value_type
}
fn field<'a>(v: &'a StructuredInfoValue, n: &str) -> &'a StructuredInfoValue {
    super::case_state::field(v, n)
}
fn number(v: &StructuredInfoValue) -> u64 {
    let StructuredInfoValueShape::Leaf(b) = v.shape() else {
        panic!("U64")
    };
    u64::from_le_bytes(b.try_into().unwrap())
}
fn record(t: &StructuredInfoType, values: &[(&str, StructuredInfoValue)]) -> StructuredInfoValue {
    StructuredInfoValue::record(
        t.clone(),
        values
            .iter()
            .map(|(n, v)| StructuredFieldValue::new(*n, v.clone()).unwrap())
            .collect(),
    )
    .unwrap()
}
fn scalar(n: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
        n.to_le_bytes().to_vec(),
    )
    .unwrap()
}
fn named(t: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Variant { cases, .. } = t.shape() else {
        panic!("policy")
    };
    let c = cases.iter().find(|c| c.tag() == tag).unwrap();
    StructuredInfoValue::variant(
        t.clone(),
        tag,
        declarations::fixture_value(c.payload_type()),
    )
    .unwrap()
}
/// One immutable checked Source document and parsed program per arithmetic profile.
/// Preparation remains allocating; this is not a Flow or whole-memory admission.
struct RichPeriodProjector {
    document: CheckedSyntaxDocument,
    program: Rc<str>,
    parsed: PortableExpressionProgram,
    wide: bool,
}
impl RichPeriodProjector {
    fn prepare(wide: bool) -> Result<Rc<Self>, String> {
        let document =
            check_syntax_document(&parse_syntax_document(&source()), &StartupCatalog::new())
                .map_err(|e| format!("{e:?}"))?;
        let graph = expand_canonical_plot_for_authoring(
            &document,
            if wide {
                "ai/fargan-rich-wide-nearest-period"
            } else {
                "ai/fargan-rich-nearest-period"
            },
            &ProfileCatalog::new(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let ConfigurationValue::Text(program) = &graph.expanded.gears[0].configuration[0].value
        else {
            return Err("Source program".into());
        };
        let parsed =
            PortableExpressionProgram::from_canonical_hex(program).map_err(|e| format!("{e:?}"))?;
        Ok(Rc::new(Self {
            document,
            program: Rc::from(program.as_str()),
            parsed,
            wide,
        }))
    }
}
pub(super) struct PreparedRichPeriod {
    original_q8: Vec<u8>,
    original_query_frame: Vec<u8>,
    original_rich_basis: Rc<[u8]>,
    original_projection: Vec<u8>,
    eligible: Vec<u8>,
    arithmetic: Vec<u8>,
    profile: &'static str,
    program: Rc<str>,
    raw: Vec<u8>,
    result: StructuredInfoValue,
    period: StructuredInfoValue,
}
impl PreparedRichPeriod {
    /// Allocating preparation; full opaque upstream custody is composed by root.
    pub(super) fn prepare(
        original_q8: &[u8],
        original_query_frame: &[u8],
        original_rich_basis: &[u8],
        original_projection: &[u8],
        epoch_index: u64,
        policy: StructuredInfoValue,
        cadence: StructuredInfoValue,
    ) -> Result<Self, String> {
        Self::prepare_profile::<false>(
            original_q8,
            original_query_frame,
            original_rich_basis,
            original_projection,
            epoch_index,
            policy,
            cadence,
        )
    }
    pub(super) fn prepare_wide(
        original_q8: &[u8],
        original_query_frame: &[u8],
        original_rich_basis: &[u8],
        original_projection: &[u8],
        epoch_index: u64,
        policy: StructuredInfoValue,
        cadence: StructuredInfoValue,
    ) -> Result<Self, String> {
        Self::prepare_profile::<true>(
            original_q8,
            original_query_frame,
            original_rich_basis,
            original_projection,
            epoch_index,
            policy,
            cadence,
        )
    }
    fn prepare_profile<const WIDE: bool>(
        original_q8: &[u8],
        original_query_frame: &[u8],
        original_rich_basis: &[u8],
        original_projection: &[u8],
        epoch_index: u64,
        policy: StructuredInfoValue,
        cadence: StructuredInfoValue,
    ) -> Result<Self, String> {
        let projector = RichPeriodProjector::prepare(WIDE)?;
        Self::prepare_shared(
            &projector,
            original_q8,
            original_query_frame,
            Rc::from(original_rich_basis),
            original_projection,
            epoch_index,
            (policy, cadence),
        )
    }
    fn prepare_shared(
        projector: &Rc<RichPeriodProjector>,
        original_q8: &[u8],
        original_query_frame: &[u8],
        original_rich_basis: Rc<[u8]>,
        original_projection: &[u8],
        epoch_index: u64,
        declarations: (StructuredInfoValue, StructuredInfoValue),
    ) -> Result<Self, String> {
        if original_rich_basis.is_empty() || original_projection.is_empty() {
            return Err("original rich basis/projection required".into());
        }
        let d = &projector.document;
        let (policy, cadence) = declarations;
        if policy.value_type() != ty(d, "FarganRichPeriodPolicy")
            || cadence.value_type() != ty(d, "FarganRichPeriodCadence")
        {
            return Err("foreign quantization policy/cadence Type".into());
        }
        // Independently re-admit original nested Source owner before wrapping it.
        let q8 = interface::admit_retained_session_native(d, "SpeechCycleQ8AtRate", original_q8)
            .map_err(|e| format!("originalQ8: {e}"))?;
        let query = StructuredInfoValue::from_canonical_bytes(original_query_frame)
            .map_err(|e| format!("{e:?}"))?;
        if query.value_type() != scalar(0).value_type() {
            return Err("original grid frame Type".into());
        }
        let eligible_name = if projector.wide {
            "FarganRichWidePeriodEligible"
        } else {
            "FarganRichPeriodEligible"
        };
        let arithmetic_name = if projector.wide {
            "FarganRichWidePeriodArithmetic"
        } else {
            "FarganRichPeriodArithmetic"
        };
        let profile = if projector.wide {
            "ai/fargan-rich-wide-period-eligible@1"
        } else {
            "ai/fargan-rich-period-eligible@1"
        };
        let eligible = record(
            ty(d, eligible_name),
            &[
                ("original", q8.clone()),
                ("policy", policy),
                ("cadence", cadence),
                ("epoch_index", scalar(epoch_index)),
                ("original_query_frame", query),
            ],
        )
        .canonical_bytes()
        .map_err(|e| format!("{e:?}"))?;
        interface::admit_retained_session_native(d, eligible_name, &eligible)
            .map_err(|e| format!("eligible: {e}"))?;
        let request = field(&q8, "request");
        let cycle = field(request, "cycle");
        let arithmetic = record(
            ty(d, arithmetic_name),
            &[
                (
                    "numerator_seconds",
                    field(cycle, "numerator_seconds").clone(),
                ),
                ("denominator", field(cycle, "denominator").clone()),
                ("sample_rate_hz", field(request, "sample_rate_hz").clone()),
            ],
        )
        .canonical_bytes()
        .map_err(|e| format!("{e:?}"))?;
        interface::admit_retained_session_native(d, arithmetic_name, &arithmetic)?;
        let raw = projector
            .parsed
            .evaluate(&arithmetic)
            .map_err(|e| format!("program evaluation: {e:?}"))?;
        let result = interface::admit_retained_session_native(d, "FarganRichPeriodRaw", &raw)
            .map_err(|e| format!("rawresult: {e}"))?;
        let period = admit_final_period(d, &result).map_err(|e| format!("finalperiod: {e}"))?;
        Ok(Self {
            original_q8: original_q8.to_vec(),
            original_query_frame: original_query_frame.to_vec(),
            original_rich_basis,
            original_projection: original_projection.to_vec(),
            eligible,
            arithmetic,
            profile,
            program: Rc::clone(&projector.program),
            raw,
            result,
            period,
        })
    }
    pub(super) fn original_q8(&self) -> &[u8] {
        &self.original_q8
    }
    pub(super) fn result(&self) -> &StructuredInfoValue {
        &self.result
    }
    pub(super) fn period(&self) -> &StructuredInfoValue {
        &self.period
    }
    pub(super) fn material(&self) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"core_fidelity_report":projection::report_material(self),"source":source(),"original_q8":self.original_q8,"original_query_frame":self.original_query_frame,"original_rich_basis":self.original_rich_basis.as_ref(),"original_projection":self.original_projection,"eligible":self.eligible,"arithmetic_profile":self.profile,"arithmetic_input":self.arithmetic,"program":self.program.as_ref(),"raw":self.raw,"admitted_result":self.result.canonical_bytes().unwrap(),"admitted_model_period":self.period.canonical_bytes().unwrap(),"policy":"nearest_whole_sample_ties_up","cadence":"epoch_onset_160_frames_at_16000_hz","scope":"declared grid queries; upstream opaque custody and clock/playback authority not established here"})).unwrap()
    }
}
fn admit_final_period(
    d: &CheckedSyntaxDocument,
    raw: &StructuredInfoValue,
) -> Result<StructuredInfoValue, String> {
    // Representation narrowing only; Source authored all arithmetic and guards.
    let n = u16::try_from(number(field(raw, "period"))).map_err(|_| "U16 representation")?;
    let final_type = ty(d, "FarganPeriod");
    let StructuredInfoTypeShape::Nominal { representation, .. } = final_type.shape() else {
        return Err("nominal model period required".into());
    };
    let representation =
        StructuredInfoValue::leaf(representation.clone(), n.to_le_bytes().to_vec())
            .map_err(|e| format!("{e:?}"))?;
    let v = StructuredInfoValue::nominal(final_type.clone(), representation)
        .map_err(|e| format!("{e:?}"))?;
    interface::admit_retained_session_native(
        d,
        "FarganPeriod",
        &v.canonical_bytes().map_err(|e| format!("{e:?}"))?,
    )
}
fn fixture(
    d: &CheckedSyntaxDocument,
    n: u64,
    den: u64,
    rate: u64,
    q8: u64,
    rem: u64,
) -> StructuredInfoValue {
    let cycle = record(
        ty(d, "SpeechFundamentalCycle"),
        &[
            ("numerator_seconds", scalar(n)),
            ("denominator", scalar(den)),
        ],
    );
    let request = record(
        ty(d, "SpeechCycleAtRateRequest"),
        &[("cycle", cycle), ("sample_rate_hz", scalar(rate))],
    );
    record(
        ty(d, "SpeechCycleQ8AtRate"),
        &[
            ("request", request),
            ("whole_q8", scalar(q8)),
            ("remainder_numerator", scalar(rem)),
        ],
    )
}
fn prepare_fixture(
    n: u64,
    den: u64,
    rate: u64,
    epoch: u64,
    frame: u64,
) -> Result<PreparedRichPeriod, String> {
    let d =
        check_syntax_document(&parse_syntax_document(&source()), &StartupCatalog::new()).unwrap();
    let numerator = u128::from(n) * u128::from(rate) * 256;
    let q8 = u64::try_from(numerator / u128::from(den)).unwrap();
    let rem = u64::try_from(numerator % u128::from(den)).unwrap();
    PreparedRichPeriod::prepare(
        &fixture(&d, n, den, rate, q8, rem)
            .canonical_bytes()
            .unwrap(),
        &scalar(frame).canonical_bytes().unwrap(),
        b"test-only original rich basis",
        b"test-only original projection",
        epoch,
        named(
            ty(&d, "FarganRichPeriodPolicy"),
            "nearest_whole_sample_ties_up",
        ),
        named(
            ty(&d, "FarganRichPeriodCadence"),
            "epoch_onset_160_frames_at_16000_hz",
        ),
    )
}
#[test]
fn rich_period_exact_fraction_nearest_policy_and_receipts() {
    let d =
        check_syntax_document(&parse_syntax_document(&source()), &StartupCatalog::new()).unwrap();
    for (n, den) in [
        (1, 200),
        (1, 300),
        (149, 32000),
        (74499, 16000000),
        (74501, 16000000),
        (68451041, 4294967295),
        (8589935, 4294967295),
        (151, 32000),
        (1, 500),
        (255, 16000),
        (2, 400),
        (4294967295, 4294967295),
    ] {
        let original = u128::from(n) * 16000;
        if original < 32 * u128::from(den) || original > 255 * u128::from(den) {
            assert!(prepare_fixture(n, den, 16000, 0, 0).is_err());
            continue;
        }
        let receipt = prepare_fixture(n, den, 16000, 3, 480).unwrap();
        let expected = (original * 2 + u128::from(den)) / (u128::from(den) * 2);
        assert_eq!(
            u128::from(number(field(receipt.result(), "period"))),
            expected
        );
        assert_eq!(
            u128::from(number(field(receipt.result(), "error_numerator"))),
            (expected * u128::from(den)).abs_diff(original)
        );
        assert_eq!(number(field(receipt.result(), "error_denominator")), den);
        assert!((expected * u128::from(den)).abs_diff(original) * 2 <= u128::from(den));
        let decoded = StructuredInfoValue::from_canonical_bytes(receipt.original_q8()).unwrap();
        assert_eq!(
            number(field(
                field(field(&decoded, "request"), "cycle"),
                "numerator_seconds"
            )),
            n
        );
        assert_eq!(
            number(field(
                field(field(&decoded, "request"), "cycle"),
                "denominator"
            )),
            den
        );
        eprintln!(
            "eligible canonical frame {}B; period final {}B",
            receipt.eligible.len(),
            receipt.period().canonical_bytes().unwrap().len()
        );
        let material: serde_json::Value = serde_json::from_slice(&receipt.material()).unwrap();
        let p =
            PortableExpressionProgram::from_canonical_hex(material["program"].as_str().unwrap())
                .unwrap();
        assert_eq!(p.evaluate(&receipt.arithmetic).unwrap(), receipt.raw);
        assert_eq!(
            StructuredInfoValue::from_canonical_bytes(&receipt.period().canonical_bytes().unwrap())
                .unwrap(),
            *receipt.period()
        );
    }
    for name in [
        "SpeechCycleQ8AtRate",
        "FarganRichPeriodEligible",
        "FarganRichWidePeriodEligible",
        "FarganRichPeriodArithmetic",
        "FarganRichWidePeriodArithmetic",
        "FarganRichPeriodRaw",
        "FarganPeriod",
    ] {
        let bytes = ty(&d, name).canonical_bytes().unwrap();
        assert_eq!(
            StructuredInfoType::from_canonical_bytes(&bytes).unwrap(),
            *ty(&d, name)
        );
        eprintln!("{name} Type {}B", bytes.len());
    }
}
#[test]
fn rich_period_refuses_foreign_grid_bounds_and_forged_original() {
    for (n, den, rate, epoch, frame) in [
        (1, 200, 8000, 0, 0),
        (1, 200, 48000, 0, 0),
        (1, 200, 16000, 2, 319),
        (1, 200, 16000, 65536, 10485760),
        (1, 501, 16000, 0, 0),
        (510001, 32000000, 16000, 0, 0),
    ] {
        assert!(
            prepare_fixture(n, den, rate, epoch, frame).is_err(),
            "{n}/{den}@{rate}"
        );
    }
    // Full canonical cycles remain meaningful; this unreduced representation is
    // explicitly beyond the separately named U32 numerical capability.
    assert!(prepare_fixture(4294967296, 858993459200, 16000, 0, 0).is_err());
    let d =
        check_syntax_document(&parse_syntax_document(&source()), &StartupCatalog::new()).unwrap();
    let policy = || {
        named(
            ty(&d, "FarganRichPeriodPolicy"),
            "nearest_whole_sample_ties_up",
        )
    };
    let cadence = || {
        named(
            ty(&d, "FarganRichPeriodCadence"),
            "epoch_onset_160_frames_at_16000_hz",
        )
    };
    for (policy_value, cadence_value) in [
        (
            named(ty(&d, "FarganRichPeriodPolicy"), "unsupported_policy"),
            cadence(),
        ),
        (
            policy(),
            named(ty(&d, "FarganRichPeriodCadence"), "unsupported_cadence"),
        ),
        (scalar(1), cadence()),
        (policy(), scalar(160)),
    ] {
        assert!(
            PreparedRichPeriod::prepare(
                &fixture(&d, 1, 200, 16000, 20480, 0)
                    .canonical_bytes()
                    .unwrap(),
                &scalar(0).canonical_bytes().unwrap(),
                b"basis",
                b"projection",
                0,
                policy_value,
                cadence_value
            )
            .is_err()
        );
    }
    for (q8, rem) in [(20480, 1), (20479, 0), (20480, 200)] {
        let original = fixture(&d, 1, 200, 16000, q8, rem)
            .canonical_bytes()
            .unwrap();
        assert!(
            PreparedRichPeriod::prepare(
                &original,
                &scalar(0).canonical_bytes().unwrap(),
                b"basis",
                b"projection",
                0,
                policy(),
                cadence()
            )
            .is_err()
        );
    }
    for forged in [0, 31, 256, u64::MAX] {
        let raw = record(
            ty(&d, "FarganRichPeriodRaw"),
            &[
                ("period", scalar(forged)),
                (
                    "exact",
                    StructuredInfoValue::leaf(
                        StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
                        vec![1],
                    )
                    .unwrap(),
                ),
                (
                    "model_above_exact",
                    StructuredInfoValue::leaf(
                        StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
                        vec![0],
                    )
                    .unwrap(),
                ),
                ("error_numerator", scalar(0)),
                ("error_denominator", scalar(1)),
            ],
        );
        assert!(admit_final_period(&d, &raw).is_err());
    }
}
#[path = "rich_period_projection.rs"]
mod projection;

#[test]
#[ignore = "actual direct16k Source feature-owner gate"]
fn rich_period_enters_actual_direct16k_feature_owner_without_legacy_clamp() {
    std::thread::Builder::new().stack_size(64*1024*1024).spawn(|| {
        let receipt=prepare_fixture(1,300,16000,0,0).unwrap();
        assert_eq!(number(field(receipt.result(),"period")),53);
        let existing=conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile::check_definition("type FarganPeriod = U16 in 32..=255\n").unwrap();
        assert_eq!(receipt.period().value_type(),existing.value_type());
        let mut samples=[0i16;160];for (i,s) in samples.iter_mut().enumerate(){*s=((i as i32*397)%65536-32768)as i16;}
        let proposal=super::runtime::run_native_first_feature16k(&samples,receipt.period());
        assert_eq!(field(&proposal,"period"),receipt.period());
        let history=field(&proposal,"history");let StructuredInfoValueShape::Collection(values)=history.shape()else{panic!("history")};assert_eq!(values.len(),640);
        if let Ok(path)=std::env::var("CONDUIT_FARGAN_RICH_PERIOD_RECEIPT") {std::fs::write(path,receipt.material()).unwrap();}
        eprintln!("actual direct160/16k Source feature proposal retains exact admitted nearest53-sample period; no legacy clamp/boundary substitution");
    }).unwrap().join().unwrap();
}

#[test]
#[ignore = "pinned root actual continuous-word frame artifact required"]
fn rich_wide_period_consumes_actual_original_wordpitch_frames_and_source_receipts() {
    use sha2::{Digest, Sha256};
    let path = std::env::var("CONDUIT_FARGAN_RICH_EXACT_FRAMES").unwrap();
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "984b90c2312dbf2ba1673f5ac5ea6c4a37a1fbea2312b8b9997629494d81db5c"
    );
    let material: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let frames = material["exact_frames"].as_array().unwrap();
    assert_eq!(frames.len(), 6);
    let d =
        check_syntax_document(&parse_syntax_document(&source()), &StartupCatalog::new()).unwrap();
    let mut receipts = Vec::new();
    for entry in frames {
        let native = |name: &str| serde_json::from_value::<Vec<u8>>(entry[name].clone()).unwrap();
        let original_frame = native("original_frame");
        assert_eq!(original_frame, native("grid_input"));
        let decoded = StructuredInfoValue::from_canonical_bytes(&original_frame).unwrap();
        assert_eq!(decoded.canonical_bytes().unwrap(), original_frame);
        let grid =
            PortableExpressionProgram::from_canonical_hex(entry["grid_program"].as_str().unwrap())
                .unwrap();
        assert_eq!(
            grid.evaluate(&original_frame).unwrap(),
            native("grid_output")
        );
        let fraction = PortableExpressionProgram::from_canonical_hex(
            entry["fraction_program"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            fraction.evaluate(&native("grid_output")).unwrap(),
            native("fraction_output")
        );
        assert_eq!(native("fraction"), native("fraction_output"));
        let exact_fraction =
            StructuredInfoValue::from_canonical_bytes(&native("fraction")).unwrap();
        let q8_bytes = native("projected_q8");
        let q8 = StructuredInfoValue::from_canonical_bytes(&q8_bytes).unwrap();
        let cycle = field(field(&q8, "request"), "cycle");
        assert_eq!(
            field(cycle, "numerator_seconds"),
            field(&exact_fraction, "numerator")
        );
        assert_eq!(
            field(cycle, "denominator"),
            field(&exact_fraction, "denominator")
        );
        let global = field(&decoded, "global_frame");
        let index = number(global) / 160;
        let query = global.canonical_bytes().unwrap();
        let upstream = serde_json::to_vec(entry).unwrap();
        let policy = || {
            named(
                ty(&d, "FarganRichPeriodPolicy"),
                "nearest_whole_sample_ties_up",
            )
        };
        let cadence = || {
            named(
                ty(&d, "FarganRichPeriodCadence"),
                "epoch_onset_160_frames_at_16000_hz",
            )
        };
        // No reduction or initializer-cycle substitution. The complete actual
        // original frame, cycle representation and upstream executions survive.
        let receipt = PreparedRichPeriod::prepare_wide(
            &q8_bytes,
            &query,
            &original_frame,
            &upstream,
            index,
            policy(),
            cadence(),
        )
        .unwrap();
        assert_eq!(receipt.original_q8(), q8_bytes);
        if number(field(cycle, "numerator_seconds")) > u64::from(u32::MAX)
            || number(field(cycle, "denominator")) > u64::from(u32::MAX)
        {
            assert!(
                PreparedRichPeriod::prepare(
                    &q8_bytes,
                    &query,
                    &original_frame,
                    &upstream,
                    index,
                    policy(),
                    cadence()
                )
                .is_err()
            );
        }
        let n = u128::from(number(field(cycle, "numerator_seconds")));
        let den = u128::from(number(field(cycle, "denominator")));
        let expected = (n * 16000 * 2 + den) / (den * 2);
        assert_eq!(
            u128::from(number(field(receipt.result(), "period"))),
            expected
        );
        assert_eq!(
            u128::from(number(field(receipt.result(), "error_numerator"))),
            (expected * den).abs_diff(n * 16000)
        );
        assert!(
            PreparedRichPeriod::prepare_wide(
                &q8_bytes,
                &scalar(number(global) + 1).canonical_bytes().unwrap(),
                &original_frame,
                &upstream,
                index,
                policy(),
                cadence()
            )
            .is_err()
        );
        eprintln!(
            "actualsegment{} global{} fullcycle{}/{} nearest{}; retainedQ8rem{}",
            entry["segment"],
            number(global),
            n,
            den,
            expected,
            number(field(&q8, "remainder_numerator"))
        );
        receipts.push(serde_json::from_slice::<serde_json::Value>(&receipt.material()).unwrap());
    }
    if let Ok(path) = std::env::var("CONDUIT_FARGAN_RICH_EXACT_OUTPUT") {
        std::fs::write(path, serde_json::to_vec(&receipts).unwrap()).unwrap();
    }
}
#[test]
fn rich_wide_arithmetic_bounds_and_overflow_refusals_are_independent() {
    let max = u128::from(u64::MAX);
    let nmax = 4503599627370u64;
    let dmax = 282574471495681u64;
    assert!(u128::from(nmax) * 16000 * 256 <= max);
    assert!(u128::from(dmax) * 65281 <= max);
    assert!(u128::from(nmax) * 16000 * 2 + u128::from(dmax) <= max);
    let d =
        check_syntax_document(&parse_syntax_document(&source()), &StartupCatalog::new()).unwrap();
    for (n, den, accepted) in [
        (4503530639462, dmax, true),
        (565148942992, dmax, true),
        (565148942991, dmax, false),
        (1412872357479, 282574471495800, false),
        (4503599627371, 282574471495681, false),
    ] {
        let numerator = u128::from(n) * 16000 * 256;
        let q8 = u64::try_from(numerator / u128::from(den)).unwrap();
        let rem = u64::try_from(numerator % u128::from(den)).unwrap();
        let original = fixture(&d, n, den, 16000, q8, rem)
            .canonical_bytes()
            .unwrap();
        let result = PreparedRichPeriod::prepare_wide(
            &original,
            &scalar(0).canonical_bytes().unwrap(),
            b"testwidebasis",
            b"testwideprojection",
            0,
            named(
                ty(&d, "FarganRichPeriodPolicy"),
                "nearest_whole_sample_ties_up",
            ),
            named(
                ty(&d, "FarganRichPeriodCadence"),
                "epoch_onset_160_frames_at_16000_hz",
            ),
        );
        assert_eq!(result.is_ok(), accepted, "{n}/{den}");
        if let Ok(receipt) = result {
            let expected = (u128::from(n) * 16000 * 2 + u128::from(den)) / (u128::from(den) * 2);
            assert_eq!(
                u128::from(number(field(receipt.result(), "period"))),
                expected
            );
        }
    }
}

#[test]
#[ignore = "pinned actual joined rich DSP frame artifact required"]
fn rich_wide_period_consumes_actual_joined_dsp_frame_receipts() {
    use sha2::{Digest, Sha256};
    let input = std::fs::read(std::env::var("CONDUIT_FARGAN_RICH_JOINED_FRAMES").unwrap()).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&input)),
        "e9d3cf258f3f9f638299e9a9993e40c811d3371ea47bef4324111feca8db7820"
    );
    let joined: serde_json::Value = serde_json::from_slice(&input).unwrap();
    let frames = joined["first_frames"].as_array().unwrap();
    assert_eq!(frames.len(), 6);
    let basis=serde_json::to_vec(&serde_json::json!({"original_intent":joined["original_intent"],"realized_intent":joined["realized_intent"],"rich_accepted":joined["rich_accepted"],"admissions":joined["admissions"]})).unwrap();
    let d =
        check_syntax_document(&parse_syntax_document(&source()), &StartupCatalog::new()).unwrap();
    let mut receipts = Vec::new();
    for frame in frames {
        let admission: Vec<u8> = serde_json::from_value(frame["pitch_admission"].clone()).unwrap();
        let original = StructuredInfoValue::from_canonical_bytes(&admission).unwrap();
        assert_eq!(original.canonical_bytes().unwrap(), admission);
        let original_frame = field(&original, "original");
        let projected = field(&original, "projected");
        let fraction = field(&original, "fraction");
        let cycle = field(field(projected, "request"), "cycle");
        assert_eq!(
            field(cycle, "numerator_seconds"),
            field(fraction, "numerator")
        );
        assert_eq!(field(cycle, "denominator"), field(fraction, "denominator"));
        let executions = frame["executions"].as_array().unwrap();
        assert_eq!(executions.len(), 3);
        for execution in executions {
            let program = PortableExpressionProgram::from_canonical_hex(
                execution["program"].as_str().unwrap(),
            )
            .unwrap();
            let input: Vec<u8> = serde_json::from_value(execution["input"].clone()).unwrap();
            let output: Vec<u8> = serde_json::from_value(execution["output"].clone()).unwrap();
            assert_eq!(program.evaluate(&input).unwrap(), output);
        }
        assert_eq!(
            serde_json::from_value::<Vec<u8>>(executions[0]["input"].clone()).unwrap(),
            original_frame.canonical_bytes().unwrap()
        );
        assert_eq!(executions[0]["output"], executions[1]["input"]);
        assert_eq!(
            serde_json::from_value::<Vec<u8>>(executions[1]["output"].clone()).unwrap(),
            fraction.canonical_bytes().unwrap()
        );
        assert_eq!(
            serde_json::from_value::<Vec<u8>>(executions[2]["input"].clone()).unwrap(),
            field(projected, "request").canonical_bytes().unwrap()
        );
        let raw_q8 = StructuredInfoValue::from_canonical_bytes(
            &serde_json::from_value::<Vec<u8>>(executions[2]["output"].clone()).unwrap(),
        )
        .unwrap();
        assert_eq!(field(&raw_q8, "whole_q8"), field(projected, "whole_q8"));
        assert_eq!(
            field(&raw_q8, "remainder_numerator"),
            field(projected, "remainder_numerator")
        );
        let query = field(original_frame, "global_frame");
        let index = number(query) / 160;
        let retained_projection = serde_json::to_vec(frame).unwrap();
        let receipt = PreparedRichPeriod::prepare_wide(
            &projected.canonical_bytes().unwrap(),
            &query.canonical_bytes().unwrap(),
            &basis,
            &retained_projection,
            index,
            named(
                ty(&d, "FarganRichPeriodPolicy"),
                "nearest_whole_sample_ties_up",
            ),
            named(
                ty(&d, "FarganRichPeriodCadence"),
                "epoch_onset_160_frames_at_16000_hz",
            ),
        )
        .unwrap();
        let n = u128::from(number(field(cycle, "numerator_seconds")));
        let denominator = u128::from(number(field(cycle, "denominator")));
        let expected = (n * 16000 * 2 + denominator) / (denominator * 2);
        assert_eq!(
            u128::from(number(field(receipt.result(), "period"))),
            expected
        );
        eprintln!(
            "actualjoined global{} nearest{}; entireoriginalbasis{}B/fullframereceipt{}B retained",
            number(query),
            expected,
            basis.len(),
            retained_projection.len()
        );
        receipts.push(serde_json::from_slice::<serde_json::Value>(&receipt.material()).unwrap());
    }
    if let Ok(path) = std::env::var("CONDUIT_FARGAN_RICH_JOINED_OUTPUT") {
        std::fs::write(path, serde_json::to_vec(&receipts).unwrap()).unwrap();
    }
}

#[path = "rich_period_aggregate.rs"]
mod aggregate;

#[test]
#[ignore = "requires new actual opaque committed-owner six-frame receipt"]
fn rich_actual_committed_trajectory_cross_sdk_canonical_admission() {
    let path = std::env::var("CONDUIT_FARGAN_OPAQUE_TRAJECTORY_RECEIPT").unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() <= 32 * 1024 * 1024);
    let input: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let d =
        check_syntax_document(&parse_syntax_document(&source()), &StartupCatalog::new()).unwrap();
    let mut receipts = Vec::new();
    for (index, entry) in input["first_frames"].as_array().unwrap().iter().enumerate() {
        let bytes = |n: &str| serde_json::from_value::<Vec<u8>>(entry[n].clone()).unwrap();
        assert_eq!(entry["bound_ordinal"], index);
        assert_eq!(entry["original_owner_identity"], true);
        assert_eq!(entry["foreign_equal_byte_owner_refused"], true);
        let trajectory = bytes("bound_original_trajectory");
        // No Rust values/types cross SDK families: original canonical bytes are
        // freshly admitted through the older SDK's exact authored Source owner.
        interface::admit_retained_session_native(&d, "SpeechLinearPitchTrajectory", &trajectory)
            .unwrap();
        let full = StructuredInfoValue::from_canonical_bytes(&bytes("pitch_admission")).unwrap();
        let original = field(&full, "original");
        let pitch = field(field(original, "pitch"), "admission");
        assert_eq!(
            pitch.canonical_bytes().unwrap(),
            bytes("bound_original_pitch_admission")
        );
        assert_eq!(
            field(pitch, "trajectory").canonical_bytes().unwrap(),
            trajectory
        );
        let query = field(original, "global_frame");
        let frame = number(query);
        assert_eq!(frame % 160, 0);
        let q8 = field(&full, "projected").canonical_bytes().unwrap();
        let basis = bytes("bound_original_rich_admission");
        let upstream = serde_json::to_vec(entry).unwrap();
        let receipt = PreparedRichPeriod::prepare_wide(
            &q8,
            &query.canonical_bytes().unwrap(),
            &basis,
            &upstream,
            frame / 160,
            named(
                ty(&d, "FarganRichPeriodPolicy"),
                "nearest_whole_sample_ties_up",
            ),
            named(
                ty(&d, "FarganRichPeriodCadence"),
                "epoch_onset_160_frames_at_16000_hz",
            ),
        )
        .unwrap();
        assert_eq!(receipt.original_q8(), q8);
        receipts.push(serde_json::json!({"ordinal":index,"original_trajectory":trajectory,"full_original_pitch_admission":bytes("bound_original_pitch_admission"),"receipt":serde_json::from_slice::<serde_json::Value>(&receipt.material()).unwrap()}));
    }
    assert_eq!(receipts.len(), 6);
    if let Ok(path) = std::env::var("CONDUIT_FARGAN_OPAQUE_TRAJECTORY_OUTPUT") {
        std::fs::write(path, serde_json::to_vec(&receipts).unwrap()).unwrap();
    }
}

#[test]
#[ignore = "requires independently verified complete continuous-word onset carrier"]
fn verified_full200_original_carrier_numeric_preflight() {
    use sha2::{Digest, Sha256};
    let input = std::fs::read(std::env::var("CONDUIT_FARGAN_FULL200_QUERIES").unwrap()).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(&input)), "3c698c51a560bf3fad61740101decff6cf66957ebf60a1567e4380e130a66a2f");
    assert!(input.len() < 32 * 1024 * 1024);
    let original: serde_json::Value = serde_json::from_slice(&input).unwrap();
    let queries = original["queries"].as_array().unwrap();
    assert_eq!(queries.len(), 200);
    assert_eq!(original["sample_rate_hz"], 16000);
    assert_eq!(original["query_interval_frames"], 160);
    let basis: Rc<[u8]> = Rc::from(serde_json::to_vec(&serde_json::json!({
        "original_intent":original["original_intent"], "realized_intent":original["realized_intent"],
        "rich_accepted":original["rich_accepted"], "word_admissions":original["word_admissions"],
        "initializations":original["initializations"], "source_programs":original["source_programs"], "files":original["files"]
    })).unwrap());
    let projector = RichPeriodProjector::prepare(true).unwrap();
    let d = &projector.document;
    let mut receipts = Vec::with_capacity(200);
    let mut constants = 0;
    let mut varying = 0;
    for (index, query) in queries.iter().enumerate() {
        assert_eq!(query["query_index"], index);
        assert_eq!(query["start_frame"], index * 160);
        assert_eq!(query["end_frame"], (index + 1) * 160);
        let segment = query["segment"].as_u64().unwrap() as usize;
        assert_eq!(segment, index / 20);
        assert_eq!(query["initialization_index"], segment);
        let projected = if index < 80 {
            assert_eq!(query["basis"], "original_constant_initializer");
            assert!(query["pitch_admission"].as_array().unwrap().is_empty());
            assert!(query["word_admission_index"].is_null());
            constants += 1;
            let bytes: Vec<u8> = serde_json::from_value(original["initializations"][segment]["q8_initialization"].clone()).unwrap();
            let request = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
            field(&request, "projected").clone()
        } else {
            assert_eq!(query["basis"], "continuous_word_pitch_frame");
            assert_eq!(query["word_admission_index"], segment - 4);
            varying += 1;
            let bytes: Vec<u8> = serde_json::from_value(query["pitch_admission"].clone()).unwrap();
            let admission = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
            assert_eq!(number(field(field(&admission, "original"), "global_frame")), index as u64 * 160);
            field(&admission, "projected").clone()
        };
        let cycle = field(field(&projected, "request"), "cycle");
        let n = u128::from(number(field(cycle,"numerator_seconds")));
        let den = u128::from(number(field(cycle,"denominator")));
        let expected = (2*n*16000+den)/(2*den);
        let receipt = PreparedRichPeriod::prepare_shared(&projector,&projected.canonical_bytes().unwrap(),&scalar(index as u64*160).canonical_bytes().unwrap(),Rc::clone(&basis),&serde_json::to_vec(query).unwrap(),index as u64,(named(ty(d,"FarganRichPeriodPolicy"),"nearest_whole_sample_ties_up"),named(ty(d,"FarganRichPeriodCadence"),"epoch_onset_160_frames_at_16000_hz"))).unwrap();
        assert_eq!(u128::from(number(field(receipt.result(),"period"))),expected);
        assert!(Rc::ptr_eq(&receipt.original_rich_basis,&basis));
        receipts.push(serde_json::json!({"query_index":index,"original_q8":receipt.original_q8,"original_query_frame":receipt.original_query_frame,"eligible":receipt.eligible,"arithmetic":receipt.arithmetic,"raw":receipt.raw,"admitted_result":receipt.result.canonical_bytes().unwrap(),"admitted_model_period":receipt.period.canonical_bytes().unwrap(),"core_numeric_fidelity":projection::report_material(&receipt),"basis_kind":query["basis"]}));
    }
    assert_eq!((constants,varying),(80,120));
    if let Ok(path)=std::env::var("CONDUIT_FARGAN_FULL200_PREFLIGHT_OUTPUT") {
        std::fs::write(path,serde_json::to_vec(&serde_json::json!({"original_artifact_sha256":"3c698c51a560bf3fad61740101decff6cf66957ebf60a1567e4380e130a66a2f","shared_original_basis":basis.as_ref(),"source":source(),"numeric_program":projector.program.as_ref(),"receipts":receipts,"scope":"200 exact original onset inputs; Source numeric eligibility/conversion and independent u128; no temporal report or public runtime authorization"})).unwrap()).unwrap();
    }
}
