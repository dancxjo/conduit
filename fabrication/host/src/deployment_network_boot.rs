//! Finite HTTP Boot carrier for one already-built Body-bound artifact.

use crate::{
    BodyBoundArtifactIdentity, CarrierTerminal, DeploymentCarrierDescriptor, DeploymentCarrierKind,
    DeploymentCarrierRefusal, DeploymentCarrierRequest, DeploymentRealizationReceipt,
    DEPLOYMENT_RECEIPT_SCHEMA,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    thread,
    time::{Duration, Instant},
};

pub const HTTP_BOOT_IMPLEMENTATION: &str = "conduit/http-boot@1";
const MAXIMUM_REQUEST_BYTES: usize = 4096;
const COPY_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkBootServeBounds {
    pub maximum_requests: u16,
    pub timeout: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkBootServeOutcome {
    pub completed_requests: u16,
    pub bytes_per_request: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkBootRefusal {
    Carrier(DeploymentCarrierRefusal),
    UnsupportedCarrier,
    InvalidBounds,
    SourceUnavailable,
    ExtentMismatch,
    ContentMismatch,
    BindUnavailable,
    ServeFailed,
    Incomplete,
}

impl From<DeploymentCarrierRefusal> for NetworkBootRefusal {
    fn from(value: DeploymentCarrierRefusal) -> Self {
        Self::Carrier(value)
    }
}

pub trait NetworkBootServer {
    fn serve(
        &mut self,
        source: &Path,
        route: &str,
        artifact_bytes: u64,
        bounds: NetworkBootServeBounds,
    ) -> Result<NetworkBootServeOutcome, NetworkBootRefusal>;
}

pub struct HttpBootServer {
    bind: SocketAddr,
}

impl HttpBootServer {
    pub const fn new(bind: SocketAddr) -> Self {
        Self { bind }
    }
}

pub fn serve_body_bound_network_boot(
    descriptor: &DeploymentCarrierDescriptor,
    artifact: &BodyBoundArtifactIdentity,
    source: &Path,
    explicit_authority: bool,
    bounds: NetworkBootServeBounds,
    server: &mut impl NetworkBootServer,
) -> Result<DeploymentRealizationReceipt, NetworkBootRefusal> {
    if descriptor.kind != DeploymentCarrierKind::NetworkBootServe
        || descriptor.implementation_id != HTTP_BOOT_IMPLEMENTATION
    {
        return Err(NetworkBootRefusal::UnsupportedCarrier);
    }
    if bounds.maximum_requests == 0
        || bounds.maximum_requests > 64
        || bounds.timeout.is_zero()
        || bounds.timeout > Duration::from_secs(3600)
    {
        return Err(NetworkBootRefusal::InvalidBounds);
    }
    descriptor.validate_request(&DeploymentCarrierRequest {
        carrier_id: &descriptor.carrier_id,
        target_id: &descriptor.target_id,
        artifact,
        explicit_authority,
        destructive_destination: None,
        confirmed_destination: None,
        destination_is_removable: None,
    })?;
    verify_source(source, artifact)?;
    let route = format!(
        "/conduit-boot/{}.img",
        &artifact.artifact_content_sha256[7..]
    );
    let outcome = server.serve(source, &route, artifact.artifact_bytes, bounds)?;
    if outcome.completed_requests == 0
        || outcome.completed_requests > bounds.maximum_requests
        || outcome.bytes_per_request != artifact.artifact_bytes
    {
        return Err(NetworkBootRefusal::Incomplete);
    }
    let receipt = DeploymentRealizationReceipt {
        schema: DEPLOYMENT_RECEIPT_SCHEMA.into(),
        carrier_id: descriptor.carrier_id.clone(),
        implementation_id: descriptor.implementation_id.clone(),
        target_id: descriptor.target_id.clone(),
        spore_id: artifact.spore_id.clone(),
        artifact_content_sha256: artifact.artifact_content_sha256.clone(),
        terminal: CarrierTerminal::Completed,
        bytes_realized: artifact.artifact_bytes,
        byte_verification_completed: true,
        explicit_authority_observed: true,
        boot_observed: false,
        join_observed: false,
        membership_admitted: false,
    };
    descriptor.validate_receipt(artifact, &receipt)?;
    Ok(receipt)
}

impl NetworkBootServer for HttpBootServer {
    fn serve(
        &mut self,
        source: &Path,
        route: &str,
        artifact_bytes: u64,
        bounds: NetworkBootServeBounds,
    ) -> Result<NetworkBootServeOutcome, NetworkBootRefusal> {
        let listener =
            TcpListener::bind(self.bind).map_err(|_| NetworkBootRefusal::BindUnavailable)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| NetworkBootRefusal::BindUnavailable)?;
        let deadline = Instant::now() + bounds.timeout;
        let mut completed = 0_u16;
        while completed < bounds.maximum_requests && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .map_err(|_| NetworkBootRefusal::ServeFailed)?;
                    if serve_request(&mut stream, source, route, artifact_bytes)? {
                        completed += 1;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return Err(NetworkBootRefusal::ServeFailed),
            }
        }
        if completed == 0 {
            return Err(NetworkBootRefusal::Incomplete);
        }
        Ok(NetworkBootServeOutcome {
            completed_requests: completed,
            bytes_per_request: artifact_bytes,
        })
    }
}

