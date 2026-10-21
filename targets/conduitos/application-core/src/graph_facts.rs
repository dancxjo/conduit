//! Bounded readonly graph contribution; the outer Face and all actions stay Root-owned.
use crate::data::{self, Refusal};
use alloc::{format, vec::Vec};
use conduit_face::{FaceDisclosure, FaceProperty, FaceRelationship, FaceSubject, FaceText};
use patchbay_graph::PatchbayGraph;
use serde::{Deserialize, Serialize};
mod projection;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub host: conduit_core::HostId,
    pub boot: conduit_core::BootId,
    pub body: conduit_body::BodyId,
    pub owner_revision: u64,
    pub graph_digest: [u8; 32],
    pub plot_present: bool,
}
impl Scope {
    pub fn new(
        host: conduit_core::HostId,
        boot: conduit_core::BootId,
        body: conduit_body::BodyId,
        owner_revision: u64,
        graph: &PatchbayGraph,
        plot_present: bool,
    ) -> Result<Self, Refusal> {
        let scope = Self {
            host,
            boot,
            body,
            owner_revision,
            graph_digest: digest(graph)?,
            plot_present,
        };
        scope.validate(graph)?;
        Ok(scope)
    }
    pub fn validate(&self, graph: &PatchbayGraph) -> Result<(), Refusal> {
        if self.owner_revision == 0 || self.graph_digest != digest(graph)? {
            return Err(Refusal::Bounds);
        }
        for identity in [self.host.as_str(), self.boot.as_str(), self.body.as_str()] {
            if identity.is_empty()
                || identity.len() > 2048
                || identity.chars().any(char::is_control)
            {
                return Err(Refusal::Bounds);
            }
        }
        Ok(())
    }
}
/// Deliberately cannot contain actions, outer basis, interaction context or Sign claims.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    pub scope: Scope,
    pub subjects: Vec<FaceSubject>,
    pub relationships: Vec<FaceRelationship>,
    pub properties: Vec<FaceProperty>,
    pub text: Vec<FaceText>,
    pub disclosures: Vec<FaceDisclosure>,
}
pub fn digest(graph: &PatchbayGraph) -> Result<[u8; 32], Refusal> {
    Ok(conduit_core::semantic_digest(
        "conduitos/graph-face-facts@1",
        &data::encode(graph)?,
    ))
}
pub fn project(graph: &PatchbayGraph, scope: Scope) -> Result<Contribution, Refusal> {
    scope.validate(graph)?;
    let body = format!("body/{}", scope.body.as_str());
    let mut result = Contribution {
        scope,
        subjects: Vec::new(),
        relationships: Vec::new(),
        properties: Vec::new(),
        text: Vec::new(),
        disclosures: Vec::new(),
    };
    projection::append(graph, &body, &mut result);
    Ok(result)
}
