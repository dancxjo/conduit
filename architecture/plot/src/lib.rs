#![no_std]

#[macro_use]
extern crate alloc;
#[cfg(test)]
extern crate std;

mod prelude {
    pub use alloc::boxed::Box;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec::Vec;
}

use crate::prelude::*;
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    CapabilityId, CheckedPlotId, ConfigurationEntry, ConfigurationValue, ExpandedPlotId, GearId,
    KindId, KindIdentity, PlotIdentity, PortDescriptor, PortDirection, PortId, SourceDocumentId,
};
use sha2::{Digest, Sha256};

mod back_catalog;
mod canonical_expansion;
mod checked_syntax;
mod diagnostic;
mod ecmascript_binding;
mod expression_check;
mod expression_definition;
mod expression_evaluate;
mod expression_numeric_type;
mod expression_f32;
mod expression_prepared;
mod expression_program;
mod expression_program_decode;
mod expression_proof;
mod expression_semantic_call;
mod functional_front;
mod integer_literal;
mod native_type;
mod package_bundle;
#[cfg(test)]
mod package_bundle_tests;
mod package_check;
#[cfg(test)]
mod package_check_tests;
mod package_resolution;
#[cfg(test)]
mod package_resolution_tests;
mod pure_expression;
pub mod rust_binding;
mod structured_expression;
mod structured_selector;
mod structured_startup;
mod surface_lex;
mod surface_parser;
pub mod syntax;
mod syntax_check;
mod syntax_highlight;
mod syntax_identity;
mod text_value;
pub use text_value::text_startup_literal;
mod type_form;
mod value_pattern;
mod value_pattern_lookahead;
mod value_pattern_source;
#[cfg(test)]
mod value_pattern_tests;
mod value_type;
mod variadic_front;

pub use back_catalog::*;
pub use canonical_expansion::*;
pub use checked_syntax::*;
pub use conduit_core::{KindConfigurationField, KindConfigurationRule};
pub use diagnostic::*;
pub use ecmascript_binding::*;
pub use expression_check::*;
pub use expression_definition::*;
pub use expression_evaluate::*;
pub use expression_prepared::*;
pub use expression_program::*;
pub use package_bundle::*;
pub use package_check::*;
pub use package_resolution::*;
pub use structured_startup::*;
pub use syntax::*;
pub use syntax_highlight::*;
pub use value_pattern::*;
pub use value_pattern_source::*;
pub use variadic_front::*;

pub const MAXIMUM_PLOT_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAXIMUM_PLOT_TOKENS: usize = 131_072;
pub const MAXIMUM_USE_DECLARATIONS: usize = 256;
pub const MAXIMUM_PLOT_NESTING_DEPTH: usize = 16;
pub const MAXIMUM_PACKAGE_EXPORTS: usize = 256;
pub const MAXIMUM_PACKAGE_REQUIREMENTS: usize = 256;
pub const MAXIMUM_PACKAGE_MEMBERS: usize = 256;
pub const MAXIMUM_PACKAGE_CONTENT_BYTES: usize = 16 * 1024 * 1024;

