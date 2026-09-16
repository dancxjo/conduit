//! Shared checked House conversation topology and exact Cord capacities.

use conduit_core::{GearId, PortId};
use conduit_form::{
    check_syntax_document, parse_syntax_document, ExpandedCanonicalForm, ProfileCatalog,
    StartupCatalog,
};
use std::collections::BTreeMap;

pub(crate) type HouseConnectionEndpoints = (GearId, PortId, GearId, PortId);

pub(crate) struct HouseConversationTopology {
    pub entry: &'static str,
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
    startup.insert_value_kind_alias(
        "PcmClip",
        conduit_core::kind_id(conduit_audio::AUDIO_PCM_CLIP_INFO_ID),
    )?;
    crate::installed_std::test_local_model_io::install_house_source_catalog(
        &mut startup,
        &mut profiles,
    );
    crate::installed_std::test_local_model_io::install_house_text_sink_catalog(
        &mut startup,
        &mut profiles,
    );
    let source = concat!(
        include_str!("../../../forms/addressed-utterance/main.conduit"),
        "\n",
        include_str!("../../../forms/house-conversation/main.conduit"),
        "\n",
        include_str!("../proof/recorded-house/main.conduit"),
    );
    let checked =
        check_syntax_document(&parse_syntax_document(source), &startup).map_err(|error| {
            format!(
                "recorded House Form check: {} {} at {}:{}",
                error.code, error.message, error.span.line, error.span.column
            )
        })?;
    let entry = match (microphone_source, spoken_output) {
        (false, false) => "recorded-house-proof",
        (false, true) => "recorded-house-spoken-proof",
        (true, false) => "microphone-house-proof",
        (true, true) => "microphone-house-spoken-proof",
    };
    let expanded = conduit_form::expand_canonical_form(&checked, entry, &profiles)
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
        entry,
        expanded,
        connection_limits,
    })
}
