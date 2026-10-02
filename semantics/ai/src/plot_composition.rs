//! Finite admission of inert model-produced Plot source.

use alloc::{format, string::String};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, ExpandedCanonicalPlot,
    ProfileCatalog, Span, StartupCatalog,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    llm_contract, CandidateLifecycle, ModelDerivedResult, ModelResultDisposition,
    ModelResultInvalidity, LLM_COMPOSE_KIND,
};

pub const MAXIMUM_COMPOSITION_INTENT_BYTES: usize = 16_384;
pub const MAXIMUM_COMPOSITION_COMMENTARY_BYTES: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlotCompositionRequest {
    pub request_identity: String,
    pub intent: String,
    pub catalog_basis_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidatePlotProvenance {
    pub implementation_identity: String,
    pub request_identity: String,
    pub run_identity: String,
    pub catalog_basis_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatePlot {
    pub candidate_identity: String,
    pub source: String,
    pub provenance: CandidatePlotProvenance,
    pub commentary: Option<String>,
    pub expanded: ExpandedCanonicalPlot,
    pub lifecycle: CandidateLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidatePlotRefusal {
    MissingExactIdentity,
    IntentBoundExceeded,
    CommentaryBoundExceeded,
    RequestIdentityMismatch,
    ModelResult(ModelResultInvalidity),
    ResultNotProduced(ModelResultDisposition),
    InvalidUtf8,
    SourceBoundExceeded,
    InvalidPlot {
        code: &'static str,
        message: String,
        span: Option<Span>,
    },
}

/// Admits a model result as an inert candidate after ordinary plot checking and expansion.
///
/// This function has no host, planner, or runtime input and therefore cannot grant authority,
/// reserve resources, create a plan, or start a play.
pub fn admit_candidate_plot(
    request: &PlotCompositionRequest,
    result: ModelDerivedResult,
    commentary: Option<String>,
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
) -> Result<CandidatePlot, CandidatePlotRefusal> {
    if request.request_identity.is_empty() || request.catalog_basis_identity.is_empty() {
        return Err(CandidatePlotRefusal::MissingExactIdentity);
    }
    if request.intent.len() > MAXIMUM_COMPOSITION_INTENT_BYTES {
        return Err(CandidatePlotRefusal::IntentBoundExceeded);
    }
    if commentary
        .as_ref()
        .is_some_and(|value| value.len() > MAXIMUM_COMPOSITION_COMMENTARY_BYTES)
    {
        return Err(CandidatePlotRefusal::CommentaryBoundExceeded);
    }
    if result.request_identity != request.request_identity {
        return Err(CandidatePlotRefusal::RequestIdentityMismatch);
    }
    let contract = llm_contract(LLM_COMPOSE_KIND).expect("llm/compose is in the reviewed catalog");
    result
        .validate(&contract)
        .map_err(CandidatePlotRefusal::ModelResult)?;
    if result.disposition != ModelResultDisposition::Produced {
        return Err(CandidatePlotRefusal::ResultNotProduced(result.disposition));
    }
    let source =
        String::from_utf8(result.payload.clone()).map_err(|_| CandidatePlotRefusal::InvalidUtf8)?;
    if source.len() > conduit_plot::MAXIMUM_PLOT_SOURCE_BYTES {
        return Err(CandidatePlotRefusal::SourceBoundExceeded);
    }

    let syntax = parse_syntax_document(&source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(CandidatePlotRefusal::InvalidPlot {
            code: diagnostic.code,
            message: diagnostic.message.clone(),
            span: Some(diagnostic.span),
        });
    }
    let checked = check_syntax_document(&syntax, startup).map_err(|diagnostic| {
        CandidatePlotRefusal::InvalidPlot {
            code: diagnostic.code,
            message: diagnostic.message,
            span: Some(diagnostic.span),
        }
    })?;
    let entry = checked
        .plots
        .last()
        .ok_or(CandidatePlotRefusal::InvalidPlot {
            code: "CND-AI-COMPOSE-001",
            message: "candidate source contains no Plot".into(),
            span: None,
        })?;
    let expanded = expand_canonical_plot(&checked, &entry.name, profile).map_err(|diagnostic| {
        CandidatePlotRefusal::InvalidPlot {
            code: diagnostic.code,
            message: diagnostic.message,
            span: None,
        }
    })?;

    let provenance = CandidatePlotProvenance {
        implementation_identity: result.implementation_identity,
        request_identity: result.request_identity,
        run_identity: result.run_identity,
        catalog_basis_identity: request.catalog_basis_identity.clone(),
    };
    let mut digest = Sha256::new();
    digest.update(b"conduit-candidate-plot-v1\0");
    digest.update(provenance.request_identity.as_bytes());
    digest.update(b"\0");
    digest.update(provenance.run_identity.as_bytes());
    digest.update(b"\0");
    digest.update(source.as_bytes());
    let candidate_identity = format!("candidate-plot/{:x}", digest.finalize());

    Ok(CandidatePlot {
        candidate_identity,
        source,
        provenance,
        commentary,
        expanded,
        lifecycle: CandidateLifecycle::AwaitingExplicitValidationPlanAndPlay,
    })
}
