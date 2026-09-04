//! Atomic preparation and commit of one returned browser presence session.

use super::*;

pub(crate) struct PreparedReturnPresence {
    table: HostPresenceTable,
    sequence: u64,
    session_id: LinkBindingId,
    observed_at_millis: u64,
    expires_at_millis: u64,
    initial_response_lost_sign: SignId,
    session_sequence: u64,
    sign_sequence: u64,
    credential_index: usize,
}

impl BrowserPresenceCoordinator {
    pub(crate) fn prepare_return(
        &self,
        prior_credential: &MembershipCredential,
        returned_credential: &MembershipCredential,
        returned_membership: &BodyMembership,
    ) -> Result<PreparedReturnPresence, String> {
        if self.workers.len() == conduit_body::MAX_BODY_PARTS {
            return Err("browser presence worker capacity exhausted".into());
        }
        let credential_index = self
            .credentials
            .iter()
            .position(|retained| retained == prior_credential)
            .ok_or("returned browser credential is not retained exactly")?;
        if returned_credential.body_id != prior_credential.body_id
            || returned_credential.part_id != prior_credential.part_id
            || returned_credential.host_id != prior_credential.host_id
        {
            return Err("renewed browser credential changed durable membership identity".into());
        }
        let retained_lease = self
            .table
            .leases
            .iter()
            .find(|lease| lease.part_id == prior_credential.part_id)
            .ok_or("returned browser Part has no retained presence lease")?;
        if retained_lease.state != HostPresenceState::Unavailable {
            return Err("returned browser presence lease is still available".into());
        }
        let returned_host = returned_membership
            .parts
            .iter()
            .find(|part| part.part_id == prior_credential.part_id)
            .and_then(|part| part.current.as_ref())
            .ok_or("returned browser Part has no current membership Host")?;
        if retained_lease.host_id != prior_credential.host_id
            || retained_lease.boot_id != prior_credential.boot_id
            || retained_lease.host_id != returned_host.host_id
            || returned_credential.boot_id != returned_host.boot_id
            || retained_lease.offer_generation != returned_host.offer_generation
        {
            return Err("returned browser presence lease identity drifted".into());
        }
        let sequence = retained_lease
            .sequence
            .checked_add(1)
            .ok_or("returned browser presence sequence exhausted")?;
        let session_sequence = self
            .session_sequence
            .checked_add(1)
            .ok_or("browser presence session sequence exhausted")?;
        let started_sign_sequence = self
            .sign_sequence
            .checked_add(1)
            .ok_or("browser presence Sign sequence exhausted")?;
        let sign_sequence = started_sign_sequence
            .checked_add(1)
            .ok_or("browser presence Sign sequence exhausted")?;
        let session_id = LinkBindingId::from(format!(
            "patchbay/browser-presence/{}/{}",
            returned_credential.credential_id.as_str(),
            session_sequence
        ));
        let started_sign = presence_sign("started", started_sign_sequence);
        let initial_response_lost_sign = presence_sign("initial-response-lost", sign_sequence);
        let observed_at_millis = self.now_millis()?;
        let mut table = self.table.clone();
        table
            .start(
                returned_membership,
                &returned_credential.part_id,
                session_id.clone(),
                sequence,
                observed_at_millis,
                LEASE_MILLIS,
                started_sign,
            )
            .map_err(debug("preflight returned browser presence"))?;
        let expires_at_millis = table
            .leases
            .iter()
            .find(|lease| lease.part_id == returned_credential.part_id)
            .expect("preflighted returned lease is retained")
            .expires_at_millis;

        let mut loss_table = table.clone();
        let mut loss_membership = returned_membership.clone();
        loss_table
            .lose_session(
                &mut loss_membership,
                &returned_credential.part_id,
                &session_id,
                observed_at_millis,
                initial_response_lost_sign.clone(),
            )
            .map_err(debug("preflight returned presence cleanup"))?;
        Ok(PreparedReturnPresence {
            table,
            sequence,
            session_id,
            observed_at_millis,
            expires_at_millis,
            initial_response_lost_sign,
            session_sequence,
            sign_sequence,
            credential_index,
        })
    }

