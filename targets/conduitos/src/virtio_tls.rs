//! Fixed-storage, descriptor-pinned TLS 1.3 over one admitted TCP session.

use embedded_tls::{
    Aes128GcmSha256, Certificate, CryptoProvider, MaxFragmentLength, NoClock, TlsConfig,
    TlsContext, TlsError, TlsVerifier, blocking::TlsConnection, pki::CertVerifier,
};
use rand_chacha::ChaCha20Rng;
use rand_core::{CryptoRng, RngCore, SeedableRng};
use smoltcp::iface::SocketStorage;

use crate::{
    arch::VirtioNetReady,
    bounded_websocket::{BinaryWebSocketIo, BoundedWebSocket, WebSocketError},
    virtio_tcp::{VirtioTcpEndpoint, VirtioTcpError},
    virtio_tcp_stream::VirtioTcpStream,
};

pub const TCP_BUFFER_BYTES: usize = 4096;
pub const TLS_RECORD_BUFFER_BYTES: usize = 4096;
const MAXIMUM_CERTIFICATE_BYTES: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioTlsReceipt {
    pub tcp_polls: u32,
    pub transmitted_plaintext_bytes: u16,
    pub received_plaintext_bytes: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtioTlsError {
    InvalidDescriptor,
    RequestTooLarge,
    ResponseTooLarge,
    Tcp(VirtioTcpError),
    Authentication,
    Handshake,
    Write,
    Read,
    Close,
    WebSocket(WebSocketError),
}

pub(crate) enum VirtioWebSocketRunError<E> {
    Transport(VirtioTlsError),
    Operation(E),
}

impl VirtioTlsError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidDescriptor => "virtio-tls-descriptor-invalid",
            Self::RequestTooLarge => "virtio-tls-request-too-large",
            Self::ResponseTooLarge => "virtio-tls-response-too-large",
            Self::Tcp(error) => error.as_str(),
            Self::Authentication => "virtio-tls-authentication-failed",
            Self::Handshake => "virtio-tls-handshake-failed",
            Self::Write => "virtio-tls-write-failed",
            Self::Read => "virtio-tls-read-failed",
            Self::Close => "virtio-tls-close-failed",
            Self::WebSocket(error) => error.as_str(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn exchange(
    device: VirtioNetReady,
    tcp_seed: u64,
    tls_seed: [u8; 32],
    websocket_seed: [u8; 32],
    endpoint: VirtioTcpEndpoint,
    server_name: &str,
    pinned_certificate_der: &[u8],
    request: &[u8],
    response: &mut [u8],
    expected_response_bytes: usize,
    maximum_polls: u32,
) -> Result<VirtioTlsReceipt, VirtioTlsError> {
    if request.is_empty() || request.len() > u16::MAX as usize {
        return Err(VirtioTlsError::RequestTooLarge);
    }
    if expected_response_bytes == 0
        || expected_response_bytes > response.len()
        || expected_response_bytes > u16::MAX as usize
    {
        return Err(VirtioTlsError::ResponseTooLarge);
    }

    let ((), tcp_polls) = with_websocket(
        device,
        tcp_seed,
        tls_seed,
        websocket_seed,
        endpoint,
        server_name,
        pinned_certificate_der,
        maximum_polls,
        |websocket| {
            websocket
                .send_binary(request)
                .map_err(VirtioTlsError::WebSocket)?;
            let received = websocket
                .receive_binary(response)
                .map_err(VirtioTlsError::WebSocket)?;
            if received != expected_response_bytes {
                return Err(VirtioTlsError::ResponseTooLarge);
            }
            Ok(())
        },
    )
    .map_err(|error| match error {
        VirtioWebSocketRunError::Transport(error) | VirtioWebSocketRunError::Operation(error) => {
            error
        }
    })?;
    Ok(VirtioTlsReceipt {
        tcp_polls,
        transmitted_plaintext_bytes: request.len() as u16,
        received_plaintext_bytes: expected_response_bytes as u16,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn with_websocket<T, E>(
    device: VirtioNetReady,
    tcp_seed: u64,
    tls_seed: [u8; 32],
    websocket_seed: [u8; 32],
    endpoint: VirtioTcpEndpoint,
    server_name: &str,
    pinned_certificate_der: &[u8],
    maximum_polls: u32,
    operation: impl FnOnce(&mut dyn BinaryWebSocketIo) -> Result<T, E>,
) -> Result<(T, u32), VirtioWebSocketRunError<E>> {
    if server_name.is_empty()
        || pinned_certificate_der.is_empty()
        || pinned_certificate_der.len() > MAXIMUM_CERTIFICATE_BYTES
    {
        return Err(VirtioWebSocketRunError::Transport(
            VirtioTlsError::InvalidDescriptor,
        ));
    }
    let mut tcp_receive = [0; TCP_BUFFER_BYTES];
    let mut tcp_transmit = [0; TCP_BUFFER_BYTES];
    let mut socket_storage = [SocketStorage::EMPTY];
    let stream = VirtioTcpStream::connect(
        device,
        tcp_seed,
        endpoint,
        maximum_polls,
        &mut tcp_receive,
        &mut tcp_transmit,
        &mut socket_storage,
    )
    .map_err(|error| VirtioWebSocketRunError::Transport(VirtioTlsError::Tcp(error)))?;
    let mut tls_receive = [0; TLS_RECORD_BUFFER_BYTES];
    let mut tls_transmit = [0; TLS_RECORD_BUFFER_BYTES];
    let config = TlsConfig::new()
        .with_server_name(server_name)
        .with_max_fragment_length(MaxFragmentLength::Bits11);
    let provider = PinnedProvider {
        rng: ErasedChaCha::new(tls_seed),
        verifier: CertVerifier::new(Certificate::X509(pinned_certificate_der)),
    };
    let mut tls =
        TlsConnection::<_, Aes128GcmSha256>::new(stream, &mut tls_receive, &mut tls_transmit);
    tls.open(TlsContext::new(&config, provider))
        .map_err(|error| VirtioWebSocketRunError::Transport(classify_handshake(error)))?;
    let mut websocket = BoundedWebSocket::connect(
        &mut tls,
        ErasedChaCha::new(websocket_seed),
        server_name,
        "/conduit",
    )
    .map_err(|error| VirtioWebSocketRunError::Transport(VirtioTlsError::WebSocket(error)))?;
    let result = operation(&mut websocket);
    let websocket_close = websocket.close();
    let tls_close = tls.close();
    let tcp_close = match tls_close {
        Ok(stream) => stream.close_tcp().map_err(VirtioTlsError::Tcp),
        Err(_) => Err(VirtioTlsError::Close),
    };
    let value = match result {
        Ok(value) => value,
        Err(error) => return Err(VirtioWebSocketRunError::Operation(error)),
    };
    websocket_close
        .map_err(|error| VirtioWebSocketRunError::Transport(VirtioTlsError::WebSocket(error)))?;
    let tcp_polls = tcp_close.map_err(VirtioWebSocketRunError::Transport)?;
    Ok((value, tcp_polls))
}

fn classify_handshake(error: TlsError) -> VirtioTlsError {
    match error {
        TlsError::InvalidCertificate
        | TlsError::InvalidCertificateEntry
        | TlsError::InvalidSignature
        | TlsError::InvalidSignatureScheme => VirtioTlsError::Authentication,
        TlsError::Io(embedded_io::ErrorKind::TimedOut) => {
            VirtioTlsError::Tcp(VirtioTcpError::Timeout)
        }
        _ => VirtioTlsError::Handshake,
    }
}

struct PinnedProvider<'a> {
    rng: ErasedChaCha,
    verifier: CertVerifier<'a, Aes128GcmSha256, NoClock, MAXIMUM_CERTIFICATE_BYTES>,
}

impl CryptoProvider for PinnedProvider<'_> {
    type CipherSuite = Aes128GcmSha256;
    type Signature = [u8; 0];

    fn rng(&mut self) -> impl embedded_tls::CryptoRngCore {
        &mut self.rng
    }

    fn verifier(&mut self) -> Result<&mut impl TlsVerifier<Aes128GcmSha256>, TlsError> {
        Ok(&mut self.verifier)
    }
}

struct ErasedChaCha(ChaCha20Rng);

impl ErasedChaCha {
    fn new(seed: [u8; 32]) -> Self {
        Self(ChaCha20Rng::from_seed(seed))
    }
}

impl RngCore for ErasedChaCha {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        self.0.fill_bytes(destination);
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core::Error> {
        self.0.try_fill_bytes(destination)
    }
}

impl CryptoRng for ErasedChaCha {}

impl Drop for ErasedChaCha {
    fn drop(&mut self) {
        let bytes = core::ptr::from_mut(&mut self.0).cast::<u8>();
        for offset in 0..size_of::<ChaCha20Rng>() {
            unsafe { core::ptr::write_volatile(bytes.add(offset), 0) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_errors_keep_authentication_distinct() {
        assert_eq!(
            classify_handshake(TlsError::InvalidCertificate),
            VirtioTlsError::Authentication
        );
        assert_eq!(
            classify_handshake(TlsError::InvalidRecord),
            VirtioTlsError::Handshake
        );
    }
}
