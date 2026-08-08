use conduit_core::{
    CapabilityId, ConnectionProvider, OperationId, ProtectedResourceAccess,
    ProtectedResourceCommitPolicy, ResourceBindingRoleId, ResourceHandleId,
};
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
use conduit_std_host::{
    CopyRequestId, CopyResult, CopyRunReceipt, CopyStopToken, ProtectedFileAvailability,
    ProtectedFileRegistry, StdHost,
};
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, TryRecvError};
use std::time::Duration;

const COPY_FORM_SOURCE: &str = "form 0\n\ncopy-task {\n    copy: file/copy\n}\n";
const DEFAULT_MAXIMUM_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DestinationMode {
    Create,
    Replace,
}

struct Arguments {
    source: PathBuf,
    destination: PathBuf,
    mode: DestinationMode,
    maximum_bytes: u64,
    run_without_prompt: bool,
    inspect: bool,
}

struct PreparedTask {
    host: StdHost,
    form: conduit_form::CheckedForm,
    plan: conduit_core::Plan,
    fragment: conduit_core::PlanFragment,
    registry: ProtectedFileRegistry,
    stop: CopyStopToken,
}

pub(crate) fn run(raw_arguments: Vec<String>) -> Result<(), String> {
    let arguments = parse_arguments(raw_arguments)?;
    let task = prepare(&arguments)?;
    let mut stdout = io::stdout().lock();
    render_readiness(&mut stdout, &arguments, &task)?;

    if arguments.run_without_prompt {
        let inspect_form = task.form.clone();
        let inspect_plan = task.plan.clone();
        writeln!(stdout, "Running now.").map_err(|error| error.to_string())?;
        let receipt = execute(task)?;
        render_result(&mut stdout, &receipt)?;
        if arguments.inspect {
            render_inspect(
                &mut stdout,
                &receipt,
                &arguments,
                &inspect_form,
                &inspect_plan,
            )?;
        }
        return Ok(());
    }

    writeln!(
        stdout,
        "Action: type 'run' to Run, or 'quit' to leave without copying."
    )
    .map_err(|error| error.to_string())?;
    stdout.flush().map_err(|error| error.to_string())?;
    let mut command = String::new();
    io::stdin()
        .read_line(&mut command)
        .map_err(|error| error.to_string())?;
    if !command.trim().eq_ignore_ascii_case("run") {
        writeln!(stdout, "No copy was run.").map_err(|error| error.to_string())?;
        return Ok(());
    }

    let inspect_form = task.form.clone();
    let inspect_plan = task.plan.clone();
    let stop = task.stop.clone();
    writeln!(stdout, "Running. Type 'stop' and press Enter to Stop.")
        .map_err(|error| error.to_string())?;
    stdout.flush().map_err(|error| error.to_string())?;
    let (result_sender, result_receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = result_sender.send(execute(task));
    });
    let (input_sender, input_receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            if input_sender.send(line.unwrap_or_default()).is_err() {
                break;
            }
        }
    });
    let receipt = loop {
        match result_receiver.try_recv() {
            Ok(result) => break result?,
            Err(TryRecvError::Disconnected) => {
                return Err("copy worker ended without a result".to_string());
            }
            Err(TryRecvError::Empty) => {}
        }
        if let Ok(input) = input_receiver.try_recv() {
            if input.trim().eq_ignore_ascii_case("stop") {
                stop.request_stop();
                writeln!(
                    stdout,
                    "Stop requested; finishing the current bounded step."
                )
                .map_err(|error| error.to_string())?;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    render_result(&mut stdout, &receipt)?;
    if arguments.inspect {
        render_inspect(
            &mut stdout,
            &receipt,
            &arguments,
            &inspect_form,
            &inspect_plan,
        )?;
    } else if matches!(receipt.result, CopyResult::Success { .. }) {
        writeln!(
            stdout,
            "Next: rerun with --inspect to reveal the Form and exact Plan."
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn parse_arguments(raw: Vec<String>) -> Result<Arguments, String> {
    let usage = "usage: conduit copy <source-file> <destination-file> [--mode create|replace] [--max-bytes N] [--run] [--inspect]";
    let source = raw
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| usage.to_string())?;
    let destination = raw
        .get(1)
        .map(PathBuf::from)
        .ok_or_else(|| usage.to_string())?;
    let mut mode = DestinationMode::Create;
    let mut maximum_bytes = DEFAULT_MAXIMUM_BYTES;
    let mut run_without_prompt = false;
    let mut inspect = false;
    let mut index = 2;
    while index < raw.len() {
        match raw[index].as_str() {
            "--mode" => {
                index += 1;
                mode = match raw.get(index).map(String::as_str) {
                    Some("create") => DestinationMode::Create,
                    Some("replace") => DestinationMode::Replace,
                    _ => return Err(usage.to_string()),
                };
            }
            "--max-bytes" => {
                index += 1;
                maximum_bytes = raw
                    .get(index)
                    .ok_or_else(|| usage.to_string())?
                    .parse::<u64>()
                    .map_err(|_| "--max-bytes must be a positive integer".to_string())?;
                if maximum_bytes == 0 || maximum_bytes > DEFAULT_MAXIMUM_BYTES {
                    return Err(format!(
                        "--max-bytes must be between 1 and {DEFAULT_MAXIMUM_BYTES}"
                    ));
                }
            }
            "--run" => run_without_prompt = true,
            "--inspect" => inspect = true,
            _ => return Err(usage.to_string()),
        }
        index += 1;
    }
    Ok(Arguments {
        source,
        destination,
        mode,
        maximum_bytes,
        run_without_prompt,
        inspect,
    })
}

fn prepare(arguments: &Arguments) -> Result<PreparedTask, String> {
    let host = StdHost::new();
    let mut catalog = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_copy_file_catalog(&mut catalog)?;
    let form =
        conduit_form::parse(COPY_FORM_SOURCE, &catalog).map_err(|error| error.to_string())?;
    let placements = default_placements(&form, std::slice::from_ref(host.advertisement()))
        .map_err(|error| error.to_string())?;
    let mut registry = ProtectedFileRegistry::default();
    let operation_id = OperationId::from("copy");
    let capability_id = CapabilityId::from(conduit_std_catalog::COPY_FILE_CAPABILITY);
    let source = registry.register(
        ResourceHandleId::from("copy/source-choice"),
        &arguments.source,
        operation_id.clone(),
        ResourceBindingRoleId::from(conduit_std_catalog::COPY_SOURCE_ROLE),
        host.advertisement().host_id.clone(),
        host.advertisement().boot_id.clone(),
        capability_id.clone(),
        ProtectedResourceAccess::ReadExisting,
        arguments.maximum_bytes,
        ProtectedResourceCommitPolicy::NotApplicable,
        ProtectedFileAvailability::Available,
    )?;
    let (access, policy) = match arguments.mode {
        DestinationMode::Create => (
            ProtectedResourceAccess::Create,
            ProtectedResourceCommitPolicy::CreateOnly,
        ),
        DestinationMode::Replace => (
            ProtectedResourceAccess::Replace,
            ProtectedResourceCommitPolicy::ReplaceExisting,
        ),
    };
    let destination = registry.register(
        ResourceHandleId::from("copy/destination-choice"),
        &arguments.destination,
        operation_id,
        ResourceBindingRoleId::from(conduit_std_catalog::COPY_DESTINATION_ROLE),
        host.advertisement().host_id.clone(),
        host.advertisement().boot_id.clone(),
        capability_id,
        access,
        arguments.maximum_bytes,
        policy,
        ProtectedFileAvailability::Available,
    )?;
    let overrides = BTreeMap::new();
    let plan = plan_with_options(
        &form,
        std::slice::from_ref(host.advertisement()),
        &placements,
        &[ConnectionProvider::Local],
        PlanningOptions {
            connection_providers: &overrides,
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[source, destination],
            link_bindings: &[],
        },
    )
    .map_err(|error| error.to_string())?;
    let fragment = plan
        .fragments
        .first()
        .cloned()
        .ok_or_else(|| "copy Plan has no local fragment".to_string())?;
    Ok(PreparedTask {
        host,
        form,
        plan,
        fragment,
        registry,
        stop: CopyStopToken::default(),
    })
}

fn execute(mut task: PreparedTask) -> Result<CopyRunReceipt, String> {
    task.host.run_copy_fragment(
        CopyRequestId::new("copy/request-1")?,
        task.fragment,
        &task.registry,
        &task.stop,
    )
}

fn render_readiness(
    output: &mut impl Write,
    arguments: &Arguments,
    task: &PreparedTask,
) -> Result<(), String> {
    writeln!(output, "Copy a file").map_err(|error| error.to_string())?;
    writeln!(output, "Source: {}", arguments.source.display()).map_err(|e| e.to_string())?;
    writeln!(output, "Destination: {}", arguments.destination.display())
        .map_err(|e| e.to_string())?;
    writeln!(output, "Target platform: local std-host filesystem")
        .map_err(|error| error.to_string())?;
    let behavior = match arguments.mode {
        DestinationMode::Create => "Create new; reject if the destination already exists",
        DestinationMode::Replace => "Replace the destination at final commit",
    };
    writeln!(output, "Behavior: {behavior}").map_err(|error| error.to_string())?;
    writeln!(output, "Maximum size: {} bytes", arguments.maximum_bytes)
        .map_err(|error| error.to_string())?;
    writeln!(
        output,
        "Ready: yes — choices are protected and Plan {} is prepared.",
        task.plan.plan_id.as_str()
    )
    .map_err(|error| error.to_string())
}

fn render_result(output: &mut impl Write, receipt: &CopyRunReceipt) -> Result<(), String> {
    let message = match receipt.result {
        CopyResult::Success { bytes_copied } => {
            format!("Copied {bytes_copied} bytes successfully.")
        }
        CopyResult::DestinationExists => "Not copied: destination already exists.".to_string(),
        CopyResult::Denied => "Not copied: access was denied.".to_string(),
        CopyResult::StaleHandle => "Not copied: a selected resource is stale.".to_string(),
        CopyResult::Oversized {
            source_bytes,
            maximum_bytes,
        } => format!(
            "Not copied: source is {source_bytes} bytes, above the {maximum_bytes}-byte limit."
        ),
        CopyResult::Partial { bytes_copied } => {
            format!(
                "Copy failed after {bytes_copied} temporary bytes; destination was not committed."
            )
        }
        CopyResult::Cancelled { bytes_copied } => {
            format!("Stopped after {bytes_copied} temporary bytes; destination was not committed.")
        }
        CopyResult::CleanupFailed { bytes_copied } => format!(
            "Copy stopped after {bytes_copied} temporary bytes, but temporary cleanup failed."
        ),
    };
    writeln!(output, "Result: {message}").map_err(|error| error.to_string())?;
    writeln!(
        output,
        "Receipt: request={} run={} plan={} source={} destination={} kernel-events={}",
        receipt.request_id.as_str(),
        receipt.run_id.as_str(),
        receipt.plan_id.as_str(),
        receipt.source_binding_id.as_str(),
        receipt.destination_binding_id.as_str(),
        receipt.kernel_events
    )
    .map_err(|error| error.to_string())
}

fn render_inspect(
    output: &mut impl Write,
    receipt: &CopyRunReceipt,
    arguments: &Arguments,
    form: &conduit_form::CheckedForm,
    plan: &conduit_core::Plan,
) -> Result<(), String> {
    if form.checked_form_id != plan.checked_form_id || plan.plan_id != receipt.plan_id {
        return Err("Inspect retained a mismatched Form and Plan".to_string());
    }
    writeln!(output, "Inspect (after the task)").map_err(|error| error.to_string())?;
    writeln!(output, "Form source:\n{COPY_FORM_SOURCE}").map_err(|error| error.to_string())?;
    writeln!(output, "Plan: {}", receipt.plan_id.as_str()).map_err(|error| error.to_string())?;
    writeln!(output, "  checked form: {}", plan.checked_form_id.as_str())
        .map_err(|error| error.to_string())?;
    for fragment in &plan.fragments {
        writeln!(output, "  host: {}", fragment.host_id.as_str())
            .map_err(|error| error.to_string())?;
        for placement in &fragment.placements {
            writeln!(
                output,
                "  operation: {} (face: {} inputs, {} outputs)",
                placement.operation_id.as_str(),
                placement.inputs.len(),
                placement.outputs.len()
            )
            .map_err(|error| error.to_string())?;
        }
    }
    writeln!(
        output,
        "  source role -> {} (read, max {} bytes)",
        receipt.source_binding_id.as_str(),
        arguments.maximum_bytes
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        output,
        "  destination role -> {} ({:?}, max {} bytes)",
        receipt.destination_binding_id.as_str(),
        arguments.mode,
        arguments.maximum_bytes
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}
