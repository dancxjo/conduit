//! A canonical law claim and independent Source-custody verification.
use crate::rust_binding::{NativeLawRegistry, PreparedNativeBounds, PreparedNativeRefusal};
use crate::{
    NativeLawContext, NativeLawCustodyRefusal, NativeLawMaterial, NativeLawWitness,
    PortableExpressionEvaluationRefusal, PortableExpressionProgram, PreparedExpressionLawBounds,
    PreparedPortableExpressionEvaluator,
};
use alloc::{collections::BTreeMap, vec::Vec};
mod compilation;
mod definition;
mod wire;
pub use definition::*;
pub const LAW_EXPRESSION_PROFILE: &str = "conduitese/checked-native-laws@1";
pub const MAXIMUM_LAW_EXPRESSION_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LawExpressionRefusal {
    Encoding,
    Capacity,
    Profile,
    Context,
    Custody(NativeLawCustodyRefusal),
    Evaluation(PortableExpressionEvaluationRefusal),
}
/// Public claims do not grant Source custody or an installed realization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LawExpressionMaterial {
    pub program: PortableExpressionProgram,
    pub roots: Vec<NativeLawMaterial>,
}
impl LawExpressionMaterial {
    /// Strict lowercase transport encoding, bounded before allocation.
    pub fn from_canonical_hex(encoded: &str) -> Result<Self, LawExpressionRefusal> {
        if encoded.len() > MAXIMUM_LAW_EXPRESSION_BYTES * 2 {
            return Err(LawExpressionRefusal::Capacity);
        }
        if encoded.len() % 2 != 0 {
            return Err(LawExpressionRefusal::Encoding);
        }
        fn digit(byte: u8) -> Result<u8, LawExpressionRefusal> {
            match byte {
                b'0'..=b'9' => Ok(byte - b'0'),
                b'a'..=b'f' => Ok(byte - b'a' + 10),
                _ => Err(LawExpressionRefusal::Encoding),
            }
        }
        let mut bytes = Vec::with_capacity(encoded.len() / 2);
        for pair in encoded.as_bytes().chunks_exact(2) {
            bytes.push(digit(pair[0])? * 16 + digit(pair[1])?);
        }
        Self::from_canonical_bytes(&bytes)
    }
}
/// Independently retained original opaque Sources. Do not construct this from
/// an untrusted capsule/graph: compiler handoff supplies it separately.
#[derive(Clone, Debug)]
pub struct LawExpressionContext {
    roots: BTreeMap<conduit_core::KindId, NativeLawWitness>,
}
impl LawExpressionContext {
    pub fn from_source_witnesses(
        witnesses: &[NativeLawWitness],
    ) -> Result<Self, LawExpressionRefusal> {
        // Admit the entire actual closure before retaining a root table.
        NativeLawRegistry::new(witnesses).map_err(registry_refusal)?;
        let mut roots = BTreeMap::new();
        for witness in witnesses {
            let identity = witness.checked_type().identity.clone();
            if roots.insert(identity, witness.clone()).is_some() {
                return Err(LawExpressionRefusal::Context);
            }
        }
        Ok(Self { roots })
    }
}
/// Source-verified capsule; complete interpreter admission is still required
/// through `prepare_evaluator` before advertising any realization.
/// This object itself is not a Host offer, Plan or admitted output value.
pub struct VerifiedLawExpression {
    program: PortableExpressionProgram,
    registry: NativeLawRegistry,
    canonical: Vec<u8>,
}
impl VerifiedLawExpression {
    pub fn verify_with_context(
        encoded: &[u8],
        context: &LawExpressionContext,
    ) -> Result<Self, LawExpressionRefusal> {
        let material = LawExpressionMaterial::from_canonical_bytes(encoded)?;
        let mut witnesses = Vec::new();
        for root in &material.roots {
            let expected = context
                .roots
                .get(&root.claimed.identity)
                .ok_or(LawExpressionRefusal::Context)?;
            // A root's semantic Type alone does not authenticate unrelated
            // declarations in its module. Bind the entire original opaque
            // preimage before replay can expose any nested/imported metadata.
            let original = expected.material().map_err(LawExpressionRefusal::Custody)?;
            if root != &original {
                return Err(LawExpressionRefusal::Custody(
                    NativeLawCustodyRefusal::Source,
                ));
            }
            let anchors = expected
                .family_anchors()
                .map_err(LawExpressionRefusal::Custody)?;
            let witness = NativeLawWitness::verify_with_context(
                root,
                &expected.checked_type().value_type,
                &NativeLawContext { families: &anchors },
            )
            .map_err(LawExpressionRefusal::Custody)?;
            witnesses.push(witness);
        }
        let registry = NativeLawRegistry::new(&witnesses).map_err(registry_refusal)?;
        Ok(Self {
            program: material.program,
            registry,
            canonical: encoded.to_vec(),
        })
    }
    pub fn program(&self) -> &PortableExpressionProgram {
        &self.program
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub fn prepare_evaluator(
        &self,
    ) -> Result<PreparedPortableExpressionEvaluator, LawExpressionRefusal> {
        PreparedPortableExpressionEvaluator::new_checked_native_laws(
            &self.program,
            &self.registry,
            law_expression_bounds(),
        )
        .map_err(LawExpressionRefusal::Evaluation)
    }
}
/// Fixed profile bound by the versioned capsule and its distinct Kind revision.
pub fn law_expression_bounds() -> PreparedExpressionLawBounds {
    PreparedExpressionLawBounds {
        guards: 4096,
        native: PreparedNativeBounds {
            canonical_bytes: 4 * 1024 * 1024,
            scopes: 4096,
            invariants: 4096,
            contracts: 4096,
        },
        occurrences_per_guard: 4096,
        steps: 16 * 1024 * 1024,
    }
}

fn registry_refusal(reason: PreparedNativeRefusal) -> LawExpressionRefusal {
    match reason {
        PreparedNativeRefusal::Bounds => LawExpressionRefusal::Capacity,
        _ => LawExpressionRefusal::Context,
    }
}
