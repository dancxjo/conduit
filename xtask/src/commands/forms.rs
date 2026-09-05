//! Standing conformance entrance for the explicit reviewed Form inventory.

use crate::cli::GlobalOpts;
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[path = "forms/browser.rs"]
mod browser;
#[path = "forms/combined.rs"]
mod combined;
#[path = "forms/composition.rs"]
mod composition;
#[path = "forms/deterministic.rs"]
mod deterministic;
#[path = "forms/inventory.rs"]
mod inventory;
#[path = "forms/report.rs"]
mod report;
#[path = "forms/reusable.rs"]
mod reusable;
#[cfg(test)]
#[path = "forms/tests.rs"]
mod tests;

use inventory::load_inventory;

const INVENTORY_PATH: &str = "forms/inventory.toml";
const INVENTORY_SCHEMA: &str = "conduit.reviewed-form-inventory/v1";
const REPORT_SCHEMA: &str = "conduit.form-conformance-report/v5";

#[derive(Args, Debug)]
pub struct FormsArgs {
    #[command(subcommand)]
    command: FormsCommand,
}

#[derive(Subcommand, Debug)]
enum FormsCommand {
    /// Check every explicitly reviewed canonical Form.
    Check,
    /// Execute every declared Form oracle valid for the selected proof mode.
    Run {
        /// Run deterministic, non-device conformance oracles.
        #[arg(long, conflicts_with = "browser")]
        deterministic: bool,
        /// Report browser proof availability without acquiring permissions or devices.
        #[arg(long, conflicts_with = "deterministic")]
        browser: bool,
    },
    /// Emit the current bounded conformance report without running gated proofs.
    Report {
        /// Write JSON to this path instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Package the reviewed initial Body workload for Crèche.
    BundleInitialBody {
        /// Exact destination for the checked concatenated source document.
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Debug, Deserialize)]
pub(super) struct Inventory {
    schema: String,
    maximum_forms: usize,
    maximum_combined_workloads: usize,
    pub(super) forms: Vec<InventoryForm>,
    pub(super) combined_workloads: Vec<CombinedWorkload>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CombinedWorkload {
    pub(super) slug: String,
    pub(super) title: String,
    pub(super) workload_revision: u64,
    pub(super) entries: Vec<CombinedWorkloadEntry>,
    pub(super) deterministic: DeterministicOracle,
}

#[derive(Debug, Deserialize)]
pub(super) struct CombinedWorkloadEntry {
    pub(super) slug: String,
    pub(super) entry: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct InventoryForm {
    pub(super) slug: String,
    pub(super) title: String,
    pub(super) entry: String,
    #[serde(default)]
    pub(super) reusable_entries: Vec<ReusableForm>,
    #[serde(default)]
    initial_body_order: Option<u8>,
    pub(super) deterministic: Option<DeterministicOracle>,
    pub(super) deterministic_not_applicable: Option<String>,
    pub(super) browser_safe: Option<BrowserOracle>,
    pub(super) browser_safe_not_applicable: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReusableForm {
    pub(super) entry: String,
    pub(super) title: String,
    pub(super) composition: Option<CompositionOracle>,
    pub(super) deterministic: Option<DeterministicOracle>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CompositionOracle {
    pub(super) parent: String,
    pub(super) occurrences: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DeterministicOracle {
    pub(super) package: String,
    #[serde(default)]
    pub(super) features: Vec<String>,
    pub(super) test: String,
    pub(super) case: String,
    #[serde(default)]
    pub(super) plan_play_evidence: bool,
    #[serde(default)]
    pub(super) workload_revision_evidence: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct BrowserOracle {
    pub(super) spec: String,
    pub(super) case: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    inventory_schema: String,
    proof_process_starts: usize,
    proof_process_starts_avoided: usize,
    results: Vec<FormProofResult>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct FormProofResult {
    slug: String,
    title: String,
    source_path: String,
    form_entry: String,
    source_document_id: Option<String>,
    checked_form_id: Option<String>,
    composition_root_entry: Option<String>,
    composition_root_checked_form_id: Option<String>,
    gear_occurrences: Vec<String>,
    proof_mode: &'static str,
    environment_profile: &'static str,
    duration_millis: u128,
    workload_slug: Option<String>,
    workload_title: Option<String>,
    workload_revision: Option<u64>,
    plan_id: Option<String>,
    play_id: Option<String>,
    status: String,
    reason: String,
    evidence_artifacts: Vec<String>,
}

pub fn run(args: FormsArgs, opts: &GlobalOpts) -> Result<(), String> {
    let root = crate::workspace::workspace_root()?;
    match args.command {
        FormsCommand::Check => {
            let report = build_report(&root, false, opts)?;
            render(&report, opts.json)?;
            if report
                .results
                .iter()
                .any(|result| result.status == "failed")
            {
                return Err("one or more reviewed Forms failed conformance checking".into());
            }
        }
        FormsCommand::Run {
            deterministic,
            browser,
        } => {
            if !deterministic && !browser {
                return Err(
                    "select exactly one proof mode with --deterministic or --browser".into(),
                );
            }
            let report = if deterministic {
                build_report(&root, true, opts)?
            } else {
                browser::build_report(&root, opts)?
            };
            render(&report, true)?;
            if report
                .results
                .iter()
                .any(|result| result.status == "failed")
            {
                return Err("one or more reviewed Form proofs failed".into());
            }
        }
        FormsCommand::Report { output } => {
            let report = report::build(&root)?;
            let bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
            if let Some(path) = output {
                fs::write(path, bytes).map_err(|error| error.to_string())?;
            } else {
                println!("{}", String::from_utf8(bytes).expect("JSON is UTF-8"));
            }
        }
        FormsCommand::BundleInitialBody { output } => bundle_initial_body(&root, &output)?,
    }
    Ok(())
}

fn bundle_initial_body(root: &Path, output: &Path) -> Result<(), String> {
    let inventory = load_inventory(root)?;
    let mut selected: Vec<_> = inventory
        .forms
        .iter()
        .filter(|form| form.initial_body_order.is_some())
        .collect();
    selected.sort_by_key(|form| form.initial_body_order);
    if selected.is_empty() || selected.len() > conduit_body::MAX_BODY_FORMS {
        return Err("initial Body inventory is empty or exceeds Body capacity".into());
    }
    if selected
        .iter()
        .enumerate()
        .any(|(index, form)| form.initial_body_order != Some((index + 1) as u8))
    {
        return Err("initial Body inventory order must be unique and contiguous from one".into());
    }
    #[derive(Serialize)]
    struct BundledForm<'a> {
        slug: &'a str,
        source: String,
    }
    #[derive(Serialize)]
    struct InitialBodyBundle<'a> {
        schema: &'static str,
        forms: Vec<BundledForm<'a>>,
    }
    let mut forms = Vec::with_capacity(selected.len());
    for form in selected {
        let path = root.join("forms").join(&form.slug).join("main.conduit");
        let source =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        forms.push(BundledForm {
            slug: &form.slug,
            source,
        });
    }
    let bundled = serde_json::to_vec_pretty(&InitialBodyBundle {
        schema: "conduit.creche/reviewed-form-bundle@1",
        forms,
    })
    .map_err(|error| error.to_string())?;
    fs::write(output, bundled).map_err(|error| error.to_string())
}

fn build_report(
    root: &Path,
    execute_deterministic: bool,
    opts: &GlobalOpts,
) -> Result<Report, String> {
    let inventory = load_inventory(root)?;
    let catalogs = catalogs()?;
    let mut results = Vec::with_capacity(inventory.forms.len() * 2);
    for form in &inventory.forms {
        let source_path = format!("forms/{}/main.conduit", form.slug);
        let started = Instant::now();
        let checked = check_one(root, &source_path, &form.entry, &catalogs);
        let elapsed = started.elapsed().as_millis();
        match checked {
            Ok((source_id, checked_id)) => {
                results.push(result(
                    form,
                    &source_path,
                    elapsed,
                    "passed",
                    "canonical source parsed and checked through the standard semantic catalog",
                    Some((source_id.clone(), checked_id.clone())),
                    "check",
                ));
                results.push(if execute_deterministic {
                    deterministic::run(
                        root,
                        form,
                        &source_path,
                        Some((source_id, checked_id)),
                        opts,
                    )
                } else {
                    deterministic::availability(form, &source_path, Some((source_id, checked_id)))
                });
            }
            Err(reason) => results.push(result(
                form,
                &source_path,
                elapsed,
                "failed",
                &reason,
                None,
                "check",
            )),
        }
        results.extend(reusable::check_all(root, form, &source_path, &catalogs));
        results.extend(composition::check_all(root, form, &source_path, &catalogs));
        results.extend(reusable::deterministic_all(
            root,
            form,
            &source_path,
            &catalogs,
            execute_deterministic,
            opts,
        ));
    }
    results.extend(combined::results(
        root,
        &inventory,
        &catalogs,
        execute_deterministic,
        opts,
    ));
    Ok(Report {
        schema: REPORT_SCHEMA,
        inventory_schema: inventory.schema,
        proof_process_starts: 0,
        proof_process_starts_avoided: 0,
        results,
    })
}

fn result(
    form: &InventoryForm,
    path: &str,
    duration: u128,
    status: &str,
    reason: &str,
    identities: Option<(String, String)>,
    mode: &'static str,
) -> FormProofResult {
    FormProofResult {
        slug: form.slug.clone(),
        title: form.title.clone(),
        source_path: path.into(),
        form_entry: form.entry.clone(),
        source_document_id: identities.as_ref().map(|item| item.0.clone()),
        checked_form_id: identities.map(|item| item.1),
        composition_root_entry: None,
        composition_root_checked_form_id: None,
        gear_occurrences: Vec::new(),
        proof_mode: mode,
        environment_profile: "repository/standard-semantic-catalog@1",
        duration_millis: duration,
        workload_slug: None,
        workload_title: None,
        workload_revision: None,
        plan_id: None,
        play_id: None,
        status: status.into(),
        reason: reason.into(),
        evidence_artifacts: vec![INVENTORY_PATH.into(), path.into()],
    }
}

fn check_one(
    root: &Path,
    path: &str,
    entry: &str,
    catalogs: &(conduit_form::StartupCatalog, conduit_form::ProfileCatalog),
) -> Result<(String, String), String> {
    let source = fs::read_to_string(root.join(path)).map_err(|error| format!("{path}: {error}"))?;
    let syntax = conduit_form::parse_syntax_document(&source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(format!(
            "{path}: {}: {}",
            diagnostic.code, diagnostic.message
        ));
    }
    let checked = conduit_form::check_syntax_document(&syntax, &catalogs.0)
        .map_err(|error| format!("{path}: {}: {}", error.code, error.message))?;
    let form = checked
        .forms
        .iter()
        .find(|candidate| candidate.name == entry)
        .ok_or_else(|| format!("{path}: declared entry '{entry}' is absent"))?;
    Ok((
        checked.source_document_id.as_str().into(),
        form.checked_form_id.as_str().into(),
    ))
}

fn catalogs() -> Result<(conduit_form::StartupCatalog, conduit_form::ProfileCatalog), String> {
    let mut startup = conduit_signal::primary_signal_startup_catalog();
    let mut profile = conduit_signal::primary_signal_profile_catalog();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_time::install_time_every_catalog(&mut startup, &mut profile)?;
    conduit_time::install_rhythm_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timing_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_flow_state_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_state_toggle_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_logic_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_presentation_composition_catalogs(
        &mut startup,
        &mut profile,
    )?;
    conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_graphics_presentation_catalog(&mut startup, &mut profile)?;
    conduit_presentation::install_bitmap_presentation_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)?;
    conduit_web::install_http_catalogs(&mut startup, &mut profile)?;
    conduit_web::install_json_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_recurrence_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_schedule_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_calendar_provider_catalogs(&mut startup, &mut profile)?;
    conduit_presentation::install_geometry_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profile)?;
    conduit_language::install_linguistics_catalogs(&mut startup, &mut profile)?;
    conduit_data::install_tabular_catalogs(&mut startup, &mut profile)?;
    conduit_data::install_finance_catalogs(&mut startup, &mut profile)?;
    conduit_data::install_measurement_window_catalog(&mut startup, &mut profile)?;
    conduit_data::install_measurement_summary_catalog(&mut startup, &mut profile)?;
    conduit_data::install_measurement_threshold_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_job_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_application_network_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_robotics_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_robotics_structured_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_education_catalogs(&mut startup, &mut profile)?;
    conduit_chat::install_messaging_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_generalized_input_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timed_pattern_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timed_button_attempt_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_sequence_normalization_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_template_storage_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_final_normalized_pattern_catalogs(
        &mut startup,
        &mut profile,
    )?;
    conduit_semantic_catalog::install_pattern_comparison_catalogs(&mut startup, &mut profile)?;
    startup.insert(conduit_form::KindSignature {
        kind: conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    let comparison_presentation = conduit_semantic_catalog::structured_presentation_contract(
        conduit_semantic_catalog::PATTERN_COMPARISON_TYPE,
        &conduit_semantic_catalog::pattern_comparison_type(),
    );
    profile
        .insert(conduit_form::KindDefinition {
            kind_id: comparison_presentation.kind_id,
            kind_contract_revision: comparison_presentation.kind_contract_revision,
            inputs: comparison_presentation.inputs,
            outputs: comparison_presentation.outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| error.to_string())?;
    conduit_alife::install_lenia_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_human_media_catalogs(&mut startup, &mut profile)?;
    conduit_chat::install_pool_chat_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_network_bootstrap_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_reminder_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile)?;
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile)?;
    conduit_tongues::install_research_catalogs(&mut startup, &mut profile)?;
    Ok((startup, profile))
}

fn render(report: &Report, json: bool) -> Result<(), String> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(report).map_err(|error| error.to_string())?
        );
    } else {
        for result in report
            .results
            .iter()
            .filter(|result| result.proof_mode == "check")
        {
            println!(
                "{:8} {} ({})",
                result.status, result.title, result.source_path
            );
        }
    }
    Ok(())
}
