//! Shared checked House conversation topology and exact Cord capacities.

use conduit_core::{GearId, PortId};
use conduit_form::{
    check_syntax_document, parse_syntax_document, ExpandedCanonicalForm, ProfileCatalog,
    StartupCatalog,
};
use std::collections::BTreeMap;

pub(crate) type HouseConnectionEndpoints = (GearId, PortId, GearId, PortId);

pub(crate) struct HouseConversationTopology {
    pub expanded: ExpandedCanonicalForm,
    pub connection_limits:
        BTreeMap<HouseConnectionEndpoints, conduit_planner::ConnectionQueueLimits>,
}

pub(crate) fn build(
    microphone_source: bool,
    spoken_output: bool,
) -> Result<HouseConversationTopology, Box<dyn std::error::Error>> {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_text::install_text_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_microphone_clip_catalogs(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles)?;
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)?;
    conduit_ai::install_model_text_catalog(&mut startup, &mut profiles)?;
    conduit_tongues::install_house_conversation_catalog(&mut startup, &mut profiles)?;
    crate::installed_std::test_local_model_io::install_house_source_catalog(
        &mut startup,
        &mut profiles,
    );
    crate::installed_std::test_local_model_io::install_house_text_sink_catalog(
        &mut startup,
        &mut profiles,
    );
    let (audio_kind, audio_wiring) = if microphone_source {
        (
            conduit_semantic_catalog::MICROPHONE_CLIP_SOURCE_KIND,
            "\"capture\" > audio.request\n audio.clip > recognize.clip",
        )
    } else {
        (
            crate::installed_std::test_local_model_io::HOUSE_AUDIO_CLIP_SOURCE_KIND,
            "audio.value > recognize.clip",
        )
    };
    let (output_declarations, output_wiring) = if spoken_output {
        (
            format!(
                "synthesize: speech/synthesize(maximum-output-bytes = {})\n convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = \"stereo-left-right\")\n output: audio/play",
                conduit_tongues::MAXIMUM_PCM_BYTES
            ),
            "house.response > synthesize.text\n synthesize.audio > convert.audio\n convert.converted > output.audio",
        )
    } else {
        (
            format!(
                "sink: {}",
                crate::installed_std::test_local_model_io::HOUSE_TEXT_SINK_KIND
            ),
            "house.response > sink.value",
        )
    };
    let source = format!(
        "{}\n{}\nform recorded-house-proof {{\n audio: {}\n recognize: speech/recognize-clip\n recognized: speech/recognition-to-text\n addresses: {}\n addressed: addressed-utterance\n context: {}\n house: house-conversation\n {}\n {}\n recognize.result > recognized.result\n recognized.text > addressed.recognized\n addresses.value > addressed.addresses\n addressed.detection > house.detection\n context.value > house.context\n {}\n}}\n",
        include_str!("../../../forms/addressed-utterance/main.conduit"),
        include_str!("../../../forms/house-conversation/main.conduit"),
        audio_kind,
        crate::installed_std::test_local_model_io::HOUSE_ADDRESSES_SOURCE_KIND,
        crate::installed_std::test_local_model_io::HOUSE_CONTEXT_SOURCE_KIND,
        output_declarations,
        audio_wiring,
        output_wiring,
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &startup).map_err(|error| {
            format!(
                "recorded House Form check: {} {}",
                error.code, error.message
            )
        })?;
    let expanded = conduit_form::expand_canonical_form(&checked, "recorded-house-proof", &profiles)
        .map_err(|error| format!("recorded House expansion: {} {}", error.code, error.message))?;
    let mut connection_limits = BTreeMap::new();
    for connection in &expanded.connections {
        let byte_capacity = if connection.source_gear_id.as_str().ends_with("/audio")
            && connection.sink_gear_id.as_str().ends_with("/recognize")
        {
            Some(conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32)
        } else if microphone_source
            && connection.sink_gear_id.as_str().ends_with("/audio")
            && connection.sink_port_id.as_str() == "request"
        {
            Some(16)
        } else if spoken_output
            && connection.source_gear_id.as_str().ends_with("/synthesize")
            && connection.source_port_id.as_str() == "audio"
        {
            Some(conduit_tongues::MAXIMUM_PCM_BYTES)
        } else if spoken_output
            && connection.source_gear_id.as_str().ends_with("/convert")
            && connection.source_port_id.as_str() == "converted"
        {
            Some(conduit_std_offers::AUDIO_CONVERT_PCM_MAXIMUM_OUTPUT_BYTES)
        } else {
            None
        };
        if let Some(byte_capacity) = byte_capacity {
            connection_limits.insert(
                (
                    connection.source_gear_id.clone(),
                    connection.source_port_id.clone(),
                    connection.sink_gear_id.clone(),
                    connection.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            );
        }
    }
    Ok(HouseConversationTopology {
        expanded,
        connection_limits,
    })
}
