//! Freestanding proof that the selected QEMU Image drives a real VirtIO-net
//! device and completes one fixed-storage IPv4/TCP exchange.

use core::fmt::Write;

use crate::{
    arch,
    boot::BootRecord,
    cryptographic_entropy::CryptographicEntropyBase,
    identity::BootIdentities,
    sign_format::FixedText,
    virtio_tcp::{self, VirtioTcpEndpoint},
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
    let mut entropy = match CryptographicEntropyBase::<_, 1>::admit(source) {
        Ok(entropy) => entropy,
        Err(error) => refuse(error.as_str()),
    };
    let (random_seed, entropy_receipt) = match entropy
        .with_secret::<8, _>(|secret, receipt| (u64::from_le_bytes(*secret), receipt))
    {
        Ok(value) => value,
        Err(error) => refuse(error.as_str()),
    };
    let mut response = [0; RESPONSE.len()];
    let receipt = match virtio_tcp::exchange(
        device,
        random_seed,
        VirtioTcpEndpoint {
            guest_address: [10, 0, 2, 15],
            prefix_length: 24,
            gateway: [10, 0, 2, 2],
            remote_address: [10, 0, 2, 100],
            remote_port: 9000,
            local_port: 49152,
        },
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
        "CONDUIT_VIRTIO_NET_SIGN {{\"schema\":\"conduit.conduitos/virtio-tcp-proof@1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"device\":\"virtio-net-pci-transitional\",\"bdf\":\"{:02x}:{:02x}.{}\",\"mac\":\"{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\",\"provider_generation\":{},\"entropy_provider_generation\":{},\"entropy_requests\":{},\"queue_entries\":256,\"queue_dma_bytes\":24576,\"frame_buffer_bytes\":4096,\"static_dma_bytes\":32768,\"socket_rx_bytes\":1024,\"socket_tx_bytes\":1024,\"tcp_polls\":{},\"tcp_transmitted_bytes\":{},\"tcp_received_bytes\":{},\"remote_ip\":\"10.0.2.100\",\"remote_port\":9000,\"tcp_claimed\":true,\"tls_claimed\":false,\"websocket_claimed\":false,\"bounded\":true}}",
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
        entropy_receipt.request_index,
        receipt.polls,
        receipt.transmitted_bytes,
        receipt.received_bytes,
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
