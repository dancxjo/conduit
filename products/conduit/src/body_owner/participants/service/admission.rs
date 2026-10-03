//! Canonical challenge and proof operations on the current service Owner.
use super::*;

impl Owner {
    pub(crate) fn browser_begin(
        &mut self,
        window_id: &str,
        binding: &LinkBindingId,
        frame: In,
        encoded_bytes: u32,
    ) -> Result<Out, String> {
        let mut window = self
            .pending_browser
            .take()
            .ok_or("no browser admission window")?;
        let result = (|| {
            let at = window.check(window_id)?;
            if !matches!(window.state, WindowState::Ready) {
                return Err("browser admission window already has a current carrier".into());
            }
            let mut manager = self.admissions.clone().unwrap_or(
                AdmissionManager::new(self.session.evidence().body_id.clone()).map_err(debug)?,
            );
            match frame {
                In::Advertise {
                    protocol: PROTOCOL,
                    advertisement,
                    friendly_label,
                    verifying_key,
                    freshness_sequence,
                } => {
                    if advertisement.host_id != window.expected {
                        return Err(
                            "browser Host differs from explicit admission authorization".into()
                        );
                    }
                    if self.session.evidence().membership.parts.iter().any(|part| {
                        part.current
                            .as_ref()
                            .is_some_and(|current| current.host_id == window.expected)
                    }) {
                        return Err("browser Host already has current membership".into());
                    }
                    if manager
                        .receipts
                        .iter()
                        .any(|receipt| receipt.credential.host_id == window.expected)
                    {
                        return Err("admitted browser must present its retained credential".into());
                    }
                    let offered: [u8; 32] = verifying_key
                        .try_into()
                        .map_err(|_| "invalid browser verifying key")?;
                    if window.new_key != Some(offered) {
                        return Err(
                            "new browser proof key differs from explicit admission authorization"
                                .into(),
                        );
                    }
                    let observation = CandidateObservation {
                        advertisement,
                        friendly_label,
                        freshness_sequence,
                        encoded_bytes,
                        observed_binding_id: binding.clone(),
                        observation_sign_id: signal(binding, "observed"),
                        proof_id: DiscoveryProofId::bind(binding.as_str()).map_err(debug)?,
                    };
                    let mut candidates =
                        CandidateInventory::new(manager.body_id.clone()).map_err(debug)?;
                    let candidate = candidates.observe(observation.clone()).map_err(debug)?;
                    let challenge = manager
                        .begin_ambient(
                            &mut candidates,
                            &candidate,
                            offered,
                            nonce()?,
                            at,
                            window.expiry(at)?,
                            signal(binding, "requested"),
                        )
                        .map_err(debug)?;
                    window.state = WindowState::Pending(Pending {
                        admission_id: challenge.admission_id.clone(),
                        binding: binding.clone(),
                        observation,
                        kind: PendingKind::Ambient(candidates),
                    });
                    self.admissions = Some(manager);
                    Ok(Out::Challenge {
                        protocol: PROTOCOL,
                        challenge,
                    })
                }
                In::ReturnAdvertise {
                    protocol: PROTOCOL,
                    credential,
                    advertisement,
                } => {
                    if advertisement.host_id != window.expected
                        || credential.host_id != window.expected
                    {
                        return Err(
                            "returning browser Host differs from explicit authorization".into()
                        );
                    }
                    if manager
                        .receipts
                        .iter()
                        .rev()
                        .find(|receipt| receipt.credential.part_id == credential.part_id)
                        .is_none_or(|receipt| receipt.credential != credential)
                    {
                        return Err("returning browser credential is stale or unknown".into());
                    }
                    let challenge = manager
                        .begin_return(
                            &self.session.evidence().membership,
                            &credential.part_id,
                            &advertisement,
                            nonce()?,
                            at,
                            window.expiry(at)?,
                        )
                        .map_err(debug)?;
                    let observation = CandidateObservation {
                        advertisement: advertisement.clone(),
                        friendly_label: "Returning browser".into(),
                        observed_binding_id: binding.clone(),
                        observation_sign_id: signal(binding, "returned"),
                        proof_id: DiscoveryProofId::bind(challenge.admission_id.as_str())
                            .map_err(debug)?,
                        freshness_sequence: self.session.evidence().membership.revision.0,
                        encoded_bytes,
                    };
                    window.state = WindowState::Pending(Pending {
                        admission_id: challenge.admission_id.clone(),
                        binding: binding.clone(),
                        observation,
                        kind: PendingKind::Returning(advertisement),
                    });
                    self.admissions = Some(manager);
                    Ok(Out::ReturnChallenge {
                        protocol: PROTOCOL,
                        challenge,
                    })
                }
                _ => Err("first browser frame must advertise admission or return".into()),
            }
        })();
        self.pending_browser = Some(window);
        result
    }

