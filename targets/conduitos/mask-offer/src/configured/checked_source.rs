//! Checked auxiliary component Source and its ordinary planning consumer.
//! This owns preparation inputs, never a Body workset, reservation or Show.
use super::*;
use conduit_plot::CheckedSyntaxDocument;

pub struct CheckedNativeComponentSource {
    catalog: ProfileCatalog,
    backs: CanonicalBackCatalog,
    document: CheckedSyntaxDocument,
    profile: NativeMaskStorageProfile,
    renderer_kind: &'static str,
    capability: &'static str,
}

impl CheckedNativeComponentSource {
    /// Conservative retained owner envelope, including this inline owner.
    /// Shared backing is charged per reachable owner by the underlying visitors.
    /// This does not cover construction, expansion or correspondence scratch.
    pub fn owned_preparation_backing_bytes(&self) -> Option<usize> {
        core::mem::size_of::<Self>()
            .checked_add(self.catalog.owned_preparation_backing_bytes()?)?
            .checked_add(self.backs.owned_preparation_backing_bytes()?)?
            .checked_add(self.document.owned_preparation_backing_bytes()?)
    }

    pub(crate) fn prepare(
        source: &str,
        profile: NativeMaskStorageProfile,
        contract: RendererContract,
    ) -> Result<Self, MaskOfferError> {
        let mut catalog = ProfileCatalog::new();
        for projection in [
            face_interaction_kind_projection(),
            face_tee_kind_projection(),
        ] {
            catalog
                .insert(projection)
                .map_err(|_| MaskOfferError::InvalidPlan)?;
        }
        for kind in [
            show_resource_source_kind(),
            contract.renderer,
            contract.component.clone(),
        ] {
            catalog
                .insert_kind(kind)
                .map_err(|_| MaskOfferError::InvalidPlan)?;
        }
        let mut startup = catalog
            .startup_catalog()
            .map_err(|_| MaskOfferError::InvalidPlan)?;
        install_mask_plot_value_aliases(&mut startup).map_err(|_| MaskOfferError::InvalidPlan)?;
        for (alias, schema) in [
            ("ComponentRecipeInfo", component_recipe_type()),
            ("LookInfo", look_type()),
        ] {
            let profile = schema.profile().map_err(|_| MaskOfferError::InvalidPlan)?;
            startup
                .insert_structured_type(alias, schema.clone())
                .map_err(|_| MaskOfferError::InvalidPlan)?;
            startup
                .ensure_structured_type(profile.value_kind().as_str(), schema)
                .map_err(|_| MaskOfferError::InvalidPlan)?;
        }
        if contract
            .component
            .configuration
            .iter()
            .any(|field| field.key == "layout")
        {
            let schema = component_layout_type();
            let value_kind = schema
                .profile()
                .map_err(|_| MaskOfferError::InvalidPlan)?
                .value_kind()
                .clone();
            startup
                .insert_structured_type("ComponentLayoutInfo", schema.clone())
                .map_err(|_| MaskOfferError::InvalidPlan)?;
            startup
                .ensure_structured_type(value_kind.as_str(), schema)
                .map_err(|_| MaskOfferError::InvalidPlan)?;
        }
        let back = check_syntax_document(&parse_syntax_document(contract.back_source), &startup)
            .map_err(|_| MaskOfferError::InvalidPlan)?;
        let mut backs = CanonicalBackCatalog::new();
        backs
            .insert(&contract.component, &back, contract.back_root)
            .map_err(|_| MaskOfferError::InvalidPlan)?;
        let document = check_syntax_document(&parse_syntax_document(source), &startup)
            .map_err(|_| MaskOfferError::InvalidPlan)?;
        Ok(Self {
            catalog,
            backs,
            document,
            profile,
            renderer_kind: contract.kind_name,
            capability: contract.capability,
        })
    }

    /// Preserve the independent Source/expansion/Plan correspondence gate.
    /// Compiler inputs must remain funded until this method returns; the two
    /// output owners require their own retained custody before application Start.
    pub fn prepare_root(
        &self,
        host: &HostAdvertisement,
        root: &str,
    ) -> Result<(MaskStage, ConfiguredMaskConfiguration), MaskOfferError> {
        let profile = self.profile;
        profile.admit_host(host, self.capability)?;
        let document = &self.document;
        let catalog = &self.catalog;
        let backs = &self.backs;
        let authoring =
            expand_canonical_plot_for_authoring_with_backs(document, root, catalog, backs)
                .map_err(|_| MaskOfferError::InvalidPlan)?;
        let mask = MaskPlot::admit(&authoring).map_err(|_| MaskOfferError::InvalidPlan)?;
        let placements =
            default_expanded_placements(&authoring.expanded, core::slice::from_ref(host))
                .map_err(|_| MaskOfferError::InvalidPlan)?;
        let boundaries: BTreeMap<_, _> = [
            (PortDirection::Input, "face"),
            (PortDirection::Output, "show"),
            (PortDirection::Output, "interaction"),
        ]
        .into_iter()
        .map(|(direction, name)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: port_id(name),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: if name == "interaction" {
                        MAX_FACE_INTERACTION_BYTES as u32
                    } else {
                        profile.bytes()
                    },
                },
            )
        })
        .collect();
        let empty = BTreeMap::new();
        let lines = BTreeMap::new();
        let grants = vec![authority_grant(
            "conduitos/component-mask/present",
            &present_authority_requirement(kind_id("face/base/conduitos-surface@1")),
            host.host_id.clone(),
            host.boot_id.clone(),
            CapabilityId::from(self.capability),
        )];
        let bases = [BaseImplementationId::from("conduit.base/local@1")];
        let options = || PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: profile.bytes(),
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        };
        let plan = plan_expanded_authoring_with_options(
            &authoring,
            core::slice::from_ref(host),
            &placements,
            &bases,
            options(),
            &boundaries,
        )
        .map_err(|_| MaskOfferError::InvalidPlan)?;
        let planned =
            PlannedMaskPlot::admit(&mask, &plan).map_err(|_| MaskOfferError::InvalidPlan)?;
        let prepared = prepare_configured_mask(
            document,
            &authoring,
            catalog,
            backs,
            &planned,
            ConfiguredMaskPlanningInputs {
                hosts: core::slice::from_ref(host),
                placements: &placements,
                bases: &bases,
                options: options(),
                boundary_limits: &boundaries,
            },
            &kind_id(self.renderer_kind),
        )
        .map_err(|_| MaskOfferError::InvalidPlan)?;
        let configuration = prepared.configuration();
        Ok((
            MaskStage {
                planned_mask: planned,
            },
            configuration,
        ))
    }
}
