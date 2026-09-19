//! Freestanding proof that the selected QEMU Image drives a real VirtIO-net
//! device and exchanges an Ethernet frame with the user-network gateway.

use core::fmt::Write;

use crate::{arch, boot::BootRecord, identity::BootIdentities, sign_format::FixedText};

const GATEWAY_IP: [u8; 4] = [10, 0, 2, 2];
const GUEST_IP: [u8; 4] = [10, 0, 2, 15];
const MAXIMUM_RECEIVE_POLLS: u32 = 1_000_000;
const MAXIMUM_TRANSMIT_POLLS: u32 = 1_000_000;

pub fn run(record: &BootRecord, identities: BootIdentities) -> ! {
    arch::initialize_machine(record, crate::boot::executable_physical_address);
    let mut device = match arch::initialize_virtio_net(
        identities.boot,
        1,
        crate::boot::executable_physical_address,
    ) {
        Ok(device) => device,
        Err(error) => refuse(error.as_str()),
    };
    let identity = device.identity();
    let request = arp_request(identity.mac);
    if let Err(error) = device.send(&request, MAXIMUM_TRANSMIT_POLLS) {
        refuse(error.as_str());
    }

    let mut frame = [0; 1514];
    for poll in 1..=MAXIMUM_RECEIVE_POLLS {
        match device.receive(&mut frame) {
            Ok(len) if arp_reply(&frame[..len], identity.mac) => {
                let mut sign = FixedText::new();
                if writeln!(
                    sign,
                    "CONDUIT_VIRTIO_NET_SIGN {{\"schema\":\"conduit.conduitos/virtio-net-proof@1\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"device\":\"virtio-net-pci-transitional\",\"bdf\":\"{:02x}:{:02x}.{}\",\"mac\":\"{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\",\"provider_generation\":{},\"queue_entries\":256,\"queue_dma_bytes\":24576,\"frame_buffer_bytes\":4096,\"static_dma_bytes\":32768,\"tx_frames\":1,\"rx_frames\":1,\"receive_polls\":{},\"gateway_ip\":\"10.0.2.2\",\"guest_ip\":\"10.0.2.15\",\"tcp_claimed\":false,\"bounded\":true}}",
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
                    poll,
                )
                .is_err()
                {
                    refuse("virtio-net-proof-sign-storage-full");
                }
                arch::early_write(sign.as_bytes());
                arch::deterministic_exit(true);
            }
            Ok(_) | Err(arch::VirtioNetError::Pressure) => {}
            Err(error) => refuse(error.as_str()),
        }
    }
    refuse("virtio-net-proof-receive-timeout")
}

fn arp_request(mac: [u8; 6]) -> [u8; 42] {
    let mut frame = [0; 42];
    frame[..6].fill(0xff);
    frame[6..12].copy_from_slice(&mac);
    frame[12..14].copy_from_slice(&[0x08, 0x06]);
    frame[14..22].copy_from_slice(&[0x00, 0x01, 0x08, 0x00, 6, 4, 0x00, 0x01]);
    frame[22..28].copy_from_slice(&mac);
    frame[28..32].copy_from_slice(&GUEST_IP);
    frame[38..42].copy_from_slice(&GATEWAY_IP);
    frame
}

fn arp_reply(frame: &[u8], mac: [u8; 6]) -> bool {
    frame.len() >= 42
        && frame[..6] == mac
        && frame[12..14] == [0x08, 0x06]
        && frame[14..22] == [0x00, 0x01, 0x08, 0x00, 6, 4, 0x00, 0x02]
        && frame[28..32] == GATEWAY_IP
        && frame[32..38] == mac
        && frame[38..42] == GUEST_IP
}

fn refuse(reason: &str) -> ! {
    arch::early_write(b"CONDUIT_VIRTIO_NET_REFUSAL ");
    arch::early_write(reason.as_bytes());
    arch::early_write(b"\n");
    arch::deterministic_exit(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_and_reply_match_only_the_exact_gateway_exchange() {
        let mac = [0x52, 0x54, 0, 0x12, 0x34, 0x56];
        let request = arp_request(mac);
        assert_eq!(&request[..6], &[0xff; 6]);
        assert_eq!(&request[6..12], &mac);
        assert_eq!(&request[28..32], &GUEST_IP);
        assert_eq!(&request[38..42], &GATEWAY_IP);

        let mut reply = request;
        reply[..6].copy_from_slice(&mac);
        reply[20..22].copy_from_slice(&[0, 2]);
        reply[28..32].copy_from_slice(&GATEWAY_IP);
        reply[32..38].copy_from_slice(&mac);
        reply[38..42].copy_from_slice(&GUEST_IP);
        assert!(arp_reply(&reply, mac));
        reply[41] = 16;
        assert!(!arp_reply(&reply, mac));
    }
}