/// Exact UTF-8 byte extent plus one-based source locations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CstTokenKind {
    Whitespace,
    Comment,
    Lexeme,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CstToken {
    pub kind: CstTokenKind,
    pub span: Span,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotDiagnostic {
    pub code: &'static str,
    pub span: Span,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedGear {
    pub gear_id: GearId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub startup_parameters: Vec<conduit_core::FrontStartupParameter>,
    pub shorthand: Option<(PortId, PortId)>,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub semantic_contract: conduit_core::KindSemanticContract,
    pub terminal_transductions: Vec<conduit_core::TerminalTransductionProfile>,
    pub resource_ports: Vec<conduit_core::ResourcePortContract>,
    pub configuration: Vec<ConfigurationEntry>,
    pub pool_references: Vec<conduit_core::SharedPoolId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedConnection {
    pub source_gear_id: GearId,
    pub source_port_id: PortId,
    pub sink_gear_id: GearId,
    pub sink_port_id: PortId,
    pub value_kind: KindId,
    pub track: conduit_core::ConnectionTrack,
    pub temporal: conduit_core::PortTemporal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPlot {
    pub source_document_id: SourceDocumentId,
    pub checked_plot_id: CheckedPlotId,
    pub expanded_plot_id: ExpandedPlotId,
    pub name: String,
    pub completion: PlotCompletionPolicy,
    pub gears: Vec<CheckedGear>,
    pub connections: Vec<CheckedConnection>,
    pub exports: Vec<CheckedExport>,
    pub nested_plots: Vec<CheckedNestedPlot>,
}

impl CheckedPlot {
    pub fn identity(&self) -> PlotIdentity {
        PlotIdentity {
            source_document_id: self.source_document_id.clone(),
            checked_plot_id: self.checked_plot_id.clone(),
            expanded_plot_id: self.expanded_plot_id.clone(),
        }
    }

    /// Recomputes the checked and recursively expanded identities from the
    /// checked structure. This is the drawbridge between mutable hosted data
    /// and planning: callers may not substitute or omit nested expansion rows
    /// while retaining a previously sealed identity.
    pub fn validate_identities(&self) -> Result<(), PlotError> {
        for pair in self.nested_plots.windows(2) {
            if pair[0].gear_id >= pair[1].gear_id {
                return Err(PlotError::InvalidIdentity(
                    "nested expansion rows are not unique canonical paths".into(),
                ));
            }
        }
        for nested in &self.nested_plots {
            nested.plot.validate_identities()?;
            let gear = self
                .gears
                .iter()
                .find(|gear| gear.gear_id == nested.gear_id)
                .ok_or_else(|| {
                    PlotError::InvalidIdentity(format!(
                        "nested expansion path '{}' has no checked gear",
                        nested.gear_id.as_str()
                    ))
                })?;
            let boundary = nested
                .plot
                .export_boundary_unvalidated(&nested.export_capability_id)?;
            let definition = boundary.kind_projection();
            if gear.kind_id != definition.kind_id
                || gear.kind_contract_revision != definition.kind_contract_revision
                || gear.inputs != definition.inputs
                || gear.outputs != definition.outputs
                || !gear.configuration.is_empty()
            {
                return Err(PlotError::InvalidIdentity(format!(
                    "nested expansion path '{}' differs from its selected export",
                    nested.gear_id.as_str()
                )));
            }
        }

        let expected_checked = checked_plot_id(
            &self.name,
            self.completion,
            &self.gears,
            &self.connections,
            &self.exports,
        );
        if self.checked_plot_id != expected_checked {
            return Err(PlotError::InvalidIdentity(
                "checked plot identity differs from its canonical semantic plot".into(),
            ));
        }
        let expected_expanded = expanded_plot_id(&expected_checked, &self.nested_plots);
        if self.expanded_plot_id != expected_expanded {
            return Err(PlotError::InvalidIdentity(
                "expanded plot identity omits or substitutes a nested expansion".into(),
            ));
        }
        Ok(())
    }

    /// Derives the only composite boundary contract this plot may expose for
    /// `capability_id`. Every field comes from a checked authored export and
    /// its checked endpoint descriptors.
    pub fn export_boundary(
        &self,
        capability_id: &CapabilityId,
    ) -> Result<CheckedCompositeBoundary, PlotError> {
        self.validate_identities()?;
        self.export_boundary_unvalidated(capability_id)
    }

    fn export_boundary_unvalidated(
        &self,
        capability_id: &CapabilityId,
    ) -> Result<CheckedCompositeBoundary, PlotError> {
        let mut export = self
            .exports
            .iter()
            .find(|export| &export.capability_id == capability_id)
            .cloned()
            .or_else(|| (self.exports.len() == 1).then(|| self.exports[0].clone()))
            .ok_or_else(|| {
                PlotError::InvalidExport(format!(
                    "checked plot has no authored capability '{}'",
                    capability_id.as_str()
                ))
            })?;
        export.capability_id = capability_id.clone();
        validate_export_fronts(&export, &self.gears)?;
        let inputs = export
            .input_fronts
            .iter()
            .map(|front| front.external_port.clone())
            .collect::<Vec<_>>();
        let outputs = export
            .output_fronts
            .iter()
            .map(|front| front.external_port.clone())
            .collect::<Vec<_>>();
        Ok(CheckedCompositeBoundary {
            capability_id: export.capability_id.clone(),
            kind_id: export.kind_id.clone(),
            kind_contract_revision: exported_contract_revision(
                &export.kind_id,
                &export.input_fronts,
                &export.output_fronts,
            ),
            inputs,
            outputs,
            input_fronts: export.input_fronts.clone(),
            output_fronts: export.output_fronts.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedNestedPlot {
    pub gear_id: GearId,
    pub export_capability_id: CapabilityId,
    pub plot: CheckedPlot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedExport {
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub input_fronts: Vec<CheckedCompositeFront>,
    pub output_fronts: Vec<CheckedCompositeFront>,
}

/// Terminal behavior is part of the exported front contract, independently for
/// every front. More policies can be added without weakening the current exact
/// `independent` contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeFrontTerminal {
    Independent,
    /// Reserved invalid value used to prove hosted mutation rejection. The
    /// authored grammar intentionally accepts only `independent` today.
    Coupled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCompositeFront {
    pub external_port: PortDescriptor,
    pub internal_gear_id: GearId,
    pub internal_port_id: PortId,
    pub track: conduit_core::ConnectionTrack,
    pub terminal: CompositeFrontTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCompositeBoundary {
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub input_fronts: Vec<CheckedCompositeFront>,
    pub output_fronts: Vec<CheckedCompositeFront>,
}

impl CheckedCompositeBoundary {
    pub fn kind_projection(&self) -> KindProjection {
        KindProjection {
            kind_id: self.kind_id.clone(),
            kind_contract_revision: self.kind_contract_revision.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            configuration: Default::default(),
        }
    }
}

/// Checker projection of a canonical [`conduit_core::Kind`].
///
/// This contains only the semantic fields needed while checking authored Plot
/// source. It is not a second Kind identity and cannot be offered by a Host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindProjection {
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<KindConfigurationField>,
}

impl From<&conduit_core::Kind> for KindProjection {
    fn from(kind: &conduit_core::Kind) -> Self {
        Self {
            kind_id: kind.kind_id.clone(),
            kind_contract_revision: kind.kind_contract_revision.clone(),
            inputs: kind.inputs.clone(),
            outputs: kind.outputs.clone(),
            configuration: kind.configuration.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileCatalog {
    kinds: BTreeMap<KindId, KindProjection>,
    canonical_kinds: BTreeMap<KindId, conduit_core::Kind>,
    variadic_fores: BTreeMap<KindId, HomogeneousVariadicFore>,
    variadic_kinds: BTreeMap<KindId, conduit_core::Kind>,
    type_invariants: BTreeMap<KindId, Vec<PortableExpressionProgram>>,
}

impl ProfileCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn install_type_invariants(&mut self, native_types: &[CheckedNativeType]) {
        for native_type in native_types {
            if !native_type.invariants.is_empty() {
                self.type_invariants.insert(
                    native_type
                        .value_type
                        .profile()
                        .expect("checked native Type has a profile")
                        .value_kind()
                        .clone(),
                    native_type.invariants.clone(),
                );
            }
        }
    }

    pub(crate) fn type_invariants(
        &self,
        value_kind: &KindId,
    ) -> Option<&[PortableExpressionProgram]> {
        self.type_invariants.get(value_kind).map(Vec::as_slice)
    }

    pub fn insert(&mut self, definition: KindProjection) -> Result<(), PlotError> {
        validate_unique_port_symbols(&definition)?;
        if self.kinds.contains_key(&definition.kind_id) {
            return Err(PlotError::DuplicateKind(
                definition.kind_id.as_str().to_string(),
            ));
        }
        self.kinds.insert(definition.kind_id.clone(), definition);
        Ok(())
    }

    /// Installs a reviewed finite family which specializes one homogeneous
    /// input prototype into exact ordinary ports at each use site.
    pub fn insert_homogeneous_variadic(
        &mut self,
        definition: KindProjection,
        minimum_inputs: u16,
        maximum_inputs: u16,
    ) -> Result<(), PlotError> {
        let ([prototype], [output]) = (definition.inputs.as_slice(), definition.outputs.as_slice())
        else {
            return Err(PlotError::InvalidKind(
                "a homogeneous variadic projection requires one input prototype and one output"
                    .into(),
            ));
        };
        let family = HomogeneousVariadicFore::new(
            prototype.clone(),
            output.clone(),
            minimum_inputs,
            maximum_inputs,
        )
        .map_err(PlotError::InvalidKind)?;
        let kind_id = definition.kind_id.clone();
        self.insert(definition)?;
        self.variadic_fores.insert(kind_id, family);
        Ok(())
    }

    /// Installs complete semantic Kind truth for a reviewed homogeneous
    /// variadic family. The template itself is never exposed as an exact Kind.
    pub fn insert_homogeneous_variadic_kind(
        &mut self,
        kind: conduit_core::Kind,
        minimum_inputs: u16,
        maximum_inputs: u16,
    ) -> Result<(), PlotError> {
        kind.validate()
            .map_err(|error| PlotError::InvalidKind(format!("{error:?}")))?;
        let kind_id = kind.kind_id.clone();
        self.insert_homogeneous_variadic(
            KindProjection::from(&kind),
            minimum_inputs,
            maximum_inputs,
        )?;
        self.variadic_kinds.insert(kind_id, kind);
        Ok(())
    }

    /// Installs canonical Kind truth while retaining the smaller checker view.
    pub fn insert_kind(&mut self, kind: conduit_core::Kind) -> Result<(), PlotError> {
        let projection = KindProjection::from(&kind);
        validate_unique_port_symbols(&projection)?;
        kind.validate()
            .map_err(|error| PlotError::InvalidKind(format!("{error:?}")))?;
        self.insert(projection)?;
        self.canonical_kinds.insert(kind.kind_id.clone(), kind);
        Ok(())
    }

    pub fn get(&self, kind_id: &KindId) -> Option<&KindProjection> {
        self.kinds.get(kind_id)
    }

    pub(crate) fn projection_for_arity(
        &self,
        kind_id: &KindId,
        input_count: usize,
    ) -> Result<Option<KindProjection>, String> {
        let Some(definition) = self.kinds.get(kind_id) else {
            return Ok(None);
        };
        let Some(family) = self.variadic_fores.get(kind_id) else {
            return Ok((definition.inputs.len() == input_count).then(|| definition.clone()));
        };
        let fore = family.specialize(input_count, Vec::new())?;
        Ok(Some(KindProjection {
            kind_id: definition.kind_id.clone(),
            kind_contract_revision: specialized_kind_identity(
                &definition.kind_contract_revision,
                input_count,
            ),
            inputs: fore.inputs().to_vec(),
            outputs: fore.outputs().to_vec(),
            configuration: definition.configuration.clone(),
        }))
    }

    pub(crate) fn is_homogeneous_variadic(&self, kind_id: &KindId) -> bool {
        self.variadic_fores.contains_key(kind_id)
    }

    pub(crate) fn canonical_kind_for_arity(
        &self,
        kind_id: &KindId,
        input_count: Option<usize>,
    ) -> Result<Option<conduit_core::Kind>, String> {
        if let Some(template) = self.variadic_kinds.get(kind_id) {
            let input_count = input_count.ok_or_else(|| {
                format!(
                    "variadic Gear '{}' requires an exact relational operand count",
                    kind_id.as_str()
                )
            })?;
            return self
                .variadic_fores
                .get(kind_id)
                .expect("variadic Kind retains its Fore family")
                .specialize_kind(template, input_count)
                .map(Some);
        }
        Ok(self.canonical_kinds.get(kind_id).cloned())
    }

    pub fn canonical_kind(&self, kind_id: &KindId) -> Option<&conduit_core::Kind> {
        self.canonical_kinds.get(kind_id)
    }

    pub(crate) fn canonical_kinds(&self) -> &BTreeMap<KindId, conduit_core::Kind> {
        &self.canonical_kinds
    }

    /// Derives the startup names and defaults needed to check canonical source.
    /// Structured startup types still require an explicitly assembled
    /// [`StartupCatalog`].
    pub fn startup_catalog(&self) -> Result<StartupCatalog, String> {
        let mut startup = StartupCatalog::new();
        for definition in self.kinds.values() {
            let canonical_kind = self
                .canonical_kind(&definition.kind_id)
                .or_else(|| self.variadic_kinds.get(&definition.kind_id));
            let signature = KindSignature {
                kind: definition.kind_id.as_str().to_string(),
                startup_parameters: canonical_kind.map_or_else(
                    || {
                        definition
                            .configuration
                            .iter()
                            .map(projected_startup_parameter)
                            .collect()
                    },
                    |kind| {
                        kind.startup_parameters
                            .iter()
                            .map(|parameter| StartupParameterSignature {
                                name: parameter.name.clone(),
                                value_type: parameter.value_type.as_str().to_string(),
                                default: parameter.has_default.then(|| {
                                    definition
                                        .configuration
                                        .iter()
                                        .find(|field| field.key == parameter.name)
                                        .map(|field| render_value(&field.default_value))
                                        .expect("validated Kind startup default has configuration")
                                }),
                            })
                            .collect()
                    },
                ),
            };
            startup.insert(signature)?;
            let fore = if let Some(kind) = canonical_kind {
                kind.checked_front()
            } else {
                let signature = startup
                    .signature(definition.kind_id.as_str())
                    .expect("the signature was inserted immediately above");
                let startup_parameters =
                    startup
                        .canonical_startup_parameters(signature)
                        .map_err(|error| {
                            format!(
                                "cannot derive checked Fore for '{}': {error:?}",
                                definition.kind_id.as_str()
                            )
                        })?;
                let shorthand = match (definition.inputs.as_slice(), definition.outputs.as_slice())
                {
                    ([input], [output]) => Some((input.port_id.clone(), output.port_id.clone())),
                    _ => None,
                };
                conduit_core::CheckedFront::new(
                    startup_parameters,
                    definition.inputs.clone(),
                    definition.outputs.clone(),
                    shorthand,
                )
            };
            if let Some(family) = self.variadic_fores.get(&definition.kind_id) {
                startup.insert_homogeneous_variadic_fore(
                    definition.kind_id.as_str(),
                    family.clone(),
                )?;
            } else {
                startup.insert_fore(definition.kind_id.as_str(), fore)?;
            }
        }
        Ok(startup)
    }

    pub fn insert_export(
        &mut self,
        plot: &CheckedPlot,
        capability_id: &CapabilityId,
    ) -> Result<CheckedCompositeBoundary, PlotError> {
        let boundary = plot.export_boundary(capability_id)?;
        self.insert(boundary.kind_projection())?;
        Ok(boundary)
    }
}

fn validate_unique_port_symbols(definition: &KindProjection) -> Result<(), PlotError> {
    let mut port_symbols = BTreeSet::new();
    if definition
        .inputs
        .iter()
        .chain(&definition.outputs)
        .any(|port| !port_symbols.insert(port.port_id.as_str()))
    {
        return Err(PlotError::InvalidKind(format!(
            "duplicate port symbol across the Fore of '{}'",
            definition.kind_id.as_str()
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlotError {
    SourceLimitExceeded,
    TokenLimitExceeded,
    IncompletePlot,
    MissingBlockEnd,
    DuplicateKind(String),
    InvalidExport(String),
    InvalidKind(String),
    InvalidIdentity(String),
    InvalidSyntax(String),
}

impl core::fmt::Display for PlotError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SourceLimitExceeded => write!(
                f,
                "plot source exceeds the {MAXIMUM_PLOT_SOURCE_BYTES}-byte limit"
            ),
            Self::TokenLimitExceeded => write!(
                f,
                "plot source exceeds the {MAXIMUM_PLOT_TOKENS}-token limit"
            ),
            Self::IncompletePlot => write!(f, "incomplete plot"),
            Self::MissingBlockEnd => write!(f, "expected closing '}}' at end of plot"),
            Self::DuplicateKind(kind) => write!(f, "duplicate profile kind '{kind}'"),
            Self::InvalidExport(message) => write!(f, "invalid export: {message}"),
            Self::InvalidKind(message) => write!(f, "invalid Kind: {message}"),
            Self::InvalidIdentity(message) => write!(f, "invalid plot identity: {message}"),
            Self::InvalidSyntax(message) => write!(f, "invalid canonical plot syntax: {message}"),
        }
    }
}

impl core::error::Error for PlotError {}

/// Parses the canonical `plot NAME (...) { ... }` surface without performing
/// catalog lookup or semantic lowering.
pub fn parse_syntax_document(source: &str) -> SyntaxDocument {
    surface_parser::parse_surface(source)
}

/// Checks immutable startup bindings in canonical Plot syntax without
/// recursively expanding plots or producing planner/runtime input.
pub fn check_syntax_document(
    document: &SyntaxDocument,
    catalog: &StartupCatalog,
) -> Result<CheckedSyntaxDocument, SyntaxCheckDiagnostic> {
    syntax_check::check_document(document, catalog)
}

pub fn parse(source: &str, catalog: &ProfileCatalog) -> Result<CheckedPlot, PlotError> {
    let startup = catalog
        .startup_catalog()
        .map_err(PlotError::InvalidSyntax)?;
    parse_with_startup(source, &startup, catalog)
}

pub fn parse_with_startup(
    source: &str,
    startup: &StartupCatalog,
    catalog: &ProfileCatalog,
) -> Result<CheckedPlot, PlotError> {
    let syntax = parse_syntax_document(source);
    if let Some(diagnostic) = syntax.diagnostics.first() {
        return Err(PlotError::InvalidSyntax(diagnostic.message.clone()));
    }
    let checked = check_syntax_document(&syntax, startup)
        .map_err(|diagnostic| PlotError::InvalidSyntax(diagnostic.message))?;
    let entry = checked
        .plots
        .last()
        .ok_or(PlotError::IncompletePlot)?
        .name
        .clone();
    let authoring = expand_canonical_plot_for_authoring(&checked, &entry, catalog)
        .map_err(|diagnostic| PlotError::InvalidSyntax(diagnostic.message))?;
    let expanded = authoring.expanded;
    let input_fronts = authoring
        .input_bindings
        .iter()
        .map(|binding| CheckedCompositeFront {
            external_port: authoring
                .front
                .inputs()
                .iter()
                .find(|port| port.port_id == binding.front_port_id)
                .expect("authoring input binding names a checked front port")
                .clone(),
            internal_gear_id: binding.gear_id.clone(),
            internal_port_id: binding.gear_port_id.clone(),
            track: binding.track,
            terminal: CompositeFrontTerminal::Independent,
        })
        .collect::<Vec<_>>();
    let output_fronts = authoring
        .output_bindings
        .iter()
        .map(|binding| CheckedCompositeFront {
            external_port: authoring
                .front
                .outputs()
                .iter()
                .find(|port| port.port_id == binding.front_port_id)
                .expect("authoring output binding names a checked front port")
                .clone(),
            internal_gear_id: binding.gear_id.clone(),
            internal_port_id: binding.gear_port_id.clone(),
            track: binding.track,
            terminal: CompositeFrontTerminal::Independent,
        })
        .collect::<Vec<_>>();
    let exports = if input_fronts.is_empty() && output_fronts.is_empty() {
        Vec::new()
    } else {
        vec![CheckedExport {
            capability_id: CapabilityId::from(entry.rsplit('/').next().unwrap_or(&entry)),
            kind_id: KindId::from(entry.as_str()),
            input_fronts,
            output_fronts,
        }]
    };
    let checked_plot_id = checked_plot_id(
        &expanded.name,
        expanded.completion,
        &expanded.gears,
        &expanded.connections,
        &exports,
    );
    let expanded_plot_id = expanded_plot_id(&checked_plot_id, &[]);
    Ok(CheckedPlot {
        source_document_id: expanded.source_document_id,
        checked_plot_id,
        expanded_plot_id,
        name: expanded.name,
        completion: expanded.completion,
        gears: expanded.gears,
        connections: expanded.connections,
        exports,
        nested_plots: Vec::new(),
    })
}

fn tokenize_losslessly(source: &str) -> Result<Vec<CstToken>, Span> {
    let mut tokens = Vec::new();
    let mut offset = 0;
    let mut line = 1;
    let mut column = 1;

    while offset < source.len() {
        let start = offset;
        let start_line = line;
        let start_column = column;
        let first = source[offset..]
            .chars()
            .next()
            .expect("offset is inside source");
        let kind;

        if first.is_whitespace() {
            kind = CstTokenKind::Whitespace;
            while offset < source.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset is inside source");
                if !next.is_whitespace() {
                    break;
                }
                advance(next, &mut offset, &mut line, &mut column);
            }
        } else if first == '#' {
            kind = CstTokenKind::Comment;
            while offset < source.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset is inside source");
                if next == '\n' {
                    break;
                }
                advance(next, &mut offset, &mut line, &mut column);
            }
        } else {
            kind = CstTokenKind::Lexeme;
            let mut quote = None;
            let mut escaped = false;
            while offset < source.len() {
                let next = source[offset..]
                    .chars()
                    .next()
                    .expect("offset is inside source");
                if quote.is_none() && (next.is_whitespace() || next == '#') {
                    break;
                }
                advance(next, &mut offset, &mut line, &mut column);
                if let Some(active) = quote {
                    if next == active && !escaped {
                        quote = None;
                    }
                    escaped = next == '\\' && !escaped;
                    if next != '\\' {
                        escaped = false;
                    }
                } else if matches!(next, '\'' | '"') {
                    quote = Some(next);
                }
            }
        }

        let span = Span {
            start,
            end: offset,
            line: start_line,
            column: start_column,
            end_line: line,
            end_column: column,
        };
        if tokens.len() == MAXIMUM_PLOT_TOKENS {
            return Err(span);
        }
        tokens.push(CstToken {
            kind,
            span,
            text: source[start..offset].to_string(),
        });
    }
    Ok(tokens)
}

fn advance(character: char, offset: &mut usize, line: &mut usize, column: &mut usize) {
    *offset += character.len_utf8();
    if character == '\n' {
        *line += 1;
        *column = 1;
    } else {
        *column += 1;
    }
}

fn whole_source_span(source: &str) -> Span {
    let end = eof_span(source);
    Span {
        start: 0,
        end: source.len(),
        line: 1,
        column: 1,
        end_line: end.line,
        end_column: end.column,
    }
}

fn eof_span(source: &str) -> Span {
    let mut line = 1;
    let mut column = 1;
    for character in source.chars() {
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    Span {
        start: source.len(),
        end: source.len(),
        line,
        column,
        end_line: line,
        end_column: column,
    }
}

fn diagnostic(error: PlotError, span: Span) -> PlotDiagnostic {
    let code = match error {
        PlotError::IncompletePlot => "CND-FRM-002",
        PlotError::MissingBlockEnd => "CND-FRM-004",
        PlotError::DuplicateKind(_) => "CND-FRM-006",
        PlotError::InvalidExport(_) => "CND-FRM-012",
        PlotError::SourceLimitExceeded => "CND-FRM-014",
        PlotError::TokenLimitExceeded => "CND-FRM-015",
        PlotError::InvalidIdentity(_) => "CND-FRM-018",
        PlotError::InvalidSyntax(_) => "CND-FRM-019",
        PlotError::InvalidKind(_) => "CND-FRM-020",
    };
    PlotDiagnostic {
        code,
        span,
        message: error.to_string(),
    }
}

fn validate_export_fronts(export: &CheckedExport, gears: &[CheckedGear]) -> Result<(), PlotError> {
    let mut names = BTreeSet::new();
    for front in export.input_fronts.iter().chain(&export.output_fronts) {
        if !names.insert(front.external_port.port_id.clone()) {
            return Err(PlotError::InvalidExport(format!(
                "duplicate front name '{}'",
                front.external_port.port_id.as_str()
            )));
        }
    }
    for (direction, fronts) in [
        (PortDirection::Input, &export.input_fronts),
        (PortDirection::Output, &export.output_fronts),
    ] {
        for front in fronts {
            if front.external_port.direction != direction {
                return Err(PlotError::InvalidExport(
                    "front direction differs from its export collection".into(),
                ));
            }
            let gear = gears
                .iter()
                .find(|gear| gear.gear_id == front.internal_gear_id)
                .ok_or_else(|| PlotError::InvalidExport("front names a missing Gear".into()))?;
            let endpoint = match direction {
                PortDirection::Input => &gear.inputs,
                PortDirection::Output => &gear.outputs,
            }
            .iter()
            .find(|port| port.port_id == front.internal_port_id)
            .ok_or_else(|| {
                PlotError::InvalidExport("front names a missing or wrongly directed Port".into())
            })?;
            let contract_matches = match front.track {
                conduit_core::ConnectionTrack::Payload => {
                    endpoint.value_kind == front.external_port.value_kind
                        && endpoint.abnormal_kind == front.external_port.abnormal_kind
                }
                conduit_core::ConnectionTrack::NormalClose => {
                    matches!(
                        endpoint.temporal,
                        conduit_core::PortTemporal::Flow { closes: true }
                    ) && front.external_port.value_kind.as_str() == conduit_core::UNIT_INFO_ID
                        && front.external_port.temporal == conduit_core::PortTemporal::Value
                }
                conduit_core::ConnectionTrack::AbnormalTerminal => {
                    endpoint.abnormal_kind.as_ref() == Some(&front.external_port.value_kind)
                        && front.external_port.temporal == conduit_core::PortTemporal::Value
                }
                conduit_core::ConnectionTrack::Quiescence => {
                    matches!(endpoint.temporal, conduit_core::PortTemporal::Flow { .. })
                        && front.external_port.value_kind.as_str() == conduit_core::UNIT_INFO_ID
                        && front.external_port.temporal == conduit_core::PortTemporal::Value
                }
            };
            if !contract_matches || front.terminal != CompositeFrontTerminal::Independent {
                return Err(PlotError::InvalidExport(
                    "front contract differs from its internal endpoint".into(),
                ));
            }
        }
    }
    Ok(())
}

fn canonical_plot_text(
    name: &str,
    completion: PlotCompletionPolicy,
    gears: &[CheckedGear],
    connections: &[CheckedConnection],
    exports: &[CheckedExport],
) -> String {
    let mut text = format!("plot:{name}\n");
    text.push_str(match completion {
        PlotCompletionPolicy::Live => "lifecycle:live|",
        PlotCompletionPolicy::SemanticCompletion => "lifecycle:complete|",
    });
    for gear in gears {
        text.push_str(&format!(
            "op:{}:{}:{}|",
            gear.gear_id.as_str(),
            gear.kind_id.as_str(),
            gear.kind_contract_revision.as_str()
        ));
        for port in gear.inputs.iter().chain(&gear.outputs) {
            let direction = match port.direction {
                conduit_core::PortDirection::Input => "input",
                conduit_core::PortDirection::Output => "output",
            };
            text.push_str(&format!(
                "port:{}:{}:{}:{}:{}|",
                port.port_id.as_str(),
                port.value_kind.as_str(),
                direction,
                port.temporal.as_str(),
                port.abnormal_kind
                    .as_ref()
                    .map_or("none", conduit_core::KindId::as_str)
            ));
        }
        push_terminal_transduction_text(&mut text, &gear.terminal_transductions);
        for resource in &gear.resource_ports {
            text.push_str(&format!(
                "resource-port:{}:{}:{:?}:{:?}:{:?}|",
                resource.port_id.as_str(),
                resource.class_id.as_str(),
                resource.ownership,
                resource.lifecycle,
                resource.mobility
            ));
        }
        for entry in &gear.configuration {
            text.push_str(&format!(
                "cfg:{}={}|",
                entry.key,
                render_value(&entry.value)
            ));
        }
    }
    for connection in connections {
        text.push_str(&format!(
            "conn:{}:{}->{}:{}:{}:{}|",
            connection.source_gear_id.as_str(),
            connection.source_port_id.as_str(),
            connection.sink_gear_id.as_str(),
            connection.sink_port_id.as_str(),
            connection.track.as_str(),
            connection.temporal.as_str()
        ));
    }
    for export in exports {
        text.push_str(&format!(
            "export:{}:{}|",
            export.capability_id.as_str(),
            export.kind_id.as_str(),
        ));
        for front in export.input_fronts.iter().chain(&export.output_fronts) {
            let direction = match front.external_port.direction {
                PortDirection::Input => "input",
                PortDirection::Output => "output",
            };
            text.push_str(&format!(
                "front:{direction}:{}:{}:{}:{}={}:{}:terminal-independent|",
                front.external_port.port_id.as_str(),
                front.external_port.value_kind.as_str(),
                front.external_port.temporal.as_str(),
                front
                    .external_port
                    .abnormal_kind
                    .as_ref()
                    .map_or("none", conduit_core::KindId::as_str),
                front.internal_gear_id.as_str(),
                front.internal_port_id.as_str(),
            ));
        }
    }
    text
}

fn push_terminal_transduction_text(
    text: &mut String,
    profiles: &[conduit_core::TerminalTransductionProfile],
) {
    use conduit_core::{
        AbnormalTerminalTransduction as Abnormal, CancellationTransduction as Cancellation,
        NormalCloseTransduction as Normal,
    };
    for profile in profiles {
        text.push_str("terminal-transduction:");
        text.push_str(profile.input_port_id.as_str());
        text.push('>');
        text.push_str(profile.output_port_id.as_str());
        text.push(':');
        match &profile.normal_close {
            Normal::NotAccepted => text.push_str("close/not-accepted"),
            Normal::PropagateAfterDrain => text.push_str("close/propagate-after-drain"),
            Normal::Consume => text.push_str("close/consume"),
            Normal::FlushThenPropagate(bound) => text.push_str(&format!(
                "close/flush-then-propagate/{}/{}",
                bound.maximum_items, bound.maximum_bytes
            )),
            Normal::FlushThenPropagateWhenAllClose(bound) => text.push_str(&format!(
                "close/flush-then-propagate-when-all-close/{}/{}",
                bound.maximum_items, bound.maximum_bytes
            )),
            Normal::PropagateWhenAllClose => text.push_str("close/propagate-when-all-close"),
            Normal::DomainSpecific { law } => {
                text.push_str(&format!("close/domain/{}", law.as_str()))
            }
        }
        text.push(':');
        match &profile.abnormal {
            Abnormal::NotAccepted => text.push_str("abnormal/not-accepted"),
            Abnormal::PropagateAfterDrain => text.push_str("abnormal/propagate-after-drain"),
            Abnormal::Recover => text.push_str("abnormal/recover"),
            Abnormal::FinalizeThenPropagate(bound) => text.push_str(&format!(
                "abnormal/finalize-then-propagate/{}/{}",
                bound.maximum_items, bound.maximum_bytes
            )),
            Abnormal::DomainSpecific { law } => {
                text.push_str(&format!("abnormal/domain/{}", law.as_str()))
            }
        }
        text.push(':');
        match &profile.cancellation {
            Cancellation::NotCancellable => text.push_str("cancel/not-cancellable"),
            Cancellation::Request { disposition_kind } => {
                text.push_str(&format!("cancel/request/{}", disposition_kind.as_str()))
            }
            Cancellation::DomainSpecific { law } => {
                text.push_str(&format!("cancel/domain/{}", law.as_str()))
            }
        }
        text.push('|');
    }
}

fn checked_plot_id(
    name: &str,
    completion: PlotCompletionPolicy,
    gears: &[CheckedGear],
    connections: &[CheckedConnection],
    exports: &[CheckedExport],
) -> CheckedPlotId {
    CheckedPlotId::from(hash_string(&canonical_plot_text(
        name,
        completion,
        gears,
        connections,
        exports,
    )))
}

fn expanded_plot_id(
    checked_plot_id: &CheckedPlotId,
    nested_plots: &[CheckedNestedPlot],
) -> ExpandedPlotId {
    let mut canonical = format!("expanded-plot:{}", checked_plot_id.as_str());
    for nested in nested_plots {
        canonical.push_str("|nested:");
        push_identity_field(&mut canonical, nested.gear_id.as_str());
        push_identity_field(&mut canonical, nested.export_capability_id.as_str());
        push_identity_field(&mut canonical, nested.plot.expanded_plot_id.as_str());
    }
    ExpandedPlotId::from(hash_string(&canonical))
}

fn exported_contract_revision(
    kind_id: &KindId,
    inputs: &[CheckedCompositeFront],
    outputs: &[CheckedCompositeFront],
) -> KindIdentity {
    let mut canonical = String::from("checked-export-contract:");
    push_identity_field(&mut canonical, kind_id.as_str());
    for (direction, fronts) in [("input", inputs), ("output", outputs)] {
        for front in fronts {
            push_identity_field(&mut canonical, direction);
            push_identity_field(&mut canonical, front.external_port.port_id.as_str());
            push_identity_field(&mut canonical, front.external_port.value_kind.as_str());
            push_identity_field(&mut canonical, front.external_port.temporal.as_str());
            push_identity_field(
                &mut canonical,
                front
                    .external_port
                    .abnormal_kind
                    .as_ref()
                    .map_or("none", conduit_core::KindId::as_str),
            );
            push_identity_field(&mut canonical, front.track.as_str());
            push_identity_field(
                &mut canonical,
                match front.terminal {
                    CompositeFrontTerminal::Independent => "independent",
                    CompositeFrontTerminal::Coupled => "coupled",
                },
            );
        }
    }
    KindIdentity::from(format!("checked-export:{}", hash_string(&canonical)))
}

fn push_identity_field(canonical: &mut String, value: &str) {
    canonical.push_str(&value.len().to_string());
    canonical.push(':');
    canonical.push_str(value);
    canonical.push('|');
}

fn projected_startup_parameter(field: &KindConfigurationField) -> StartupParameterSignature {
    StartupParameterSignature {
        name: field.key.clone(),
        value_type: match (&field.rule, &field.default_value) {
            (
                KindConfigurationRule::QuantityRange { canonical_unit, .. },
                ConfigurationValue::Quantity(_),
            ) => canonical_unit.dimension().info_id(),
            (_, ConfigurationValue::Bool(_)) => "Boolean",
            (_, ConfigurationValue::U64(_)) => "Count",
            (_, ConfigurationValue::I64(_)) => "Scalar",
            (_, ConfigurationValue::Text(_)) => "Text",
            (_, ConfigurationValue::Structured(value)) => value.profile().as_str(),
            (_, ConfigurationValue::Quantity(_)) => "Quantity",
        }
        .into(),
        // A legacy projection has no independently declared callable Fore, so
        // its historical configuration default remains the only available
        // omission contract. Canonical Kinds take the stricter branch above.
        default: Some(render_value(&field.default_value)),
    }
}

fn render_value(value: &ConfigurationValue) -> String {
    match value {
        ConfigurationValue::Bool(value) => value.to_string(),
        ConfigurationValue::U64(value) => value.to_string(),
        ConfigurationValue::I64(value) => value.to_string(),
        ConfigurationValue::Text(value) => format!("{value:?}"),
        ConfigurationValue::Structured(value) => alloc::format!(
            "<structured:{}:{}-bytes>",
            value.profile().as_str(),
            value.canonical_value().len()
        ),
        ConfigurationValue::Quantity(value) => {
            alloc::format!("{}{}", value.value(), value.unit().plot_suffix())
        }
    }
}

fn hash_string(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex(byte >> 4));
        encoded.push(hex(byte & 0x0f));
    }
    encoded
}

fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!("nibble out of range"),
    }
}

#[cfg(test)]
mod surface_tests;

#[cfg(test)]
mod activation_tests;
#[cfg(test)]
mod behavior_parameter_tests;
#[cfg(test)]
mod generic_plot_tests;
#[cfg(test)]
mod syntax_check_tests;

#[cfg(test)]
mod refinement_tests;

#[cfg(test)]
mod canonical_expansion_tests;

mod source_type_preparation;
pub use source_type_preparation::{prepare_source_types, PreparedSourceTypes};

mod ieee_literal;
