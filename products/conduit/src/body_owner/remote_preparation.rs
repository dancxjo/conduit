//! Preparation from retained owner truth, never caller-supplied approval bytes.
use super::Owner;
use conduit_body::{BodyPlayIdentity, PreparedBodyFragment};
use conduit_core::HostAdvertisement;
use conduit_embedded_build::{
    generate_prepared_body_workset, BodyFragmentGenerationRefusal, BodyFragmentProjectionLimits,
    BoundResidentExpression, GeneratedBodyWorksetDocument,
};

impl Owner {
    /// Resolve the selected resident implementations from current member offers;
    /// no arbitrary encoded projection crosses this owner preparation boundary.
    #[allow(dead_code)]
    pub(super) fn prepare_remote_resident_workset(
        &self,
        actual: &HostAdvertisement,
        images: &[&conduit_embedded_build::PreparedResidentExpression],
        limits: BodyFragmentProjectionLimits,
    ) -> Result<(BodyPlayIdentity, GeneratedBodyWorksetDocument), String> {
        self.prepare_remote_mixed_workset(actual, images, &[], limits)
    }

    /// Native offers are explicitly selected descriptive inputs, not possession
    /// or grants. The receiver must separately authorize and admit the exact
    /// native operation before execution. Resident-only selection stays strict.
    pub(super) fn prepare_remote_mixed_workset(
        &self,
        actual: &HostAdvertisement,
        images: &[&conduit_embedded_build::PreparedResidentExpression],
        native: &[&conduit_core::CapabilityOffer],
        limits: BodyFragmentProjectionLimits,
    ) -> Result<(BodyPlayIdentity, GeneratedBodyWorksetDocument), String> {
        self.prepare_remote_body_workset(actual, limits.maximum_document_bytes, |scope| {
            let resident = conduit_embedded_build::bind_current_mixed_resident_expressions(
                scope.plot_plan(),
                &scope.fragment().fragment_id,
                actual,
                images,
                native,
            )
            .map_err(BodyFragmentGenerationRefusal::Resident)?;
            Ok((resident, limits))
        })
    }
    // The installed remote dispatcher must call this before signing or sending.
    // It is not wired to Start while native receiver installation is absent.
    #[allow(dead_code)]
    pub(super) fn prepare_remote_body_workset(
        &self,
        actual: &HostAdvertisement,
        maximum_document_bytes: u32,
        resolve: impl FnMut(
            PreparedBodyFragment<'_>,
        ) -> Result<
            (Vec<BoundResidentExpression>, BodyFragmentProjectionLimits),
            BodyFragmentGenerationRefusal,
        >,
    ) -> Result<(BodyPlayIdentity, GeneratedBodyWorksetDocument), String> {
        if self.host.is_playing() || actual.host_id == self.host.advertisement().host_id {
            return Err("remote preparation requires the idle original owner Host".into());
        }
        self.require_observed_member_offer(actual)?;
        let proposal = self
            .session
            .realization()
            .ok_or("checked Body proposal required")?;
        if proposal.play.is_some() {
            return Err("Body proposal is already Playing".into());
        }
        let play = self
            .host
            .prospective_body_play(&proposal.wake, &proposal.plan)?;
        let fragment = proposal
            .plan
            .plots
            .iter()
            .map(|plot| &plot.plan)
            .chain(
                proposal
                    .plan
                    .mask_topologies
                    .iter()
                    .flat_map(|topology| topology.chains.iter().map(|chain| &chain.plan)),
            )
            .flat_map(|plan| &plan.fragments)
            .find(|fragment| fragment.host_id == actual.host_id)
            .ok_or("current Body proposal has no fragment for that member")?;
        let scope = PreparedBodyFragment::prepare(&proposal.wake, &proposal.plan, &play, fragment)
            .map_err(|_| "current Body fragment containment refused")?;
        let document =
            generate_prepared_body_workset(scope, actual, maximum_document_bytes, resolve)
                .map_err(|error| format!("remote Body projection refused: {error:?}"))?;
        Ok((play, document))
    }
}

