//! Prepared source evaluation only, not retained Body state or current input authority.
use crate::PreparedCheckedSourcePlan;
use alloc::vec::Vec;
use conduit_core::{
    validate_canonical_structured_value, ConfigurationValue, PlanId, PlotIdentity,
    PreparedStructuredComposer, StructuredInfoType, ValidatedCanonicalStructuredValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    NativeTypeValueContract, PortableExpressionProgram, PreparedPortableExpressionEvaluator,
    ProfileCatalog, StartupCatalog, PURE_EXPRESSION_REVISION,
};

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/parsec_map_types.rs"));
    pub(super) fn state_contracts() -> Vec<conduit_plot::NativeTypeValueContract> {
        ParsecMapState::value_contracts()
    }
    pub(super) fn command_contracts() -> Vec<conduit_plot::NativeTypeValueContract> {
        ParsecMapCommand::value_contracts()
    }
}
pub use generated::{ParsecMapCommand, ParsecMapState, ParsecMapTransitionResult};

pub(crate) fn parsec_map_state_checked_type(
) -> Result<conduit_plot::CheckedNativeType, conduit_plot::rust_binding::NativeBindingRefusal> {
    let value_type = ParsecMapState::semantic_type()?;
    let conduit_core::StructuredInfoTypeShape::Record { schema, .. } = value_type.shape() else {
        return Err(
            conduit_plot::rust_binding::NativeBindingRefusal::InvalidValue(
                conduit_core::StructuredInfoRefusal::WrongType,
            ),
        );
    };
    Ok(conduit_plot::CheckedNativeType {
        name: "ParsecMapState".into(),
        identity: schema.clone(),
        value_type,
        value_contracts: generated::state_contracts(),
        // parsec_map_build.rs fences any future owner where law.
        invariants: Vec::new(),
    })
}

/// Exact specimen source bundle. Consumers must check and plan this source;
/// source correspondence is not execution or physical input admission.
pub const PARSEC_MAP_TRANSITION_SOURCE: &str = concat!(
    include_str!("../components/parsec-state-step.conduit"),
    "\n",
    include_str!("../components/parsec-map-transition.conduit"),
);
/// Prepared whole-frame storage and admitted boundary queue size. This is
/// distinct from authored action/revision laws: the ordinary expression Fore
/// uses its existing exact unannotated contract rather than declaring this size.
pub const PARSEC_MAP_FRAME_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsecMapTransitionRefusal {
    Source,
    Shape,
    Preparation,
    State,
    Command,
    Evaluation,
    Output,
}

struct SourceCustody {
    plan: PlanId,
    plot: PlotIdentity,
}

/// Allocated before Play. Evaluation accepts borrowed canonical frames and
/// uses prepared composers/evaluators only; it never accepts a caller's step.
pub struct PreparedParsecMapTransition {
    game: PreparedPortableExpressionEvaluator,
    map: PreparedPortableExpressionEvaluator,
    game_request: PreparedStructuredComposer,
    map_request: PreparedStructuredComposer,
    next_state: PreparedStructuredComposer,
    state_laws: PreparedParsecLaws,
    command_laws: PreparedParsecLaws,
    game_custody: SourceCustody,
    map_custody: SourceCustody,
}

pub struct SourceEvaluatedParsecMapTransition<'a> {
    result: &'a [u8],
    state: &'a [u8],
    game_custody: &'a SourceCustody,
    map_custody: &'a SourceCustody,
}
impl SourceEvaluatedParsecMapTransition<'_> {
    pub fn result_bytes(&self) -> &[u8] {
        self.result
    }
    pub fn state_bytes(&self) -> &[u8] {
        self.state
    }
    pub fn game_plan_id(&self) -> &PlanId {
        &self.game_custody.plan
    }
    pub fn map_plan_id(&self) -> &PlanId {
        &self.map_custody.plan
    }
    pub fn game_plot(&self) -> &PlotIdentity {
        &self.game_custody.plot
    }
    pub fn map_plot(&self) -> &PlotIdentity {
        &self.map_custody.plot
    }
}

