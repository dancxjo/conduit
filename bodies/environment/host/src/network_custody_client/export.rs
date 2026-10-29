//! Read-only, single-request advancement of a finite durable snapshot export.
use super::*;

pub(crate) enum Progress {
    Pending,
    Complete(VerifiedDurableSnapshot),
}
/// Possession serializes one local export, not membership or erase authority.
/// Dropping this owner spends any active lease; reopening cannot extend it.
pub(crate) struct Export {
    run: PathBuf,
    body: BodyId,
    root: [u8; 32],
    boot: BootId,
    _lock: File,
    bound: Option<[u8; OFFER]>,
    bound_id: Option<[u8; 32]>,
    spent: usize,
    deadline: Option<Instant>,
    polls: usize,
    awaiting_offer: bool,
    finished: bool,
    complete: Option<VerifiedDurableSnapshot>,
}
impl Export {
    pub(crate) fn open(run: &Path, body: &BodyId, root: [u8; 32], boot: &BootId) -> Result<Self> {
        if [body.as_str(), boot.as_str()]
            .iter()
            .any(|id| id.is_empty() || id.len() > 128 || id.chars().any(char::is_control))
        {
            return Err("network export identity extent".into());
        }
        private(run, true)?;
        let lock_path = run.join("export.lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&lock_path)?;
        private(&lock_path, false)?;
        // SAFETY: actual retained owned descriptor; File drop releases the lock.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("network export already running".into());
        }
        durable(
            &run.join("scope.json"),
            &serde_json::to_vec(&serde_json::json!({
                "body":body,"root":root,"boot":boot,"maximum_leases":LEASES
            }))?,
        )?;
        let mut spent = 0;
        for lease in 0..LEASES {
            let path = run.join(format!("lease-{lease}.bin"));
            if path.try_exists()? {
                reopen(&path, 0)?;
                if spent != lease {
                    return Err("network lease sequence differs".into());
                }
                spent += 1;
            }
        }
        let mut complete = None;
        let mut bound_id = None;
        let bound = if run.join("offer.bin").try_exists()? {
            let offer: [u8; OFFER] = reopen(&run.join("offer.bin"), OFFER)?.try_into().unwrap();
            let scope = scope(body, root, boot, &offer);
            let id = verify_offer(&scope, &offer)?;
            bound_id = Some(id);
            let completed = run.join(format!("network-{}", hex(&id)));
            if completed.try_exists()? {
                private(&completed, true)?;
            }
            if completed.join("snapshot.bin").try_exists()? {
                let bytes = reopen(&completed.join("snapshot.bin"), CAPACITY)?;
                complete = Some(VerifiedDurableSnapshot::retain(
                    run, &offer, &bytes, &scope,
                )?);
            }
            if spent == 0 {
                return Err("network export retained offer has no spent lease".into());
            }
            Some(offer)
        } else {
            None
        };
        Ok(Self {
            run: run.into(),
            body: body.clone(),
            root,
            boot: boot.clone(),
            _lock: lock,
            bound,
            bound_id,
            spent,
            deadline: None,
            polls: 0,
            awaiting_offer: true,
            finished: false,
            complete,
        })
    }
    /// Actual active lease deadline, never renewed by a tick or process restart.
    pub(crate) fn lease_deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// At most one GET. The caller must enforce `effective_deadline` in its
    /// transport; late responses are discarded even if the transport overruns.
    /// No signer, POST transport, grant, retirement or erase entrance is accepted.
    pub(crate) fn advance(
        &mut self,
        operation_deadline: Instant,
        get: impl FnMut(&str, Instant) -> Result<(u16, Vec<u8>)>,
    ) -> Result<Progress> {
        let result = self.advance_inner(operation_deadline, get);
        if result.is_err() {
            self.finished = true;
        }
        result
    }
    fn advance_inner(
        &mut self,
        operation_deadline: Instant,
        mut get: impl FnMut(&str, Instant) -> Result<(u16, Vec<u8>)>,
    ) -> Result<Progress> {
        let now = Instant::now();
        if self.finished {
            return Err("network export already terminal".into());
        }
        if let Some(token) = self.complete.take() {
            self.finished = true;
            return Ok(Progress::Complete(token));
        }
        if operation_deadline <= now {
            return Ok(Progress::Pending);
        }
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.deadline = None;
        }
        if self.deadline.is_none() {
            if self.spent == LEASES {
                return Err(
                    "network export lease bound exhausted; partials remain unverified".into(),
                );
            }
            // Spend durably BEFORE any request. No active deadline is restored.
            durable(&self.run.join(format!("lease-{}.bin", self.spent)), &[])?;
            self.spent += 1;
            self.deadline = Some(now + Duration::from_secs(150));
            self.polls = 0;
            self.awaiting_offer = true;
        }
        let lease_deadline = self.deadline.unwrap();
        // A new lease always reacquires the exact signed offer, even when chunks
        // survived a prior lease. An offer is not an extendable transport lease.
        let offer_pending = self.awaiting_offer;
        let mut offset = None;
        let path = if offer_pending {
            "/network/custody/offer".to_owned()
        } else {
            let offer = self.bound.as_ref().ok_or("network export missing offer")?;
            let id = self
                .bound_id
                .ok_or("network export missing verified identifier")?;
            let chunks = self.run.join(format!("unverified-{}", hex(&id)));
            for at in (0..CAPACITY).step_by(CHUNK) {
                if !chunks.join(format!("{at:05}.bin")).try_exists()? {
                    offset = Some(at);
                    break;
                }
            }
            if let Some(at) = offset {
                format!("/network/custody/chunk?snapshot={}&offset={at}", hex(&id))
            } else {
                let mut snapshot = Vec::with_capacity(CAPACITY);
                for at in (0..CAPACITY).step_by(CHUNK) {
                    snapshot
                        .extend_from_slice(&reopen(&chunks.join(format!("{at:05}.bin")), CHUNK)?);
                }
                let token = VerifiedDurableSnapshot::retain(
                    &self.run,
                    offer,
                    &snapshot,
                    &scope(&self.body, self.root, &self.boot, offer),
                )?;
                self.finished = true;
                return Ok(Progress::Complete(token));
            }
        };
        let deadline = operation_deadline.min(lease_deadline);
        if Instant::now() >= deadline {
            return Ok(Progress::Pending);
        }
        let response = get(&path, deadline);
        if Instant::now() >= deadline {
            if Instant::now() >= lease_deadline {
                self.deadline = None;
            }
            return Ok(Progress::Pending);
        }
        self.polls += 1;
        if response.as_ref().is_ok_and(|(_, bytes)| bytes.len() > 2048) {
            return Err("network export response extent".into());
        }
        let raw = match response {
            Ok((200, bytes)) => bytes,
            Ok((409, _)) => {
                self.deadline = None;
                return Ok(Progress::Pending);
            }
            Ok((202 | 503, _)) | Err(_) => {
                // Preserve which request is pending; 376 retries per request is
                // the original finite bound, in addition to the absolute lease.
                if self.polls >= 376 {
                    self.deadline = None;
                }
                return Ok(Progress::Pending);
            }
            Ok((status, _)) => return Err(format!("network export HTTP status {status}").into()),
        };
        if offer_pending {
            let envelope: live::Offered = serde_json::from_slice(&raw)?;
            if envelope.profile != "network-retirement-offer@1" {
                return Err("network offer profile".into());
            }
            let offer = live::unhex::<OFFER>(&envelope.offer)?;
            let id = verify_offer(&scope(&self.body, self.root, &self.boot, &offer), &offer)?;
            if live::unhex::<32>(&envelope.snapshot)? != id {
                return Err("network offer id differs".into());
            }
            if self.bound.as_ref().is_some_and(|old| old != &offer) {
                return Err("network snapshot changed; partials retained unverified".into());
            }
            durable(&self.run.join("offer.bin"), &offer)?;
            directory(&self.run.join(format!("unverified-{}", hex(&id))))?;
            self.bound = Some(offer);
            self.bound_id = Some(id);
        } else {
            let chunk: Chunk = serde_json::from_slice(&raw)?;
            let id = self
                .bound_id
                .ok_or("network export missing verified identifier")?;
            let at = offset.unwrap();
            if chunk.profile != "network-retirement-chunk@1"
                || live::unhex::<32>(&chunk.snapshot)? != id
                || chunk.offset != at
            {
                return Err("network chunk scope differs".into());
            }
            durable(
                &self
                    .run
                    .join(format!("unverified-{}", hex(&id)))
                    .join(format!("{at:05}.bin")),
                &live::unhex::<CHUNK>(&chunk.bytes)?,
            )?;
        }
        self.awaiting_offer = false;
        self.polls = 0;
        Ok(Progress::Pending)
    }
}