    pub(crate) fn prepare_return_socket(
        &self,
        socket: &BrowserAdmissionSocket,
    ) -> Result<(), String> {
        socket
            .set_read_timeout(Some(Duration::from_millis(50)))
            .map_err(debug("set returned browser presence timeout"))
    }

    pub(crate) fn commit_return(
        &mut self,
        mut socket: BrowserAdmissionSocket,
        credential: MembershipCredential,
        membership: &mut BodyMembership,
        prepared: PreparedReturnPresence,
    ) -> Result<u64, String> {
        self.table = prepared.table;
        self.session_sequence = prepared.session_sequence;
        self.sign_sequence = prepared.sign_sequence;
        self.credentials[prepared.credential_index] = credential.clone();
        let initial_response = socket
            .send(&BrowserAdmissionEgress::Admitted {
                protocol: BROWSER_ADMISSION_PROTOCOL,
                credential: credential.clone(),
            })
            .map_err(debug("send renewed browser membership credential"))
            .and_then(|()| {
                send_accepted(&mut socket, prepared.sequence, prepared.expires_at_millis)
            });
        if let Err(error) = initial_response {
            self.table
                .lose_session(
                    membership,
                    &credential.part_id,
                    &prepared.session_id,
                    prepared.observed_at_millis,
                    prepared.initial_response_lost_sign,
                )
                .expect("preflight proved returned presence cleanup");
            return Err(error);
        }
        let (receiver, outbound) = spawn_worker(socket);
        self.workers.push(PresenceWorker {
            credential,
            session_id: prepared.session_id,
            receiver,
            outbound,
        });
        Ok(prepared.sequence)
    }

    #[cfg(test)]
    pub(crate) fn set_return_sequence_for_test(
        &mut self,
        part_id: &conduit_body::PartId,
        sequence: u64,
    ) {
        self.table
            .leases
            .iter_mut()
            .find(|lease| &lease.part_id == part_id)
            .expect("test Part has retained presence")
            .sequence = sequence;
    }

    #[cfg(test)]
    pub(crate) fn make_return_lease_available_for_test(&mut self, part_id: &conduit_body::PartId) {
        let lease = self
            .table
            .leases
            .iter_mut()
            .find(|lease| &lease.part_id == part_id)
            .expect("test Part has retained presence");
        lease.state = HostPresenceState::Available;
        self.table
            .events
            .iter_mut()
            .rev()
            .find(|event| &event.part_id == part_id)
            .expect("test Part has retained presence event")
            .kind = conduit_body::HostPresenceEventKind::Started;
    }

    #[cfg(test)]
    pub(crate) fn drift_return_lease_for_test(&mut self, part_id: &conduit_body::PartId) {
        let drifted = conduit_core::HostId::from("browser/drifted-return-host");
        self.table
            .leases
            .iter_mut()
            .find(|lease| &lease.part_id == part_id)
            .expect("test Part has retained presence")
            .host_id = drifted.clone();
        self.table
            .events
            .iter_mut()
            .rev()
            .find(|event| &event.part_id == part_id)
            .expect("test Part has retained presence event")
            .host_id = drifted;
    }

    #[cfg(test)]
    pub(crate) fn exhaust_return_workers_for_test(&mut self, credential: &MembershipCredential) {
        while self.workers.len() < conduit_body::MAX_BODY_PARTS {
            let (_sender, receiver) = mpsc::channel();
            let (outbound, _outbound_receiver) = mpsc::sync_channel(1);
            self.workers.push(PresenceWorker {
                credential: credential.clone(),
                session_id: LinkBindingId::from(format!(
                    "patchbay/test/exhausted-worker/{}",
                    self.workers.len()
                )),
                receiver,
                outbound,
            });
        }
    }

    #[cfg(test)]
    pub(crate) fn exhaust_return_session_for_test(&mut self) {
        self.session_sequence = u64::MAX;
    }

    #[cfg(test)]
    pub(crate) fn exhaust_return_sign_for_test(&mut self) {
        self.sign_sequence = u64::MAX;
    }

    #[cfg(test)]
    pub(crate) fn atomic_state_for_test(&self) -> (HostPresenceTable, usize, u64, u64) {
        (
            self.table.clone(),
            self.workers.len(),
            self.session_sequence,
            self.sign_sequence,
        )
    }
}