fn serve_request(
    stream: &mut TcpStream,
    source: &Path,
    route: &str,
    artifact_bytes: u64,
) -> Result<bool, NetworkBootRefusal> {
    let mut request = [0_u8; MAXIMUM_REQUEST_BYTES];
    let mut count = 0;
    while count < request.len()
        && !request[..count]
            .windows(4)
            .any(|bytes| bytes == b"\r\n\r\n")
    {
        let received = stream
            .read(&mut request[count..])
            .map_err(|_| NetworkBootRefusal::ServeFailed)?;
        if received == 0 {
            break;
        }
        count += received;
    }
    let expected = format!("GET {route} HTTP/");
    let complete = request[..count]
        .windows(4)
        .any(|bytes| bytes == b"\r\n\r\n");
    if !complete || !request[..count].starts_with(expected.as_bytes()) {
        stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .map_err(|_| NetworkBootRefusal::ServeFailed)?;
        return Ok(false);
    }
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {artifact_bytes}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|_| NetworkBootRefusal::ServeFailed)?;
    let mut file = File::open(source).map_err(|_| NetworkBootRefusal::SourceUnavailable)?;
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| NetworkBootRefusal::SourceUnavailable)?;
        if count == 0 {
            break;
        }
        stream
            .write_all(&buffer[..count])
            .map_err(|_| NetworkBootRefusal::ServeFailed)?;
    }
    stream
        .flush()
        .map_err(|_| NetworkBootRefusal::ServeFailed)?;
    Ok(true)
}

