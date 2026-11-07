use super::*;
use conduit_host_make::{build_host_image, check_host_configuration, BuildInputs};
use std::io::{Read, Seek, SeekFrom};

type ArtifactAdmissionIdentity = (String, SporeOutputKind, String, u64, Option<String>);

impl CrecheEncounterFactory {
    pub fn new(
        catalog: MakeCatalog,
        packages: MakePackageSet,
        installation: CrecheEncounterInstallation,
        mut artifacts: Vec<CrecheReviewedArtifact>,
    ) -> Result<Self, String> {
        let b = installation.production_bounds;
        if !installation.limits.valid()
            || installation.encounter_id.is_empty()
            || installation.encounter_id.len() > 128
            || installation.encounter_id.chars().any(char::is_control)
            || installation.resource_pool.as_str().is_empty()
            || b.maximum_artifact_bytes == 0
            || !(1..=1024 * 1024).contains(&b.maximum_manifest_bytes)
            || b.maximum_receipt_bytes < 1024
            || b.maximum_receipt_bytes as u64 > installation.limits.maximum_evidence_bytes
            || installation.limits.maximum_evidence_bytes > 16 * 1024 * 1024
            || artifacts.is_empty()
            || artifacts.len() > 8
        {
            return Err("Crèche installation lacks finite admitted inventory/limits".into());
        }
        // Admit the form and immutable catalogue before publishing any slot.
        if let Some(source) = &installation.initial_source {
            HostConfigurationEncounter::load(source, &catalog, &packages)?;
        } else {
            HostConfigurationEncounter::new(&catalog, &packages)?;
        }
        for (index, artifact) in artifacts.iter().enumerate() {
            if artifact.output == SporeOutputKind::NativeBundle
                && artifacts[..index].iter().any(|previous| {
                    previous.output == SporeOutputKind::NativeBundle
                        && previous.configuration.profile().target.key()
                            == artifact.configuration.profile().target.key()
                })
            {
                return Err("Crèche native release target selection is ambiguous".into());
            }
        }
        let mut identities = Vec::with_capacity(artifacts.len());
        for artifact in &mut artifacts {
            if identities.iter().any(|id: &ArtifactAdmissionIdentity| {
                id.0 == artifact.configuration.configuration_id()
            }) {
                return Err("Crèche reviewed artifact selection is ambiguous".into());
            }
            validate(artifact, &catalog, &packages, b)?;
            identities.push((
                artifact.configuration.configuration_id().into(),
                artifact.output.clone(),
                artifact.artifact_digest.clone(),
                artifact.artifact_bytes,
                artifact
                    .bundle
                    .as_ref()
                    .map(|b| b.release().image_content_digest().to_owned()),
            ));
        }
        let admission = serde_json::to_vec(&(
            "conduit.creche/installed-inventory@1",
            catalogue_digest(&catalog, &packages),
            &installation.encounter_id,
            &installation.resource_pool,
            installation.limits,
            (
                b.maximum_artifact_bytes,
                b.maximum_manifest_bytes,
                b.maximum_receipt_bytes,
            ),
            &installation.initial_source,
            identities,
        ))
        .map_err(|e| format!("Crèche inventory identity: {e}"))?;
        let resource = conduit_core::resource_offer(
            installation.resource_pool.as_str(),
            conduit_body_make::deployment_runtime_catalog::CRECHE_RESOURCE,
            1,
        );
        Ok(Self {
            inputs: InputsCustody::Reference(Arc::new(Inputs {
                catalog,
                packages,
                installation,
                artifacts,
                resource,
                admission_digest: Sha256::digest(admission).into(),
            })),
        })
    }
    /// Construct under an independently qualified Root IMAGE envelope. The
    /// original charge follows shared inputs into every encounter/controller.
    /// This does not advertise a provider or admit its per-Play effects.
    ///
    /// # Safety
    /// Root must own and authorize all selected files and inputs. The peak must
    /// cover complete synchronous preparation; retention must include all input
    /// backing/capacities and the shared IMAGE wrapper allocation. Shared stores
    /// require independent lifetime custody. The callback must leave no worker
    /// or platform effect live on error/unwind, and must not export unfunded
    /// input aliases.
    /// These cooperative bounds confer no Source, Make or device authority.
    pub unsafe fn prepare_in_image(
        root: &conduit_std_host::body_execution::ImageRootPreparation<'_>,
        preparation_bytes: usize,
        retained_bytes: usize,
        prepare: impl FnOnce() -> Result<Self, String>,
    ) -> Result<Self, String> {
        use conduit_std_host::body_execution::ImagePreparedValue;
        // The original Inputs Arc is retained as the value; this additional Arc
        // owns the value and its affine charge. Both headers/backing are covered
        // by Root's retained envelope, never an implicit enlarged grant.
        let minimum = core::mem::size_of::<ImagePreparedValue<Arc<Inputs>>>()
            + 2 * core::mem::size_of::<usize>();
        if retained_bytes < minimum {
            return Err("Crèche IMAGE retention cannot own its shared custody wrapper".into());
        }
        // SAFETY: caller qualifies the complete factory and retained shared
        // stores. The outer Arc is allocated only after its retained charge is
        // established, and that charge stays inside the Arc until its last drop.
        let prepared = unsafe {
            root.prepare_value(preparation_bytes, retained_bytes, || {
                match prepare()?.inputs {
                    InputsCustody::Reference(inputs) if Arc::strong_count(&inputs) == 1 => {
                        Ok(inputs)
                    }
                    _ => Err("Crèche IMAGE preparation requires unique unfunded inputs".into()),
                }
            })
        }?;
        Ok(Self {
            inputs: InputsCustody::Image(Arc::new(prepared)),
        })
    }

