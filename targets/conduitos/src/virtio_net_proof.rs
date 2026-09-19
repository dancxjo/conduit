//! Freestanding proof that the selected QEMU Image drives a real VirtIO-net
//! device and completes one fixed-storage IPv4/TCP exchange.

use core::fmt::Write;

use crate::{
    arch,
    boot::BootRecord,
    cryptographic_entropy::CryptographicEntropyBase,
    identity::BootIdentities,
    sign_format::FixedText,
    virtio_tcp::VirtioTcpEndpoint,
    virtio_tls,
    virtio_tls_fixture::{PINNED_CERTIFICATE_DER, SERVER_NAME},
};

const REQUEST: &[u8] = b"CONDUIT TCP PING\n";
const RESPONSE: &[u8] = b"CONDUIT TCP PONG\n";
const MAXIMUM_POLLS: u32 = 1_000_000;

pub fn run(record: &BootRecord, identities: BootIdentities) -> ! {
    arch::initialize_machine(record, crate::boot::executable_physical_address);
    let device = match arch::initialize_virtio_net(
        identities.boot,
        1,
        crate::boot::executable_physical_address,
    ) {
        Ok(device) => device,
        Err(error) => refuse(error.as_str()),
    };
    let identity = device.identity();
    let source = match arch::RdrandEntropy::detect(1) {
        Ok(source) => source,
        Err(error) => refuse(error.as_str()),
    };
    let mut entropy = match CryptographicEntropyBase::<_, 2>::admit(source) {
        Ok(entropy) => entropy,
        Err(error) => refuse(error.as_str()),
    };
    let (tcp_seed, tls_seed, entropy_receipt) =
        match entropy.with_secret::<40, _>(|secret, receipt| {
            let mut tcp_bytes = [0; 8];
            tcp_bytes.copy_from_slice(&secret[..8]);
            let tcp_seed = u64::from_le_bytes(tcp_bytes);
            let mut tls_seed = [0; 32];
            tls_seed.copy_from_slice(&secret[8..40]);
            (tcp_seed, tls_seed, receipt)
        }) {
            Ok(value) => value,
            Err(error) => refuse(error.as_str()),
        };
    let (websocket_seed, websocket_entropy_receipt) =
        match entropy.with_secret::<32, _>(|secret, receipt| (*secret, receipt)) {
            Ok(value) => value,
            Err(error) => refuse(error.as_str()),
        };
    let mut response = [0; RESPONSE.len()];
    let receipt = match virtio_tls::exchange(
        device,
        tcp_seed,
        tls_seed,
        websocket_seed,
        VirtioTcpEndpoint {
            guest_address: [10, 0, 2, 15],
            prefix_length: 24,
            gateway: [10, 0, 2, 2],
            remote_address: [10, 0, 2, 100],
            remote_port: 9000,
            local_port: 49152,
        },
        SERVER_NAME,
        PINNED_CERTIFICATE_DER,
        REQUEST,
        &mut response,
        RESPONSE.len(),
        MAXIMUM_POLLS,
    ) {
        Ok(receipt) if response == RESPONSE => receipt,
        Ok(_) => refuse("virtio-tcp-response-mismatch"),
        Err(error) => refuse(error.as_str()),
    };

    let mut sign = FixedText::new();
    if writeln!(
        sign,
        "CONDUIT_VIRTIO_NET_SIGN {{\"schema\":\"conduit.conduitos/virtio-websocket-proof@1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"device\":\"virtio-net-pci-transitional\",\"bdf\":\"{:02x}:{:02x}.{}\",\"mac\":\"{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\",\"provider_generation\":{},\"entropy_provider_generation\":{},\"entropy_requests\":{},\"entropy_bytes\":72,\"queue_entries\":256,\"queue_dma_bytes\":24576,\"frame_buffer_bytes\":4096,\"static_dma_bytes\":32768,\"tcp_rx_bytes\":4096,\"tcp_tx_bytes\":4096,\"tls_record_rx_bytes\":4096,\"tls_record_tx_bytes\":4096,\"websocket_handshake_bytes\":1024,\"websocket_frame_bytes\":4096,\"tcp_polls\":{},\"plaintext_transmitted_bytes\":{},\"plaintext_received_bytes\":{},\"remote_ip\":\"10.0.2.100\",\"remote_port\":9000,\"server_name\":\"relay.conduit.invalid\",\"websocket_path\":\"/conduit\",\"certificate_sha256\":\"b58b58d2cfc273d464dd6dfaa5eacc8d5b0b404b236839af0360f78caebe7648\",\"tcp_claimed\":true,\"tls_claimed\":true,\"websocket_claimed\":true,\"bounded\":true}}",
        identity.bus,
        identity.device,
        identity.function,
        identity.mac[0],
        identity.mac[1],
        identity.mac[2],
        identity.mac[3],
        identity.mac[4],
        identity.mac[5],
        identity.provider_generation,
        entropy_receipt.provider.provider_generation,
        websocket_entropy_receipt.request_index,
        receipt.tcp_polls,
        receipt.transmitted_plaintext_bytes,
        receipt.received_plaintext_bytes,
    )
    .is_err()
    {
        refuse("virtio-tcp-proof-sign-storage-full");
    }
    arch::early_write(sign.as_bytes());
    arch::deterministic_exit(true)
}

fn refuse(reason: &str) -> ! {
    arch::early_write(b"CONDUIT_VIRTIO_NET_REFUSAL ");
    arch::early_write(reason.as_bytes());
    arch::early_write(b"\n");
    arch::deterministic_exit(false)
}
