use serde::{Deserialize, Serialize};
use tongues_pipeline::{fixture_catalog, starter_graph, GraphDocument, StarterGraph};

pub const TONGUES_REVISION: &str = "5748f20ee4fd133be6a9332b01d96dc0649b26a3";
pub const TONGUES_STARTER_PATH: &str = "crates/tongues-pipeline/src/starter.rs";
pub const TONGUES_STARTER_ID: &str = "text_to_speech";
pub const SPECIMEN_TEXT: &str = "Hello from Tongues.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TonguesSpecimenIdentity {
    pub repository: String,
    pub revision: String,
    pub source_path: String,
    pub starter_id: String,
    pub graph_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TonguesTextToSpeechSpecimen {
    pub identity: TonguesSpecimenIdentity,
    pub graph: GraphDocument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecimenError {
    StarterUnavailable(String),
    GraphIdentityMismatch,
    NodeContractMismatch,
    EdgeContractMismatch,
}

pub fn load_text_to_speech_specimen() -> Result<TonguesTextToSpeechSpecimen, SpecimenError> {
    let graph = starter_graph(StarterGraph::TextToSpeech, &fixture_catalog())
        .map_err(|error| SpecimenError::StarterUnavailable(error.to_string()))?;
    validate_graph(&graph)?;
    Ok(TonguesTextToSpeechSpecimen {
        identity: TonguesSpecimenIdentity {
            repository: "https://github.com/dancxjo/tongues".into(),
            revision: TONGUES_REVISION.into(),
            source_path: TONGUES_STARTER_PATH.into(),
            starter_id: TONGUES_STARTER_ID.into(),
            graph_id: graph.graph_id.clone(),
        },
        graph,
    })
}

fn validate_graph(graph: &GraphDocument) -> Result<(), SpecimenError> {
    if graph.graph_id != "starter:text_to_speech" {
        return Err(SpecimenError::GraphIdentityMismatch);
    }
    let nodes = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.kind.as_str()))
        .collect::<Vec<_>>();
    if nodes
        != [
            ("text", "text_source"),
            ("tts", "tts"),
            ("audio", "audio_output"),
        ]
    {
        return Err(SpecimenError::NodeContractMismatch);
    }
    let edges = graph
        .edges
        .iter()
        .map(|edge| {
            (
                edge.from.node_id.as_str(),
                edge.from.port_id.as_str(),
                edge.to.node_id.as_str(),
                edge.to.port_id.as_str(),
            )
        })
        .collect::<Vec<_>>();
    if edges != [("text", "out", "tts", "in"), ("tts", "out", "audio", "in")] {
        return Err(SpecimenError::EdgeContractMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_real_starter_retains_original_tongues_terms() {
        let specimen = load_text_to_speech_specimen().expect("pinned starter loads");
        assert_eq!(specimen.identity.revision, TONGUES_REVISION);
        assert_eq!(specimen.identity.source_path, TONGUES_STARTER_PATH);
        assert_eq!(specimen.graph.metadata.name, "Text to speech");
        assert_eq!(specimen.graph.selected_sinks[0].node_id, "audio");
        assert_eq!(specimen.graph.selected_sinks[0].port_id, "in");
    }
}