fn verify_source(
    source: &Path,
    artifact: &BodyBoundArtifactIdentity,
) -> Result<(), NetworkBootRefusal> {
    let mut file = File::open(source).map_err(|_| NetworkBootRefusal::SourceUnavailable)?;
    if file
        .metadata()
        .map_err(|_| NetworkBootRefusal::SourceUnavailable)?
        .len()
        != artifact.artifact_bytes
    {
        return Err(NetworkBootRefusal::ExtentMismatch);
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| NetworkBootRefusal::SourceUnavailable)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if format!("sha256:{:x}", digest.finalize()) != artifact.artifact_content_sha256 {
        return Err(NetworkBootRefusal::ContentMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::net::TcpStream;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct FakeServer {
        calls: u8,
    }

    impl NetworkBootServer for FakeServer {
        fn serve(
            &mut self,
            _: &Path,
            route: &str,
            bytes: u64,
            _: NetworkBootServeBounds,
        ) -> Result<NetworkBootServeOutcome, NetworkBootRefusal> {
            assert!(route.starts_with("/conduit-boot/"));
            self.calls += 1;
            Ok(NetworkBootServeOutcome {
                completed_requests: 1,
                bytes_per_request: bytes,
            })
        }
    }

    fn fixture() -> (
        DeploymentCarrierDescriptor,
        BodyBoundArtifactIdentity,
        std::path::PathBuf,
    ) {
        let bytes = b"exact body-bound boot image";
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        let path = std::env::temp_dir().join(format!(
            "conduit-http-boot-{}-{}",
            std::process::id(),
            FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        (
            DeploymentCarrierDescriptor {
                schema: crate::DEPLOYMENT_CARRIER_SCHEMA.into(),
                carrier_id: "conduit-carrier/http-boot@1".into(),
                target_id: "conduitos/x86_64/pc".into(),
                kind: DeploymentCarrierKind::NetworkBootServe,
                implementation_id: HTTP_BOOT_IMPLEMENTATION.into(),
                maximum_artifact_bytes: 4096,
                requires_explicit_authority: true,
                verifies_written_bytes: false,
            },
            BodyBoundArtifactIdentity {
                target_id: "conduitos/x86_64/pc".into(),
                image_id: "image/reviewed".into(),
                image_content_sha256: digest.clone(),
                spore_id: "spore/body/host".into(),
                artifact_content_sha256: digest,
                artifact_bytes: bytes.len() as u64,
            },
            path,
        )
    }

    #[test]
    fn exact_spore_is_served_without_claiming_guest_boot() {
        let (descriptor, artifact, path) = fixture();
        let mut server = FakeServer { calls: 0 };
        let receipt = serve_body_bound_network_boot(
            &descriptor,
            &artifact,
            &path,
            true,
            NetworkBootServeBounds {
                maximum_requests: 1,
                timeout: Duration::from_secs(1),
            },
            &mut server,
        )
        .unwrap();
        assert_eq!(server.calls, 1);
        assert_eq!(receipt.terminal, CarrierTerminal::Completed);
        assert!(!receipt.boot_observed && !receipt.join_observed && !receipt.membership_admitted);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn stale_bytes_and_absent_authority_refuse_before_serving() {
        let (descriptor, mut artifact, path) = fixture();
        let mut server = FakeServer { calls: 0 };
        assert_eq!(
            serve_body_bound_network_boot(
                &descriptor,
                &artifact,
                &path,
                false,
                NetworkBootServeBounds {
                    maximum_requests: 1,
                    timeout: Duration::from_secs(1),
                },
                &mut server
            ),
            Err(NetworkBootRefusal::Carrier(
                DeploymentCarrierRefusal::AuthorityRequired
            ))
        );
        artifact.artifact_content_sha256 = format!("sha256:{}", "0".repeat(64));
        assert_eq!(
            serve_body_bound_network_boot(
                &descriptor,
                &artifact,
                &path,
                true,
                NetworkBootServeBounds {
                    maximum_requests: 1,
                    timeout: Duration::from_secs(1),
                },
                &mut server
            ),
            Err(NetworkBootRefusal::ContentMismatch)
        );
        assert_eq!(server.calls, 0);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn installed_http_boot_server_serves_the_exact_digest_route() {
        let (_, artifact, path) = fixture();
        let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reservation.local_addr().unwrap();
        drop(reservation);
        let route = format!(
            "/conduit-boot/{}.img",
            &artifact.artifact_content_sha256[7..]
        );
        let server_path = path.clone();
        let server_route = route.clone();
        let artifact_bytes = artifact.artifact_bytes;
        let handle = std::thread::spawn(move || {
            HttpBootServer::new(address).serve(
                &server_path,
                &server_route,
                artifact_bytes,
                NetworkBootServeBounds {
                    maximum_requests: 1,
                    timeout: Duration::from_secs(2),
                },
            )
        });
        let mut stream = (0..100)
            .find_map(|_| match TcpStream::connect(address) {
                Ok(stream) => Some(stream),
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(10));
                    None
                }
            })
            .expect("HTTP Boot listener becomes reachable");
        write!(stream, "GET {route} HTTP/1.1\r\nHost:").unwrap();
        std::thread::sleep(Duration::from_millis(20));
        write!(stream, " conduit\r\n\r\n").unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(response.ends_with(b"exact body-bound boot image"));
        assert_eq!(handle.join().unwrap().unwrap().completed_requests, 1);
        fs::remove_file(path).unwrap();
    }
}