    /// Verify the original selected Host counter, never copied Host/IMAGE IDs.
    /// This checks input memory custody only, not provider or execution rights.
    pub fn verify_image_custody(&self, host: &conduit_std_host::StdHost) -> Result<(), String> {
        match &self.inputs {
            InputsCustody::Image(inputs) => host.verify_selected_image_prepared_value(inputs),
            InputsCustody::Reference(_) => Err("Crèche factory has no IMAGE input custody".into()),
        }
    }

    pub fn resource_offer(&self) -> ResourceOffer {
        self.inputs.resource.clone()
    }
    pub fn limits(&self) -> CrecheLimits {
        self.inputs.installation.limits
    }

    pub fn encounter(
        &mut self,
        binding: CrecheBodyBinding,
        custody: Box<dyn CrecheMaskShowCustody>,
    ) -> Result<(CrecheEncounterAdapter<'_>, CrecheEncounterController), String> {
        if matches!(&self.inputs, InputsCustody::Image(_)) {
            return Err("IMAGE Crèche factory requires its funded encounter entry".into());
        }
        let channel = self.prepare_channel(binding, custody)?;
        let local = self.prepare_local()?;
        Ok(Self::assemble(
            LocalCustody::Reference(local),
            ChannelCustody::Reference(Arc::new(channel)),
        ))
    }

    /// Prepare local and shared encounter stores on the actual Host counter.
    /// Both complete highwater envelopes remain charged until their owners drop.
    /// Views retain shared custody; workers require a separate pre-Play admission.
    /// Failed final Mask retirement quarantines shared state until process recovery.
    ///
    /// # Safety
    /// Root must independently qualify these complete lifetime envelopes for the
    /// selected context, form edits, retained Face revisions, Mask custody and
    /// destruction, including all spare capacity and ownership headers. Borrowed
    /// and externally shared stores need independent custody. Preparation must
    /// start no worker/effect that survives failure or unwind. These cooperative
    /// bounds are not allocator containment or permission to begin a Play.
    pub unsafe fn encounter_in_image(
        &mut self,
        host: &mut conduit_std_host::StdHost,
        local_bytes: usize,
        shared_bytes: usize,
        binding: CrecheBodyBinding,
        custody: Box<dyn CrecheMaskShowCustody>,
    ) -> Result<(CrecheEncounterAdapter<'_>, CrecheEncounterController), String> {
        use conduit_std_host::body_execution::ImagePreparedValue;
        self.verify_image_custody(host)?;
        if !host
            .advertisement()
            .resources
            .contains(&self.resource_offer())
            || !host
                .advertisement()
                .capabilities
                .contains(&conduit_body_make::deployment_runtime_catalog::creche_std_offer())
        {
            return Err("Crèche IMAGE encounter differs from the actual offered resource".into());
        }
        let minimum_local = core::mem::size_of::<Local<'_>>()
            .checked_add(self.limits().maximum_evidence_bytes as usize)
            .ok_or("Crèche local envelope overflow")?;
        let minimum_shared =
            core::mem::size_of::<ImagePreparedValue<Channel>>() + 2 * core::mem::size_of::<usize>();
        if local_bytes < minimum_local || shared_bytes < minimum_shared {
            return Err("Crèche IMAGE encounter cannot retain its mandatory stores".into());
        }
        // SAFETY: Root qualifies the complete bounded mutable local lifetime.
        let local = unsafe {
            host.prepare_selected_image_value(local_bytes, local_bytes, || {
                self.prepare_local().map(std::cell::UnsafeCell::new)
            })
        }?;
        // SAFETY: Root qualifies all shared state and exported Face revisions.
        // The outer Arc is included in the envelope before its allocation.
        let channel = unsafe {
            host.prepare_selected_image_value(shared_bytes, shared_bytes, || {
                self.prepare_channel(binding, custody)
            })
        }?;
        Ok(Self::assemble(
            LocalCustody::Image(local),
            ChannelCustody::Image(Arc::new(channel)),
        ))
    }

