//! Finite ordered batch meaning, using existing Value reads and closing Flows.
//! References are data. No residence, Host, offer, authority or Play is created.
use super::*;
use std::fmt::Write;

pub struct RecordingBatchRecoverySource {
    expanded: conduit_plot::ExpandedCanonicalPlot,
    references: Vec<c::BoundedResourceRef>,
}
impl RecordingBatchRecoverySource {
    pub fn compile(references: &[c::BoundedResourceRef]) -> Result<Self, String> {
        if references.is_empty() || references.len() > r::MAXIMUM_CATALOG_GENERATIONS {
            return Err("recording batch reference count".into());
        }
        for (index, reference) in references.iter().enumerate() {
            reference
                .validate()
                .map_err(|e| format!("reference: {e:?}"))?;
            if reference.content_profile.as_str() != "text/json-utf8@1"
                || reference.access_class.as_str() != "resource/snapshot@1"
                || reference.extent.items != Some(1)
                || reference.extent.bytes == 0
                || reference.extent.bytes > r::MAXIMUM_CHUNK_BYTES as u64
            {
                return Err("recording snapshot profile".into());
            }
            if references[..index].iter().any(|previous| {
                previous.identity == reference.identity
                    && previous.lifetime.version == reference.lifetime.version
            }) {
                return Err("duplicate recording generation".into());
            }
        }
        let (mut startup, mut profiles) = catalogs()?;
        let kind = c::kind_id("text/json-utf8@1");
        let value =
            c::CheckedValueContract::new(kind.clone(), r::MAXIMUM_CHUNK_BYTES as u32, vec![])
                .map_err(|e| format!("{e:?}"))?;
        let schema = c::StructuredInfoType::leaf(kind).map_err(|e| format!("{e:?}"))?;
        conduit_semantic_catalog::install_flow_concat_finite_kind(
            &value,
            &schema,
            &mut startup,
            &mut profiles,
        )?;
        let mut source = String::from("plot recorder {\n");
        for (index, reference) in references.iter().enumerate() {
            let hex: String = reference
                .encode()
                .map_err(|e| format!("{e:?}"))?
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            writeln!(source, "generation{index}: environment/recording-generation\nsnapshot{index}: resource/read-json-snapshot(\"{hex}\")\nevents{index}: flow/singleton\ngeneration{index}.generation >> snapshot{index}.source\nsnapshot{index}.value >> events{index}.value").expect("String write");
        }
        let mut previous = String::from("events0.flow");
        for index in 1..references.len() {
            writeln!(source, "joined{index}: flow/concat/finite\n{previous} >> joined{index}.left\nevents{index}.flow >> joined{index}.right").expect("String write");
            previous = format!("joined{index}.concatenated");
        }
        writeln!(
            source,
            "history: environment/recover-recording\n{previous} >> history.chunk\n}}"
        )
        .expect("String write");
        let expanded = checked_source(&source, &startup, &profiles)?;
        Ok(Self {
            expanded,
            references: references.to_vec(),
        })
    }
    pub fn expanded(&self) -> &conduit_plot::ExpandedCanonicalPlot {
        &self.expanded
    }
    pub fn references(&self) -> &[c::BoundedResourceRef] {
        &self.references
    }
}