struct PreparedParsecLaws {
    schema: Vec<u8>,
    contracts: Vec<NativeTypeValueContract>,
}
impl PreparedParsecLaws {
    fn new(
        ty: StructuredInfoType,
        contracts: Vec<NativeTypeValueContract>,
    ) -> Result<Self, ParsecMapTransitionRefusal> {
        Ok(Self {
            schema: ty
                .canonical_bytes()
                .map_err(|_| ParsecMapTransitionRefusal::Preparation)?,
            contracts,
        })
    }
    fn validate(&self, value: ValidatedCanonicalStructuredValue<'_>) -> Result<(), ()> {
        if value.type_bytes() != self.schema {
            return Err(());
        }
        for law in &self.contracts {
            let mut field = value;
            for name in law
                .representation_path
                .strip_prefix('.')
                .ok_or(())?
                .split('.')
            {
                field = field.record_field(name).map_err(|_| ())?.ok_or(())?;
            }
            let bytes = field
                .primitive_bytes(law.contract.value_kind.as_str())
                .map_err(|_| ())?;
            law.contract.validate(bytes).map_err(|_| ())?;
        }
        Ok(())
    }
}

pub fn prepare_checked_parsec_map_transition(
    game: &PreparedCheckedSourcePlan<'_>,
    map: &PreparedCheckedSourcePlan<'_>,
) -> Result<PreparedParsecMapTransition, ParsecMapTransitionRefusal> {
    // Reproduction pins this specimen's actual authored programs, not merely
    // names or an imported record shape. All checking happens before Play.
    let checked = check_syntax_document(
        &parse_syntax_document(PARSEC_MAP_TRANSITION_SOURCE),
        &StartupCatalog::new(),
    )
    .map_err(|_| ParsecMapTransitionRefusal::Source)?;
    let profile = ProfileCatalog::new();
    let expected_game =
        expand_canonical_plot_for_authoring(&checked, "parsec/state-step", &profile)
            .map_err(|_| ParsecMapTransitionRefusal::Source)?;
    let expected_map =
        expand_canonical_plot_for_authoring(&checked, "parsec/map-transition", &profile)
            .map_err(|_| ParsecMapTransitionRefusal::Source)?;
    if game.expanded() != &expected_game || map.expanded() != &expected_map {
        return Err(ParsecMapTransitionRefusal::Source);
    }
    let game_program = program(game)?;
    let map_program = program(map)?;
    let native = |value: Result<
        StructuredInfoType,
        conduit_plot::rust_binding::NativeBindingRefusal,
    >| value.map_err(|_| ParsecMapTransitionRefusal::Shape);
    if game_program.input_type != native(generated::ParsecStepRequest::semantic_type())?
        || game_program.output_type != native(generated::ParsecStepResult::semantic_type())?
        || map_program.input_type != native(generated::ParsecMapTransitionRequest::semantic_type())?
        || map_program.output_type != native(ParsecMapTransitionResult::semantic_type())?
    {
        return Err(ParsecMapTransitionRefusal::Shape);
    }
    let composer = |ty: &StructuredInfoType| {
        PreparedStructuredComposer::new(ty, PARSEC_MAP_FRAME_BYTES)
            .map_err(|_| ParsecMapTransitionRefusal::Preparation)
    };
    Ok(PreparedParsecMapTransition {
        game: PreparedPortableExpressionEvaluator::new(&game_program)
            .map_err(|_| ParsecMapTransitionRefusal::Preparation)?,
        map: PreparedPortableExpressionEvaluator::new(&map_program)
            .map_err(|_| ParsecMapTransitionRefusal::Preparation)?,
        game_request: composer(&game_program.input_type)?,
        map_request: composer(&map_program.input_type)?,
        next_state: composer(&native(ParsecMapState::semantic_type())?)?,
        state_laws: PreparedParsecLaws::new(
            native(ParsecMapState::semantic_type())?,
            generated::state_contracts(),
        )?,
        command_laws: PreparedParsecLaws::new(
            native(ParsecMapCommand::semantic_type())?,
            generated::command_contracts(),
        )?,
        game_custody: custody(game),
        map_custody: custody(map),
    })
}