    fn assemble(
        local: LocalCustody<'_>,
        channel: ChannelCustody,
    ) -> (CrecheEncounterAdapter<'_>, CrecheEncounterController) {
        (
            CrecheEncounterAdapter {
                local,
                channel: channel.clone(),
            },
            CrecheEncounterController { channel },
        )
    }

    fn prepare_local(&self) -> Result<Local<'_>, String> {
        let encounter = match &self.inputs.installation.initial_source {
            Some(source) => HostConfigurationEncounter::load(
                source,
                &self.inputs.catalog,
                &self.inputs.packages,
            )?,
            None => HostConfigurationEncounter::new(&self.inputs.catalog, &self.inputs.packages)?,
        };
        let mut evidence = Vec::new();
        evidence
            .try_reserve_exact(self.limits().maximum_evidence_bytes as usize)
            .map_err(|_| "Crèche retained evidence allocation unavailable")?;
        Ok(Local {
            encounter,
            basis: None,
            request: None,
            evidence,
            outcome: None,
            begun: false,
            retired: false,
        })
    }

    fn prepare_channel(
        &self,
        binding: CrecheBodyBinding,
        custody: Box<dyn CrecheMaskShowCustody>,
    ) -> Result<Channel, String> {
        binding
            .biography
            .validate()
            .map_err(|e| format!("Crèche Body biography: {e:?}"))?;
        binding
            .invitation
            .validate(clock()?)
            .map_err(|e| format!("Crèche invitation: {e:?}"))?;
        if binding.biography.body_id != binding.invitation.claim.body_id {
            return Err("Crèche invitation belongs to another Body".into());
        }
        Ok(Channel {
            state: Mutex::new(State {
                active: false,
                retired: false,
                face: None,
                acknowledged: None,
                command: None,
                checked: None,
                request: None,
                started: false,
                worker: None,
                cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                result: None,
                products: None,
                published: false,
                worker_memory: None,
            }),
            custody: Mutex::new(custody),
            inputs: self.inputs.clone(),
            binding,
        })
    }
}

fn validate(
    a: &mut CrecheReviewedArtifact,
    catalog: &MakeCatalog,
    packages: &MakePackageSet,
    bounds: JoiningProductionBounds,
) -> Result<(), String> {
    let checked =
        check_host_configuration(a.configuration.configuration().clone(), catalog, packages)
            .map_err(|e| format!("Crèche artifact configuration: {e:?}"))?;
    if checked.configuration_id() != a.configuration.configuration_id()
        || checked.profile() != a.configuration.profile()
        || a.artifact_bytes == 0
        || a.artifact_bytes > bounds.maximum_artifact_bytes
        || a.image_manifest.len() > bounds.maximum_manifest_bytes
        || a.source_identity.is_empty()
        || a.source_identity != a.image.manifest.source_identity
    {
        return Err("Crèche artifact envelope exceeds its exact reviewed selection".into());
    }
    let (expected, _) = build_host_image(
        checked.profile().clone(),
        catalog,
        packages,
        &a.output,
        &BuildInputs {
            source_identity: a.source_identity.clone(),
            toolchain_available: true,
        },
    )
    .map_err(|e| format!("Crèche artifact IMAGE: {e:?}"))?;
    if expected != a.image {
        return Err("Crèche artifact IMAGE/configuration mismatch".into());
    }
    conduit_host_make::verify_image_binding(&a.image, &a.image_manifest)
        .map_err(|e| format!("Crèche artifact manifest: {e:?}"))?;
    if let Some(bundle) = &a.bundle {
        bundle.validate(a, bounds)?;
        return Ok(());
    }
    if matches!(
        a.output,
        SporeOutputKind::NativeBundle | SporeOutputKind::BrowserBundle
    ) {
        return Err("Crèche aggregate output requires verified retained bundle payloads".into());
    }
    let metadata = a
        .artifact
        .metadata()
        .map_err(|e| format!("Crèche artifact metadata: {e}"))?;
    if !metadata.is_file() || metadata.len() != a.artifact_bytes {
        return Err("Crèche artifact file length/type mismatch".into());
    }
    a.artifact.rewind().map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0; 8192];
    loop {
        let capacity = (a.artifact_bytes - total).min(buffer.len() as u64).max(1) as usize;
        let n = a
            .artifact
            .read(&mut buffer[..capacity])
            .map_err(|e| format!("Crèche artifact read: {e}"))?;
        if n == 0 {
            break;
        }
        if n as u64 > a.artifact_bytes - total {
            return Err("Crèche artifact grew beyond reviewed length".into());
        }
        total += n as u64;
        hash.update(&buffer[..n]);
    }
    a.artifact
        .seek(SeekFrom::Start(0))
        .map_err(|e| e.to_string())?;
    if total != a.artifact_bytes || format!("sha256:{:x}", hash.finalize()) != a.artifact_digest {
        return Err("Crèche artifact differs from owner-reviewed digest".into());
    }
    Ok(())
}
