//! Finite admission of inert model-produced Form source.

use alloc::{format, string::String};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ExpandedCanonicalForm,
    ProfileCatalog, Span, StartupCatalog,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    llm_contract, ModelDerivedResult, ModelResultDisposition, ModelResultInvalidity,
    LLM_COMPOSE_KIND,
};

pub const MAXIMUM_COMPOSITION_INTENT_BYTES: usize = 16_384;
pub const MAXIMUM_COMPOSITION_COMMENTARY_BYTES: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormCompositionRequest {
    pub request_identity: String,
    pub intent: String,
    pub catalog_basis_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateFormProvenance {
    pub implementation_identity: String,
    pub request_identity: String,
    pub run_identity: String,
    pub catalog_basis_identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateLifecycle {
    AwaitingExplicitValidationPlanAndPlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateForm {
    pub candidate_identity: String,
    pub source: String,
    pub provenance: CandidateFormProvenance,
    pub commentary: Option<String>,
    pub expanded: ExpandedCanonicalForm,
    pub lifecycle: CandidateLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateFormRefusal {
    MissingExactIdentity,
    IntentBoundExceeded,
    CommentaryBoundExceeded,
    RequestIdentityMismatch,
    ModelResult(ModelResultInvalidity),
    ResultNotProduced(ModelResultDisposition),
    InvalidUtf8,
    SourceBoundExceeded,
    InvalidForm {
        code: &'static str,
        message: String,
        span: Option<Span>,
    },
}

/// Admits a model result as an inert candidate after ordinary Form checking and expansion.
///
/// This function has no host, planner, or runtime input and therefore cannot grant authority,
/// reserve resources, create a Plan, or start a Play.
pub fn admit_candidate_form(
    request: &FormCompositionRequest,
    result: ModelDerivedResult,
    commentary: Option<String>,
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
) -> Result<CandidateForm, CandidateFormRefusal> {
    if request.request_identity.is_empty() || request.catalog_basis_identity.is_empty() {
        return Err(CandidateFormRefusal::MissingExactIdentity);
    }
    if request.intent.len() > MAXIMUM_COMPOSITION_INTENT_BYTES {
        return Err(CandidateFormRefusal::IntentBoundExceeded);
    }
    if commentary
        .as_ref()
        .is_some_and(|value| value.len() > MAXIMUM_COMPOSITION_COMMENTARY_BYTES)
    {
        return Err(CandidateFormRefusal::CommentaryBoundExceeded);
    }
    if result.request_identity != request.request_identity {
        return Err(CandidateFormRefusal::RequestIdentityMismatch);
    }
    let contract = llm_contract(LLM_COMPOSE_KIND).expect("llm/compose is in the reviewed catalog");
    result
        .validate(&contract)
        .map_err(CandidateFormRefusal::ModelResult)?;
    if result.disposition != ModelResultDisposition::Produced {
        return Err(CandidateFormRefusal::ResultNotProduced(result.disposition));
    }
    let source =
        String::from_utf8(result.payload.clone()).map_err(|_| CandidateFormRefusal::InvalidUtf8)?;
    if source.len() > conduit_form::MAXIMUM_FORM_SOURCE_BYTES {
        return Err(CandidateFormRefusal::SourceBoundExceeded);
    }

    let syntax = parse_syntax_document(&source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(CandidateFormRefusal::InvalidForm {
            code: diagnostic.code,
            message: diagnostic.message.clone(),
            span: Some(diagnostic.span),
        });
    }
    let checked = check_syntax_document(&syntax, startup).map_err(|diagnostic| {
        CandidateFormRefusal::InvalidForm {
            code: diagnostic.code,
            message: diagnostic.message,
            span: Some(diagnostic.span),
        }
    })?;
    let entry = checked
        .forms
        .last()
        .ok_or(CandidateFormRefusal::InvalidForm {
            code: "CND-AI-COMPOSE-001",
            message: "candidate source contains no Form".into(),
            span: None,
        })?;
    let expanded = expand_canonical_form(&checked, &entry.name, profile).map_err(|diagnostic| {
        CandidateFormRefusal::InvalidForm {
            code: diagnostic.code,
            message: diagnostic.message,
            span: None,
        }
    })?;

    let provenance = CandidateFormProvenance {
        implementation_identity: result.implementation_identity,
        request_identity: result.request_identity,
        run_identity: result.run_identity,
        catalog_basis_identity: request.catalog_basis_identity.clone(),
    };
    let mut digest = Sha256::new();
    digest.update(b"conduit-candidate-form-v1\0");
    digest.update(provenance.request_identity.as_bytes());
    digest.update(b"\0");
    digest.update(provenance.run_identity.as_bytes());
    digest.update(b"\0");
    digest.update(source.as_bytes());
    let candidate_identity = format!("candidate-form/{:x}", digest.finalize());

    Ok(CandidateForm {
        candidate_identity,
        source,
        provenance,
        commentary,
        expanded,
        lifecycle: CandidateLifecycle::AwaitingExplicitValidationPlanAndPlay,
    })
}