impl Owner {
    /// Carry only a workset derived and signed by this live original owner.
    /// Source selects resident implementations, never arbitrary projection bytes.
    /// A transport failure preserves the original approval slot: no blind retry.
    pub(crate) fn submit_remote_workset_usb(
        &mut self,
        port: &std::path::Path,
        member: &super::super::MemberPresentationBasis,
        source: String,
        entries: &[String],
        native_capabilities: &[String],
    ) -> Result<serde_json::Value, String> {
        use conduit_std_host::pico_spawn::{
            PicoSpawnSocket, PreparationCommandBytes, PreparationExchange,
        };
        if source.len() > 256 * 1024
            || entries.is_empty()
            || entries.len() > 16
            || native_capabilities.len() > 16
            || entries.iter().any(|s| s.len() > 128)
            || native_capabilities.iter().any(|s| s.len() > 256)
        {
            return Err("remote implementation selection exceeds finite bounds".into());
        }
        self.require_current_native_member(member)?;
        self.require_observed_member_offer(&member.host_advertisement)?;
        // Refuse before compiling a codebook or opening USB when no checked
        // owner proposal exists. Full containment is rechecked at approval.
        if self.host.is_playing() || self.session.realization().is_none() {
            return Err("idle owner with a checked Body proposal required".into());
        }
        // This is the same checked product Source entrance used by authoring.
        let source = crate::plot_source::parse_owned(source)?;
        let checked = source.check()?;
        let mut images = Vec::new();
        for entry in entries {
            let expanded = conduit_plot::expand_canonical_plot_for_authoring(
                &checked,
                entry,
                source.authoring_catalog(),
            )
            .map_err(|e| format!("resident entry expansion: {e:?}"))?;
            let [gear] = expanded.expanded.gears.as_slice() else {
                return Err("resident entry must expand to one checked expression".into());
            };
            let [configuration] = gear.configuration.as_slice() else {
                return Err("resident expression configuration absent".into());
            };
            let conduit_core::ConfigurationValue::Text(text) = &configuration.value else {
                return Err("resident expression program absent".into());
            };
            if configuration.key != "program" {
                return Err("resident expression program absent".into());
            }
            let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(text)
                .map_err(|e| format!("resident program: {e:?}"))?;
            let temporal = gear.inputs.first().ok_or("resident input absent")?.temporal;
            images.push(
                conduit_embedded_build::PreparedResidentExpression::prepare(
                    &checked,
                    &program,
                    temporal,
                    conduit_embedded_build::ResidentExpressionBounds {
                        native: conduit_plot::rust_binding::PreparedNativeBounds {
                            canonical_bytes: 4096,
                            scopes: 64,
                            invariants: 64,
                            contracts: 64,
                        },
                        maximum_occurrences: 4096,
                        maximum_program_bytes: 512 * 1024,
                        maximum_generated_bytes: 1024 * 1024,
                        private_work_units: 1000,
                    },
                )
                .map_err(|e| format!("resident implementation: {e:?}"))?,
            );
        }
        let mut native = Vec::new();
        for id in native_capabilities {
            if native_capabilities
                .iter()
                .filter(|candidate| *candidate == id)
                .count()
                != 1
            {
                return Err("duplicate native capability selection".into());
            }
            native.push(
                member
                    .host_advertisement
                    .capabilities
                    .iter()
                    .find(|offer| offer.capability_id.as_str() == id)
                    .ok_or("selected native capability is absent from current member offers")?,
            );
        }
        let images: Vec<_> = images.iter().collect();
        let limits = conduit_embedded_build::BodyFragmentProjectionLimits {
            image: conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
            computations: conduit_embedded_build::EmbeddedComputationBounds {
                maximum_program_bytes: 512 * 1024,
                maximum_total_program_bytes: 1024 * 1024,
            },
            assigned: conduit_core::AssignedPlanMaxima {
                encoded_bytes: 4096,
                runtime_state_bytes: 16384,
                counts: [32; conduit_core::ASSIGNED_PLAN_COUNT_KINDS],
            },
            maximum_fore_ports: 16,
            maximum_document_bytes: 4096,
        };
        let mut socket = PicoSpawnSocket::open(port.to_str().ok_or("USB path is not UTF-8")?)
            .map_err(|e| format!("open preparation carrier: {e:?}"))?;
        let mut approval_refusal = None;
        let refusal = &mut approval_refusal;
        let exchange = socket
            .exchange_preparation(std::time::Duration::from_secs(10), move |challenge| {
                let command = self
                    .approve_remote_workset(member, challenge, &images, &native, limits)
                    .map_err(|error| { *refusal = Some(error); })?;
                Ok(PreparationCommandBytes {
                    payload: command.payload(),
                    signature: command.signature(),
                })
            })
            .map_err(|e| {
                format!("remote workset delivery: {e:?}; owner: {}; any original approval remains retained", approval_refusal.as_deref().unwrap_or("no approval refusal reported"))
            })?;
        let (occupied, digest) = match exchange {
            PreparationExchange::InputReported(report) => (false, report.payload_sha256),
            PreparationExchange::Occupied(report) => (true, report.payload_sha256),
        };
        Ok(
            serde_json::json!({"phase":"input-retained", "evidence":"device-report", "occupied":occupied,
            "payload_sha256":digest, "admission":"not-confirmed", "play":"not-confirmed"}),
        )
    }
}
