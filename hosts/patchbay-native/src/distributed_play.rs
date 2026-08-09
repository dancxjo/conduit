//! Asynchronous native source half of one real distributed Signal Play.

use conduit_core::{BootId, HostId};
use conduit_std_host::distributed_signal::{bind_listener, DistributedSource};
use std::sync::mpsc::{self, Receiver, TryRecvError};

pub struct NativeDistributedPlay {
    lines: Vec<String>,
    receiver: Option<Receiver<Result<String, String>>>,
}

impl NativeDistributedPlay {
    pub fn start(host_id: HostId, boot_id: BootId) -> Result<Self, String> {
        let source = DistributedSource::prepare_for_source(host_id, boot_id)?;
        let binding = source.binding().clone();
        let listener = bind_listener()?;
        let url = listener.url().map_err(|error| format!("{error:?}"))?;
        let lines = vec![
            format!(
                "DISTRIBUTED PLAY status=awaiting-peer plan={}",
                binding.plan_id.as_str()
            ),
            format!(
                "  SOURCE host={} boot={} active-play={}",
                binding.source.host_id.as_str(),
                binding.source.boot_id.as_str(),
                binding.source_active_play_id.as_str()
            ),
            format!(
                "  PEER host={} boot={} active-play={}",
                binding.sink.host_id.as_str(),
                binding.sink.boot_id.as_str(),
                binding.sink_active_play_id.as_str()
            ),
            format!("  ATTACH url={url}"),
        ];
        println!(
            "patchbay distributed url={url} source-host={} source-boot={} plan={}",
            binding.source.host_id.as_str(),
            binding.source.boot_id.as_str(),
            binding.plan_id.as_str()
        );
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut report = Vec::with_capacity(1024);
            let result = source
                .run(listener, &mut report)
                .and_then(|()| String::from_utf8(report).map_err(|error| error.to_string()));
            let _ = sender.send(result);
        });
        Ok(Self {
            lines,
            receiver: Some(receiver),
        })
    }

    pub fn poll(&mut self) -> Result<bool, String> {
        let Some(receiver) = &self.receiver else {
            return Ok(false);
        };
        match receiver.try_recv() {
            Ok(Ok(report)) => {
                let summary = report
                    .lines()
                    .find(|line| line.starts_with("summary "))
                    .ok_or("distributed execution omitted its terminal summary")?;
                for required in [
                    "source_terminal=completed",
                    "browser_terminal=completed",
                    "capacity_stable=true",
                    "retained=0",
                    "in_flight=0",
                ] {
                    if !summary.contains(required) {
                        return Err(format!("distributed summary omitted {required}"));
                    }
                }
                println!("{summary}");
                self.lines[0] = self.lines[0].replace("awaiting-peer", "completed");
                self.lines.push(format!("  EXECUTION {summary}"));
                self.lines.push(
                    "  KERNEL-CLUE RemoteValueDelivered OperationCompleted pressure-retry=1".into(),
                );
                self.receiver = None;
                Ok(true)
            }
            Ok(Err(error)) => {
                self.receiver = None;
                Err(error)
            }
            Err(TryRecvError::Empty) => Ok(false),
            Err(TryRecvError::Disconnected) => {
                self.receiver = None;
                Err("distributed execution worker disconnected".into())
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.receiver.is_some()
    }

    pub fn is_complete(&self) -> bool {
        !self.is_running() && self.lines[0].contains("status=completed")
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }
}

pub fn run_server() -> Result<(), String> {
    let model = patchbay_model::PatchbayModel::fresh();
    let source = DistributedSource::prepare_for_source(
        model.projection().host_id().clone(),
        model.projection().boot_id().clone(),
    )?;
    let binding = source.binding().clone();
    let listener = bind_listener()?;
    let url = listener.url().map_err(|error| format!("{error:?}"))?;
    println!(
        "{url} source_host={} source_boot={} plan={}",
        binding.source.host_id.as_str(),
        binding.source.boot_id.as_str(),
        binding.plan_id.as_str()
    );
    use std::io::Write;
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    source.run(listener, &mut std::io::stdout())
}