fn program(
    source: &PreparedCheckedSourcePlan<'_>,
) -> Result<PortableExpressionProgram, ParsecMapTransitionRefusal> {
    let [gear] = source.expanded().expanded.gears.as_slice() else {
        return Err(ParsecMapTransitionRefusal::Shape);
    };
    if gear.kind_contract_revision.as_str() != PURE_EXPRESSION_REVISION {
        return Err(ParsecMapTransitionRefusal::Shape);
    }
    let [field] = gear.configuration.as_slice() else {
        return Err(ParsecMapTransitionRefusal::Shape);
    };
    let ("program", ConfigurationValue::Text(encoded)) = (field.key.as_str(), &field.value) else {
        return Err(ParsecMapTransitionRefusal::Shape);
    };
    PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|_| ParsecMapTransitionRefusal::Shape)
}
fn custody(source: &PreparedCheckedSourcePlan<'_>) -> SourceCustody {
    let expanded = &source.expanded().expanded;
    SourceCustody {
        plan: source.plan().plan_id.clone(),
        plot: PlotIdentity {
            source_document_id: expanded.source_document_id.clone(),
            checked_plot_id: expanded.checked_plot_id.clone(),
            expanded_plot_id: expanded.expanded_plot_id.clone(),
        },
    }
}

impl PreparedParsecMapTransition {
    /// Prior state remains supplied until an installed Body scan owns it.
    /// Refused actions are source results with unchanged state, distinct from
    /// malformed frames/source custody failures. No input availability is proven.
    pub fn evaluate<'a>(
        &'a mut self,
        state: &[u8],
        command: &[u8],
    ) -> Result<SourceEvaluatedParsecMapTransition<'a>, ParsecMapTransitionRefusal> {
        if state.len() > PARSEC_MAP_FRAME_BYTES {
            return Err(ParsecMapTransitionRefusal::State);
        }
        if command.len() > PARSEC_MAP_FRAME_BYTES {
            return Err(ParsecMapTransitionRefusal::Command);
        }
        let state = validate_canonical_structured_value(state)
            .map_err(|_| ParsecMapTransitionRefusal::State)?;
        let command = validate_canonical_structured_value(command)
            .map_err(|_| ParsecMapTransitionRefusal::Command)?;
        self.state_laws
            .validate(state)
            .map_err(|_| ParsecMapTransitionRefusal::State)?;
        self.command_laws
            .validate(command)
            .map_err(|_| ParsecMapTransitionRefusal::Command)?;
        let request = self
            .game_request
            .record(&[field(command, "action")?, field(state, "started")?])
            .map_err(|_| ParsecMapTransitionRefusal::Evaluation)?;
        let step = self
            .game
            .evaluate(request)
            .map_err(|_| ParsecMapTransitionRefusal::Evaluation)?;
        let step = validate_canonical_structured_value(step)
            .map_err(|_| ParsecMapTransitionRefusal::Output)?;
        let request = self
            .map_request
            .record(&[command, state, step])
            .map_err(|_| ParsecMapTransitionRefusal::Evaluation)?;
        let result = self
            .map
            .evaluate(request)
            .map_err(|_| ParsecMapTransitionRefusal::Evaluation)?;
        if result.len() > PARSEC_MAP_FRAME_BYTES {
            return Err(ParsecMapTransitionRefusal::Output);
        }
        let borrowed = validate_canonical_structured_value(result)
            .map_err(|_| ParsecMapTransitionRefusal::Output)?;
        let next = field(borrowed, "state")?;
        self.state_laws
            .validate(next)
            .map_err(|_| ParsecMapTransitionRefusal::Output)?;
        let state = self
            .next_state
            .record(&[
                field(next, "keyboard")?,
                field(next, "mapping_revision")?,
                field(next, "started")?,
            ])
            .map_err(|_| ParsecMapTransitionRefusal::Output)?;
        Ok(SourceEvaluatedParsecMapTransition {
            result,
            state,
            game_custody: &self.game_custody,
            map_custody: &self.map_custody,
        })
    }

    pub fn prepared_buffer_bytes(&self) -> usize {
        self.game.output_capacity()
            + self.map.output_capacity()
            + self.game_request.prepared_heap_bytes()
            + self.map_request.prepared_heap_bytes()
            + self.next_state.prepared_heap_bytes()
    }
}

fn field<'a>(
    value: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> Result<ValidatedCanonicalStructuredValue<'a>, ParsecMapTransitionRefusal> {
    value
        .record_field(name)
        .map_err(|_| ParsecMapTransitionRefusal::Shape)?
        .ok_or(ParsecMapTransitionRefusal::Shape)
}