    pub(crate) fn browser_complete(
        &mut self,
        root: &Path,
        window_id: &str,
        frame: In,
    ) -> Result<BrowserAdmittedSnapshot, String> {
        let mut window = self
            .pending_browser
            .take()
            .ok_or("no browser admission window")?;
        let result = (|| {
            let at = window.check(window_id)?;
            let WindowState::Pending(pending) = &window.state else {
                return Err("browser admission window has no pending challenge".into());
            };
            let mut manager = self
                .admissions
                .clone()
                .ok_or("browser admission manager absent")?;
            let mut session = self.session.clone();
            let authority = self.host.advertisement();
            let credential = match (&pending.kind, frame) {
                (
                    PendingKind::Ambient(candidates),
                    In::AmbientProof {
                        protocol: PROTOCOL,
                        admission_id,
                        body_id,
                        host_id,
                        boot_id,
                        nonce,
                        signature,
                    },
                ) if admission_id == pending.admission_id => {
                    let proof = AmbientAdmissionProof {
                        admission_id,
                        body_id,
                        host_id,
                        boot_id,
                        nonce: nonce.try_into().map_err(|_| "invalid proof nonce")?,
                        signature: signature
                            .try_into()
                            .map_err(|_| "invalid proof signature")?,
                    };
                    let mut candidates = candidates.clone();
                    session
                        .admit_ambient_host(
                            &mut manager,
                            &mut candidates,
                            &proof,
                            at,
                            &authority.host_id,
                            &authority.boot_id,
                        )
                        .map_err(debug)?
                }
                (
                    PendingKind::Returning(advertisement),
                    In::ReturnProof {
                        protocol: PROTOCOL,
                        admission_id,
                        body_id,
                        part_id,
                        host_id,
                        boot_id,
                        nonce,
                        signature,
                    },
                ) if admission_id == pending.admission_id => {
                    let proof = PartReturnProof {
                        admission_id,
                        body_id,
                        part_id,
                        host_id,
                        boot_id,
                        nonce: nonce.try_into().map_err(|_| "invalid return nonce")?,
                        signature: signature
                            .try_into()
                            .map_err(|_| "invalid return signature")?,
                    };
                    session
                        .admit_returning_host(
                            &mut manager,
                            advertisement,
                            &proof,
                            at,
                            &authority.host_id,
                            &authority.boot_id,
                        )
                        .map_err(debug)?
                }
                _ => return Err("browser proof differs from current challenge".into()),
            };
            remaining(window.deadline)?;
            let offer = disclose_host_offer(
                &pending.observation,
                RemoteProofClass::SelfReported,
                &OfferDisclosureRequest {
                    stage: OfferDisclosureStage::AdmittedMembership,
                    capability_ids: vec![],
                    resource_pool_ids: vec![],
                },
            )
            .map_err(debug)?;
            let before_session = std::mem::replace(&mut self.session, session);
            let before_manager = self.admissions.replace(manager);
            if let Err(error) = self.persist(root) {
                self.session = before_session;
                self.admissions = before_manager;
                return Err(error);
            }
            let snapshot = BrowserAdmittedSnapshot {
                credential: credential.clone(),
                biography: Box::new(self.session.evidence().clone()),
                offer: Box::new(offer),
            };
            window.state = WindowState::Active(credential);
            Ok(snapshot)
        })();
        self.pending_browser = Some(window);
        result
    }
}
