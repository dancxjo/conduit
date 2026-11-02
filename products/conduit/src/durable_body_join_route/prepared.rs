//! One explicitly initialized owner transport, assigned one original invitation.
//! This is finite TCB carrier custody, not membership or effect authority.
use super::*;
use std::path::PathBuf;
use std::sync::mpsc::{self, SyncSender};
use std::thread::JoinHandle;

pub(crate) struct PreparedAdmissionRoute {
    candidate: RendezvousCandidate,
    assignment: Option<SyncSender<(PathBuf, PortableInvitation)>>,
    worker: JoinHandle<()>,
}
impl PreparedAdmissionRoute {
    #[allow(clippy::too_many_arguments)] // Exact existing route selection inputs.
    pub(crate) fn bind(
        ttl_seconds: u64,
        bind: SocketAddr,
        public_url: &str,
        certificate: &Path,
        private_key: &Path,
        authorize_route: bool,
    ) -> Result<Self, String> {
        if !authorize_route || !(1..=600).contains(&ttl_seconds) {
            return Err(
                "prepared admission route requires authorization and a 1..600 second lifetime"
                    .into(),
            );
        }
        if !public_url.starts_with("wss://") || public_url.len() > 256 {
            return Err("prepared admission route requires one bounded wss URL".into());
        }
        let server_identity = route_server_identity(public_url)?;
        let listener = SecureWebSocketListener::bind(
            bind,
            certificate,
            private_key,
            MAXIMUM_ROUTE_FRAME_BYTES as u32,
            true,
        )
        .map_err(|e| format!("prepare Body admission route: {e:?}"))?;
        if listener
            .local_addr()
            .map_err(|e| format!("inspect prepared route: {e:?}"))?
            != bind
        {
            return Err("prepared route differs from its exact authorized bind".into());
        }
        let candidate = RendezvousCandidate {
            candidate_id: "candidate/prepared-body-owner-route".into(),
            line_family: RendezvousLineFamily::AuthenticatedTlsStream,
            reachability: public_url.into(),
            authentication: RendezvousAuthentication {
                server_identity,
                transport_binding_sha256: listener.certificate_binding_sha256(),
            },
            expires_at_millis: current_time_millis()?
                .checked_add(ttl_seconds * 1000)
                .ok_or("prepared route deadline overflow")?,
            maximum_attempts: 1,
            attempt_timeout_millis: ttl_seconds.min(30) as u32 * 1000,
        };
        let (sender, receiver) = mpsc::sync_channel::<(PathBuf, PortableInvitation)>(1);
        let expected = candidate.clone();
        let worker = std::thread::Builder::new()
            .name("conduit-admission-route".into())
            .spawn(move || {
                let wait = expected
                    .expires_at_millis
                    .saturating_sub(current_time_millis().unwrap_or(u64::MAX));
                let Ok((root, invitation)) = receiver.recv_timeout(Duration::from_millis(wait))
                else {
                    return;
                };
                let valid = invitation
                    .rendezvous
                    .as_ref()
                    .is_some_and(|r| r.candidates == [expected.clone()]);
                if !valid {
                    eprintln!("prepared route refused a different original invitation descriptor");
                    return;
                }
                if let Err(error) = serve_existing_invitation_route(&root, &listener, &invitation) {
                    eprintln!("prepared original invitation route ended: {error}");
                }
            })
            .map_err(|e| format!("start bounded prepared route: {e}"))?;
        Ok(Self {
            candidate,
            assignment: Some(sender),
            worker,
        })
    }
    pub(crate) fn is_live(&self) -> bool {
        !self.worker.is_finished()
    }
    pub(crate) fn candidate(&self) -> Result<RendezvousCandidate, String> {
        if self.assignment.is_none()
            || !self.is_live()
            || current_time_millis()? >= self.candidate.expires_at_millis
        {
            return Err("prepared route is consumed or expired".into());
        }
        Ok(self.candidate.clone())
    }
    pub(crate) fn assign(
        &mut self,
        root: &Path,
        invitation: &PortableInvitation,
    ) -> Result<(), String> {
        if invitation
            .rendezvous
            .as_ref()
            .is_none_or(|r| r.candidates != [self.candidate.clone()])
        {
            return Err("original invitation does not select the prepared route".into());
        }
        let sender = self
            .assignment
            .take()
            .ok_or("prepared route already consumed")?;
        sender
            .try_send((root.to_path_buf(), invitation.clone()))
            .map_err(|_| "prepared route expired before original invitation assignment".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, TcpListener};
    fn prepared(ttl: u64) -> (PreparedAdmissionRoute, PathBuf, SocketAddr) {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let directory = std::env::temp_dir().join(format!(
            "conduit-prepared-admission-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let certificate = directory.join("certificate.pem");
        let key = directory.join("key.pem");
        std::fs::write(&certificate, cert.pem()).unwrap();
        std::fs::write(&key, signing_key.serialize_pem()).unwrap();
        let reservation = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
        let bind = reservation.local_addr().unwrap();
        drop(reservation);
        let route = PreparedAdmissionRoute::bind(
            ttl,
            bind,
            &format!("wss://localhost:{}/body", bind.port()),
            &certificate,
            &key,
            true,
        )
        .unwrap();
        (route, directory, bind)
    }
    #[test]
    fn initialized_unassigned_listener_expires_without_any_invitation() {
        let (route, directory, bind) = prepared(1);
        assert!(TcpListener::bind(bind).is_err());
        let candidate = route.candidate().unwrap();
        assert_ne!(candidate.authentication.transport_binding_sha256, [0; 32]);
        route.worker.join().unwrap();
        assert!(TcpListener::bind(bind).is_ok());
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn dropping_unassigned_custody_releases_the_actual_listener() {
        let (route, directory, bind) = prepared(60);
        assert!(TcpListener::bind(bind).is_err());
        drop(route);
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if TcpListener::bind(bind).is_ok() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "actual listener did not close after custody drop"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
