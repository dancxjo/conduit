//! Read-only local selection hints. Selection and admission still happen at
//! each Play against the installed Host's current Boot and ALSA observation.

use std::{fs, io::Write, path::PathBuf};

use conduit_std_host::{
    hosted_audio::discover_alsa_playback, hosted_speech_synthesis::EspeakDiscovery,
};
use serde_json::{json, Value};

const MAXIMUM_SPEAKERS: usize = 64;
const MAXIMUM_PROVIDERS: usize = 8;

pub(crate) fn run(json_output: bool, output: &mut impl Write) -> Result<(), String> {
    let speakers =
        discover_alsa_playback().map_err(|error| format!("discover local speakers: {error}"))?;
    if speakers.len() > MAXIMUM_SPEAKERS {
        return Err("local speaker inventory exceeds 64 observations".into());
    }
    if speakers.iter().any(|speaker| {
        speaker.card_id.len() > 128
            || speaker.card_name.len() > 256
            || speaker.device_name.len() > 256
            || speaker.base_identity.len() > 256
    }) {
        return Err("local speaker observation exceeds text bounds".into());
    }
    let providers = discover_common_espeak_providers();
    if json_output {
        let speakers = speakers
            .iter()
            .map(|speaker| {
                json!({
                    "card_id": speaker.card_id,
                    "card_index": speaker.card_index,
                    "card_name": speaker.card_name,
                    "device": speaker.device,
                    "device_name": speaker.device_name,
                    "base_identity": speaker.base_identity,
                })
            })
            .collect::<Vec<_>>();
        let inventory = json!({
            "schema": "conduit.body/local-speech-options@1",
            "speakers": speakers,
            "providers": providers,
            "effect_performed": false,
        })
        .to_string();
        if inventory.len() > 65_536 {
            return Err("local speech-options JSON exceeds 64 KiB".into());
        }
        writeln!(output, "{inventory}").map_err(|error| error.to_string())?;
    } else {
        writeln!(
            output,
            "Current local speaker observations (no device opened):"
        )
        .map_err(|error| error.to_string())?;
        for speaker in &speakers {
            writeln!(
                output,
                "  --speaker-card {} --speaker-device {}  {}",
                speaker.card_id, speaker.device, speaker.device_name
            )
            .map_err(|error| error.to_string())?;
        }
        if speakers.is_empty() {
            writeln!(output, "  No ALSA playback device observed.")
                .map_err(|error| error.to_string())?;
        }
        writeln!(output, "Verified local eSpeak NG providers:")
            .map_err(|error| error.to_string())?;
        for provider in &providers {
            writeln!(
                output,
                "  --speech-executable {} --speech-data {} --speech-engine {} --speech-voice {}\n    provider SHA-256 {}",
                provider["executable"].as_str().unwrap_or(""),
                provider["data"].as_str().unwrap_or(""),
                provider["engine"].as_str().unwrap_or(""),
                provider["voice"].as_str().unwrap_or(""),
                provider["provider_sha256"].as_str().unwrap_or("")
            )
            .map_err(|error| error.to_string())?;
        }
        if providers.is_empty() {
            writeln!(
                output,
                "  No verified provider found in common system locations."
            )
            .map_err(|error| error.to_string())?;
        }
        writeln!(output, "Copy one exact speaker and provider into `conduit body birth --screen-free --speak`; current availability is rechecked at Play.")
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn discover_common_espeak_providers() -> Vec<Value> {
    let executables = ["/usr/bin/espeak-ng", "/usr/local/bin/espeak-ng"];
    let mut libraries = vec![
        PathBuf::from("/usr/lib"),
        PathBuf::from("/usr/lib64"),
        PathBuf::from("/usr/local/lib"),
    ];
    if let Ok(entries) = fs::read_dir("/usr/lib") {
        for entry in entries.take(512).flatten() {
            if entry.file_name().to_string_lossy().ends_with("-linux-gnu") {
                libraries.push(entry.path());
            }
        }
    }
    libraries.sort();
    libraries.dedup();
    let mut providers = Vec::new();
    for library in libraries {
        let Ok(engine) = fs::canonicalize(library.join("libespeak-ng.so.1")) else {
            continue;
        };
        let data_roots = [
            library.join("espeak-ng-data"),
            PathBuf::from("/usr/share/espeak-ng-data"),
            PathBuf::from("/usr/local/share/espeak-ng-data"),
        ];
        for executable in executables {
            for data in &data_roots {
                let Ok(discovery) = EspeakDiscovery::inspect(
                    &PathBuf::from(executable),
                    data,
                    "en-us",
                    std::slice::from_ref(&engine),
                ) else {
                    continue;
                };
                providers.push(json!({
                    "executable": executable,
                    "data": data,
                    "engine": engine,
                    "voice": "en-us",
                    "provider_sha256": discovery.provider_sha256,
                }));
                if providers.len() == MAXIMUM_PROVIDERS {
                    return providers;
                }
            }
        }
    }
    providers
}
