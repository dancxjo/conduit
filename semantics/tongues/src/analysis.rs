//! Bounded post-freeze analysis of the exact paired-latent research artifact.

use crate::analysis_math::{dot, library, millionths, nearest, plv, ridge, signed_millionths};
use crate::{
    train_shared_latent, Pb2007Slice, ProbeSegment, ResearchError, ResearchReport, RESEARCH_SEED,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const MAX_ANALYSIS_FRAMES: usize = 192;
const LAGS: std::ops::RangeInclusive<isize> = -3..=3;

#[derive(Clone, Debug, Serialize)]
pub struct DynamicsAnalysisReport {
    pub schema: String,
    pub identity: String,
    pub source_checkpoint_identity: String,
    pub source_derivation_identity: String,
    pub source_split_profile: String,
    pub extraction_profile: String,
    pub provider_identity: String,
    pub analysis_seed: u64,
    pub work_bound_frames: usize,
    pub phase_lag: PhaseLagEvidence,
    pub events: EventEvidence,
    pub categories: CategoryEvidence,
    pub frozen_probe_accuracy_millionths: u64,
    pub sparse_dynamics: SparseDynamicsEvidence,
    pub robustness: RobustnessEvidence,
    pub theory_comparisons: Vec<TheoryComparison>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhaseLagEvidence {
    pub method: String,
    pub phase_locking_millionths: u64,
    pub pairing_shuffled_millionths: u64,
    pub best_lag_bins: i64,
    pub best_correlation_millionths: i64,
    pub assumptions: Vec<String>,
    pub relative_phase_milliradians: Vec<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EventEvidence {
    pub detector: String,
    pub discovered_events: usize,
    pub post_hoc_boundaries: usize,
    pub aligned_within_one_bin: usize,
    pub systematically_misaligned: usize,
    pub event_bins: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CategoryEvidence {
    pub method: String,
    pub clusters: usize,
    pub test_frames: usize,
    pub post_hoc_purity_millionths: u64,
    pub assignment_entropy_millionths: u64,
    pub labels_visible_during_clustering: bool,
    pub test_assignments: Vec<usize>,
    pub post_hoc_labels: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SparseDynamicsEvidence {
    pub library: Vec<String>,
    pub coefficients: Vec<Vec<i64>>,
    pub nonzero_terms: usize,
    pub held_out_mse_millionths: u64,
    pub constant_state_baseline_millionths: u64,
    pub interpretation: String,
    pub held_out_observed_delta_millionths: Vec<i64>,
    pub held_out_predicted_delta_millionths: Vec<i64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RobustnessEvidence {
    pub alternate_seed: u64,
    pub alternate_checkpoint_identity: String,
    pub alternate_phase_locking_millionths: u64,
    pub cross_speaker: String,
    pub front_end: String,
    pub negative_controls: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TheoryComparison {
    pub hypothesis: String,
    pub disposition: String,
    pub evidence: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisError {
    Research(ResearchError),
    InvalidData,
    WorkBoundExceeded,
}

impl From<ResearchError> for AnalysisError {
    fn from(value: ResearchError) -> Self {
        Self::Research(value)
    }
}

pub fn run_dynamics_analysis() -> Result<DynamicsAnalysisReport, AnalysisError> {
    let source = crate::run_research()?;
    let corpus = Pb2007Slice::load().map_err(|_| AnalysisError::InvalidData)?;
    let utterances = corpus.training_utterances();
    let (checkpoint, _) =
        train_shared_latent(&utterances, RESEARCH_SEED).map_err(|_| AnalysisError::InvalidData)?;
    let (alternate, _) = train_shared_latent(&utterances, RESEARCH_SEED + 1)
        .map_err(|_| AnalysisError::InvalidData)?;
    if source.training.checkpoint_identity
        != hex(checkpoint
            .identity()
            .map_err(|_| AnalysisError::InvalidData)?)
    {
        return Err(AnalysisError::InvalidData);
    }
    let mut rows = Vec::new();
    for utterance in &corpus.utterances {
        for (bin, (audio, articulation)) in utterance
            .acoustic
            .iter()
            .zip(&utterance.articulation)
            .enumerate()
        {
            rows.push(Row {
                utterance: utterance.identity.as_str(),
                split: utterance.split.as_str(),
                bin,
                latent: checkpoint
                    .encode_acoustic(audio)
                    .map_err(|_| AnalysisError::InvalidData)?
                    .mean,
                alternate: alternate
                    .encode_acoustic(audio)
                    .map_err(|_| AnalysisError::InvalidData)?
                    .mean,
                articulation: articulation.iter().map(|value| *value as f64).collect(),
                label: label_at(&utterance.post_freeze_probe_labels, bin),
            });
        }
    }
    if rows.len() > MAX_ANALYSIS_FRAMES {
        return Err(AnalysisError::WorkBoundExceeded);
    }
    let phase_lag_evidence = phase_lag(&rows, false)?;
    let alternate_phase = phase_lag(&rows, true)?.phase_locking_millionths;
    let events = events(&rows);
    let categories = categories(&rows)?;
    let sparse_dynamics = sparse_dynamics(&rows)?;
    let robustness = RobustnessEvidence {
        alternate_seed: RESEARCH_SEED + 1,
        alternate_checkpoint_identity: hex(alternate
            .identity()
            .map_err(|_| AnalysisError::InvalidData)?),
        alternate_phase_locking_millionths: alternate_phase,
        cross_speaker: "not-identifiable: the admitted PB2007 slice contains one speaker".into(),
        front_end: "not-tested: #2145 uses one documented handcrafted acoustic front end".into(),
        negative_controls: vec![
            "pairing-reversed phase locking is reported beside observed pairing".into(),
            "constant-state framewise prediction is reported beside sparse dynamics".into(),
            "alternate seed checkpoint is analyzed without selecting the better result".into(),
        ],
    };
    let mut report = DynamicsAnalysisReport {
        schema: "conduit.tongues/dynamics-analysis-report@1".into(),
        identity: String::new(),
        source_checkpoint_identity: source.training.checkpoint_identity.clone(),
        source_derivation_identity: corpus.derivation.identity.clone(),
        source_split_profile: "PB2007-derived/train-8-validation-2-test-2@1".into(),
        extraction_profile: "acoustic-encoder/continuous-z2/all-splits@1".into(),
        provider_identity: "conduit-tongues/std-deterministic-analysis@1".into(),
        analysis_seed: RESEARCH_SEED,
        work_bound_frames: MAX_ANALYSIS_FRAMES,
        phase_lag: phase_lag_evidence,
        events,
        categories,
        frozen_probe_accuracy_millionths: source.post_freeze_probe.accuracy_millionths,
        sparse_dynamics,
        robustness,
        theory_comparisons: theory_comparisons(),
        limitations: limitations(&source),
    };
    report.identity = hex(Sha256::digest(
        serde_json::to_vec(&report).map_err(|_| AnalysisError::InvalidData)?,
    )
    .into());
    Ok(report)
}

pub fn run_dynamics_analysis_json() -> Result<String, AnalysisError> {
    serde_json::to_string_pretty(&run_dynamics_analysis()?).map_err(|_| AnalysisError::InvalidData)
}

struct Row<'a> {
    utterance: &'a str,
    split: &'a str,
    bin: usize,
    latent: Vec<f64>,
    alternate: Vec<f64>,
    articulation: Vec<f64>,
    label: &'a str,
}

fn phase_lag(rows: &[Row<'_>], alternate: bool) -> Result<PhaseLagEvidence, AnalysisError> {
    let selected = rows
        .iter()
        .filter(|row| row.split == "test")
        .collect::<Vec<_>>();
    let latent = selected
        .iter()
        .map(|row| {
            if alternate {
                &row.alternate
            } else {
                &row.latent
            }
        })
        .collect::<Vec<_>>();
    let latent_center = [
        latent.iter().map(|value| value[0]).sum::<f64>() / latent.len() as f64,
        latent.iter().map(|value| value[1]).sum::<f64>() / latent.len() as f64,
    ];
    let articulation_center = [
        selected.iter().map(|row| row.articulation[0]).sum::<f64>() / selected.len() as f64,
        selected.iter().map(|row| row.articulation[1]).sum::<f64>() / selected.len() as f64,
    ];
    let relative = latent
        .iter()
        .zip(&selected)
        .map(|(z, row)| {
            (z[1] - latent_center[1]).atan2(z[0] - latent_center[0])
                - (row.articulation[1] - articulation_center[1])
                    .atan2(row.articulation[0] - articulation_center[0])
        })
        .collect::<Vec<_>>();
    let shuffled = latent
        .iter()
        .zip(selected.iter().rev())
        .map(|(z, row)| {
            (z[1] - latent_center[1]).atan2(z[0] - latent_center[0])
                - (row.articulation[1] - articulation_center[1])
                    .atan2(row.articulation[0] - articulation_center[0])
        })
        .collect::<Vec<_>>();
    let (best_lag, best_correlation) = LAGS
        .map(|lag| (lag, lag_correlation(&selected, lag)))
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .ok_or(AnalysisError::InvalidData)?;
    Ok(PhaseLagEvidence {
        method: "descriptive atan2 phase on centered two-coordinate projections; Pearson lag over -3..3 bins@1".into(),
        phase_locking_millionths: millionths(plv(&relative)), pairing_shuffled_millionths: millionths(plv(&shuffled)),
        best_lag_bins: best_lag as i64, best_correlation_millionths: signed_millionths(best_correlation),
        assumptions: vec!["phase is descriptive only; no latent oscillator is asserted".into(), "the two projections are numerically centered but not Hilbert-phase estimates".into(), "one 100 Hz bin is 10 ms".into()],
        relative_phase_milliradians: relative.iter().map(|value| (value * 1_000.0).round() as i64).collect(),
    })
}

fn events(rows: &[Row<'_>]) -> EventEvidence {
    let test = rows
        .iter()
        .filter(|row| row.split == "test")
        .collect::<Vec<_>>();
    let mut found = Vec::new();
    for window in test.windows(3) {
        if window[0].utterance == window[2].utterance {
            let before = window[1].latent[0] - window[0].latent[0];
            let after = window[2].latent[0] - window[1].latent[0];
            if before.signum() != after.signum() {
                found.push(window[1].bin);
            }
        }
    }
    let boundaries = test
        .iter()
        .filter(|row| row.bin > 0)
        .filter(|row| {
            test.iter()
                .find(|prior| prior.utterance == row.utterance && prior.bin + 1 == row.bin)
                .is_some_and(|prior| prior.label != row.label)
        })
        .map(|row| row.bin)
        .collect::<Vec<_>>();
    let aligned = found
        .iter()
        .filter(|event| {
            boundaries
                .iter()
                .any(|boundary| event.abs_diff(*boundary) <= 1)
        })
        .count();
    EventEvidence {
        detector: "label-free latent-z0 velocity sign change@1".into(),
        discovered_events: found.len(),
        post_hoc_boundaries: boundaries.len(),
        aligned_within_one_bin: aligned,
        systematically_misaligned: found.len().saturating_sub(aligned),
        event_bins: found,
    }
}

fn categories(rows: &[Row<'_>]) -> Result<CategoryEvidence, AnalysisError> {
    let train = rows
        .iter()
        .filter(|row| row.split == "train")
        .collect::<Vec<_>>();
    let test = rows
        .iter()
        .filter(|row| row.split == "test")
        .collect::<Vec<_>>();
    let mut centers = [
        train[0].latent.clone(),
        train[train.len() / 2].latent.clone(),
        train[train.len() - 1].latent.clone(),
    ];
    for _ in 0..8 {
        let mut sums = [[0.0; 2]; 3];
        let mut counts = [0usize; 3];
        for row in &train {
            let k = nearest(&row.latent, &centers);
            counts[k] += 1;
            for (sum, value) in sums[k].iter_mut().zip(&row.latent) {
                *sum += value;
            }
        }
        for k in 0..3 {
            if counts[k] > 0 {
                centers[k] = sums[k].iter().map(|v| v / counts[k] as f64).collect();
            }
        }
    }
    let mut cells: BTreeMap<(usize, &str), usize> = BTreeMap::new();
    let mut cluster_counts = [0usize; 3];
    let mut assignments = Vec::with_capacity(test.len());
    for row in &test {
        let k = nearest(&row.latent, &centers);
        assignments.push(k);
        *cells.entry((k, row.label)).or_default() += 1;
        cluster_counts[k] += 1;
    }
    let correct = (0..3)
        .map(|k| {
            cells
                .iter()
                .filter(|((cluster, _), _)| *cluster == k)
                .map(|(_, count)| *count)
                .max()
                .unwrap_or(0)
        })
        .sum::<usize>();
    let entropy = cluster_counts
        .iter()
        .filter(|count| **count > 0)
        .map(|count| {
            let p = *count as f64 / test.len() as f64;
            -p * p.ln()
        })
        .sum::<f64>()
        / (3.0f64).ln();
    Ok(CategoryEvidence { method: "three-centroid deterministic k-means over frozen continuous train latents; labels consulted on test only@1".into(), clusters: 3, test_frames: test.len(), post_hoc_purity_millionths: millionths(correct as f64 / test.len() as f64), assignment_entropy_millionths: millionths(entropy), labels_visible_during_clustering: false, test_assignments: assignments, post_hoc_labels: test.iter().map(|row| row.label.into()).collect() })
}

fn sparse_dynamics(rows: &[Row<'_>]) -> Result<SparseDynamicsEvidence, AnalysisError> {
    let pairs = |split| {
        rows.windows(2)
            .filter(move |pair| pair[0].split == split && pair[0].utterance == pair[1].utterance)
            .collect::<Vec<_>>()
    };
    let train = pairs("train");
    let test = pairs("test");
    let x = train
        .iter()
        .map(|pair| library(&pair[0].latent))
        .collect::<Vec<_>>();
    let y = (0..2)
        .map(|dimension| {
            train
                .iter()
                .map(|pair| pair[1].latent[dimension] - pair[0].latent[dimension])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let coefficients = y
        .iter()
        .map(|target| ridge(&x, target, 0.01))
        .collect::<Result<Vec<_>, _>>()?;
    let sparse = coefficients
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|value| if value.abs() < 0.1 { 0.0 } else { value })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut error = 0.0;
    let mut baseline = 0.0;
    let mut observed = Vec::new();
    let mut predicted_values = Vec::new();
    for pair in &test {
        let features = library(&pair[0].latent);
        for (dimension, coefficients) in sparse.iter().enumerate() {
            let actual = pair[1].latent[dimension] - pair[0].latent[dimension];
            let predicted = dot(&features, coefficients);
            observed.push((actual * 1_000_000.0).round() as i64);
            predicted_values.push((predicted * 1_000_000.0).round() as i64);
            error += (actual - predicted).powi(2);
            baseline += actual.powi(2);
        }
    }
    let count = (test.len() * 2) as f64;
    Ok(SparseDynamicsEvidence {
        library: vec![
            "1".into(),
            "z0".into(),
            "z1".into(),
            "z0*z1".into(),
            "z0^2".into(),
            "z1^2".into(),
        ],
        coefficients: sparse
            .iter()
            .map(|row| {
                row.iter()
                    .map(|v| (v * 1_000_000.0).round() as i64)
                    .collect()
            })
            .collect(),
        nonzero_terms: sparse.iter().flatten().filter(|v| **v != 0.0).count(),
        held_out_mse_millionths: millionths(error / count),
        constant_state_baseline_millionths: millionths(baseline / count),
        interpretation: "predictive sparse association on held-out utterances; not a causal law"
            .into(),
        held_out_observed_delta_millionths: observed,
        held_out_predicted_delta_millionths: predicted_values,
    })
}

fn lag_correlation(rows: &[&Row<'_>], lag: isize) -> f64 {
    let pairs = (0..rows.len())
        .filter_map(|i| {
            let j = i as isize + lag;
            (j >= 0 && (j as usize) < rows.len() && rows[i].utterance == rows[j as usize].utterance)
                .then(|| (rows[i].latent[0], rows[j as usize].articulation[0]))
        })
        .collect::<Vec<_>>();
    correlation(&pairs)
}
fn correlation(pairs: &[(f64, f64)]) -> f64 {
    if pairs.len() < 2 {
        return 0.0;
    }
    let mx = pairs.iter().map(|p| p.0).sum::<f64>() / pairs.len() as f64;
    let my = pairs.iter().map(|p| p.1).sum::<f64>() / pairs.len() as f64;
    let numerator = pairs.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>();
    let denominator = (pairs.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>()
        * pairs.iter().map(|p| (p.1 - my).powi(2)).sum::<f64>())
    .sqrt();
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}
fn label_at(segments: &[ProbeSegment], bin: usize) -> &str {
    segments
        .iter()
        .find(|s| s.start_bin <= bin && bin < s.end_bin)
        .map_or("__", |s| s.label.as_str())
}
fn hex(identity: [u8; 32]) -> String {
    format!(
        "sha256:{}",
        identity
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
fn theory_comparisons() -> Vec<TheoryComparison> {
    vec![TheoryComparison{hypothesis:"stable 0 or 180 degree coupled-oscillator organization".into(),disposition:"not-identifiable".into(),evidence:"descriptive phase is measured, but the tiny learned coordinates are not established oscillators".into()},TheoryComparison{hypothesis:"learned events coincide with conventional segment boundaries".into(),disposition:"partially-supported-or-contradicted".into(),evidence:"both aligned and systematically misaligned events are reported without selecting only matches".into()},TheoryComparison{hypothesis:"C-center onset organization".into(),disposition:"not-identifiable".into(),evidence:"the slice lacks a controlled onset-cluster/context comparison".into()}]
}
fn limitations(source: &ResearchReport) -> Vec<String> {
    let mut values = source.limitations.clone();
    values.extend([
        "phase extraction does not establish oscillation or directed influence".into(),
        "one speaker prevents cross-speaker stability claims".into(),
        "post-hoc purity does not establish natural linguistic categories".into(),
        "sparse predictive equations are associations, not causal laws".into(),
    ]);
    values
}
