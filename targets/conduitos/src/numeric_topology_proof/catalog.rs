//! Generic owner contracts selected by a finite preparation recipe.
//! Layer order, recurrent equations and frame policy remain in checked Source.
use super::recipe::{Recipe, schema};
use alloc::{format, string::String, sync::Arc, vec, vec::Vec};
use conduit_ai::{
    native_profile::PreparedNativeProfile, nominal_weakening::PreparedNominalWeakening,
};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};

pub(super) struct Context {
    pub startup: StartupCatalog,
    pub profiles: ProfileCatalog,
    pub native: Vec<Arc<PreparedNativeProfile>>,
    pub weakening: Vec<Arc<PreparedNominalWeakening>>,
    pub guards: Vec<conduit_ai::fixed_numeric_guard::FixedGuardProfile>,
    pub pairs: Vec<conduit_ai::closing_structured_pair::ClosingStructuredPairProfile>,
    pub scalar: Arc<conduit_ai::fixed_numeric_u16_profile::PreparedU16Profile>,
    pub offers: Vec<CapabilityOffer>,
}
impl Context {
    pub fn prepare(recipe: &Recipe, definition: &str) -> Result<Self, String> {
        use conduit_ai::*;
        let mut startup = StartupCatalog::new();
        let mut profiles = ProfileCatalog::new();
        fixed_numeric_catalog::install_fixed_numeric_catalogs_capacity64(
            &mut startup,
            &mut profiles,
        )?;
        fixed_numeric_pair_catalog::install_fixed_numeric_pair_catalogs(
            &mut startup,
            &mut profiles,
        )?;
        fixed_numeric_flow::install_affine_flow_catalogs(&mut startup, &mut profiles)?;
        fixed_numeric_linear_flow::install_linear_flow_catalogs(&mut startup, &mut profiles)?;
        fixed_numeric_embedding_flow::install_embedding_flow_catalogs(&mut startup, &mut profiles)?;
        fixed_numeric_temporal::install_closing_numeric_catalogs_capacity64(
            &mut startup,
            &mut profiles,
        )?;
        fixed_numeric_pair_flow::install_fixed_flow_pair_catalogs(&mut startup, &mut profiles)?;
        fixed_numeric_dsp_catalog::install_fixed_dsp_flow_catalogs(&mut startup, &mut profiles)?;
        fixed_numeric_integer_narrowing::install_checked_integer_flow_catalogs(
            &mut startup,
            &mut profiles,
        )?;
        fixed_numeric_float_integer::install_float_integer_catalogs(&mut startup, &mut profiles)?;
        let scalar = Arc::new(
            fixed_numeric_u16_profile::PreparedU16Profile::check_definition(
                &recipe.u16_profile_source,
            )?,
        );
        scalar.install(&mut startup, &mut profiles, true)?;
        let mut offers = vec![scalar.offer(true)?];
        let mut native = vec![];
        for row in &recipe.native {
            let profile = Arc::new(PreparedNativeProfile::check_definition(
                definition, &row.name,
            )?);
            if profile.kind_identity(true) != row.kind {
                return Err("native recipe identity".into());
            }
            profile.install(&mut startup, &mut profiles, true)?;
            offers.push(profile.offer(true)?);
            native.push(profile);
        }
        let mut weakening = vec![];
        for row in &recipe.weakening {
            let profile = Arc::new(PreparedNominalWeakening::prepare(schema(
                &row.input_type_canonical,
            )?)?);
            if profile.kind_identity(true) != row.kind {
                return Err("weakening recipe identity".into());
            }
            profile.install(&mut startup, &mut profiles, true)?;
            offers.push(profile.offer(true)?);
            weakening.push(profile);
        }
        let mut guards = vec![];
        for row in &recipe.guards {
            let profile = fixed_numeric_guard::FixedGuardProfile::prepare(schema(
                &row.input_type_canonical,
            )?)?;
            if profile.contract()?.kind_id.as_str() != row.kind {
                return Err("guard recipe identity".into());
            }
            profile.install(&mut startup, &mut profiles)?;
            offers.push(profile.offer()?);
            guards.push(profile);
        }
        let mut pairs = vec![];
        for row in &recipe.pairs {
            let profile = closing_structured_pair::ClosingStructuredPairProfile::prepare(
                schema(&row.left_type_canonical)?,
                schema(&row.right_type_canonical)?,
            )?;
            if profile.identity() != row.kind {
                return Err("pair recipe identity".into());
            }
            profile.install(&mut startup, &mut profiles)?;
            offers.push(profile.offer()?);
            pairs.push(profile);
        }
        for fixture in &recipe.reference_fixture_placements {
            if !fixture.configuration.is_empty()
                || !fixture.semantic_contract.configuration.is_empty()
                || fixture.inputs.len() + fixture.outputs.len() != 1
                || !matches!(fixture.semantic_contract.laws.as_slice(), [KindSemanticLaw::ValueContracts(contracts)] if contracts.len()==1)
                || fixture.limits.max_active_instances != 1
                || fixture.limits.max_queue_items != 1
                || fixture.limits.max_queue_bytes > super::MAXIMUM_BOUNDARY_CONTRACT_BYTES
            {
                return Err("synthetic boundary supports one exact finite port only".into());
            }
            let kind = Kind {
                kind_id: fixture.kind_id.clone(),
                kind_contract_revision: fixture.kind_contract_revision.clone(),
                startup_parameters: vec![],
                shorthand: None,
                configuration: vec![],
                inputs: fixture.inputs.clone(),
                outputs: fixture.outputs.clone(),
                semantic_laws: fixture.semantic_contract.laws.clone(),
                limits: fixture.limits.clone(),
            };
            startup.insert(KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: vec![],
            })?;
            profiles
                .insert_kind(kind.clone())
                .map_err(|e| format!("{e:?}"))?;
            offers.push(
                BackOfferBuilder::new(
                    kind,
                    Back {
                        capability_id: format!(
                            "conduitos/static-numeric-boundary/{}",
                            fixture.kind_id.as_str()
                        )
                        .into(),
                        execution_profile_id: "conduitos/static-numeric-boundary@1".into(),
                        implementation_id: format!(
                            "conduitos/static-numeric-boundary/{}",
                            fixture.kind_id.as_str()
                        )
                        .into(),
                        artifact_id: "conduitos/static-numeric-boundary@1".into(),
                        host_calls: vec![],
                        resource_requirements: vec![],
                        authority_requirements: vec![],
                    },
                )
                .build(),
            );
        }
        Ok(Self {
            startup,
            profiles,
            native,
            weakening,
            guards,
            pairs,
            scalar,
            offers,
        })
    }
}

pub(super) fn numeric_offer(kind: &str) -> Result<CapabilityOffer, String> {
    use conduit_ai::operation_owners::*;
    fixed_numeric::fixed_numeric_offer_capacity64(kind)
        .or_else(|_| fixed_numeric_flow::fixed_affine_flow_offer(kind))
        .or_else(|_| fixed_numeric_linear_flow::fixed_linear_flow_offer(kind))
        .or_else(|_| fixed_numeric_embedding_flow::fixed_embedding_flow_offer(kind))
        .or_else(|_| conduit_ai::fixed_numeric_pair_flow::fixed_flow_pair_offer(kind))
        .or_else(|_| match kind {
            "numeric/flow-u64-to-u16" => {
                conduit_ai::fixed_numeric_integer_narrowing::checked_integer_narrowing_offer(true)
            }
            "numeric/flow-f32-to-i16-nearest-away160" => {
                conduit_ai::fixed_numeric_float_integer::float_integer_offer(true)
            }
            _ => Err(format!("unsupported numeric proof owner {kind}")),
        })
}
