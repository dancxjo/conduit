//! Exact Plan-scoped distributed-Lenia worker session above ESP GATT.

use conduit_bluetooth::{
    BleGattProfile, BleReassembler, MAXIMUM_BLE_GATT_PACKET_BYTES, encode_fragment, fragment_count,
};
use conduit_core::{
    DistributedLeniaWorker, LENIA_LINE_FRAME_MAX_BYTES, LeniaLineFrameIdentity, LeniaLineFrameView,
    LeniaWorkerAdmission,
};
use heapless::Vec;

// The widest reviewed region produces two Line frames which fragment into at
// most eighteen packets at the admitted 185-byte ATT MTU.
const MAXIMUM_REPLY_PACKETS: usize = 18;
type ReplyPacket = Vec<u8, MAXIMUM_BLE_GATT_PACKET_BYTES>;
pub type ReplyPackets = Vec<ReplyPacket, MAXIMUM_REPLY_PACKETS>;

pub struct ConduitLeniaSession<'worker> {
    boot_id: alloc::string::String,
    reassembler: BleReassembler,
    send_sequence: u8,
    session_id: Option<[u8; 16]>,
    worker: &'worker mut DistributedLeniaWorker,
}

impl<'worker> ConduitLeniaSession<'worker> {
    pub fn new(
        boot: &crate::receipts::BootIdentity,
        worker: &'worker mut DistributedLeniaWorker,
    ) -> Result<Self, &'static str> {
        Ok(Self {
            boot_id: boot.boot_id(),
            reassembler: BleReassembler::new(BleGattProfile::FIRST),
            send_sequence: 0,
            session_id: None,
            worker,
        })
    }

    pub fn next_sequence(&self) -> u64 {
        u64::from(self.send_sequence)
    }

    pub fn admit_packet(&mut self, packet: &[u8]) -> Result<ReplyPackets, &'static str> {
        let Some(bytes) = self
            .reassembler
            .admit(packet)
            .map_err(|_| "invalid-fragment")?
        else {
            return Ok(ReplyPackets::new());
        };
        let mut frame_bytes = [0; LENIA_LINE_FRAME_MAX_BYTES];
        let frame_len = bytes.len();
        frame_bytes[..frame_len].copy_from_slice(bytes);
        let frame = LeniaLineFrameView::decode(&frame_bytes[..frame_len])
            .map_err(|_| "invalid-lenia-frame")?;
        self.validate_work(&frame)?;
        match self.worker.admit(frame.chunk).map_err(|_| "work-refused")? {
            LeniaWorkerAdmission::Progress { admitted_cells } => {
                esp_println::println!("CONDUIT_LENIA_BOUNDARY_ADMITTED cells={}", admitted_cells);
                Ok(ReplyPackets::new())
            }
            LeniaWorkerAdmission::ResultReady => {
                esp_println::println!("CONDUIT_LENIA_RESULT_READY");
                self.encode_results()
            }
        }
    }

    fn validate_work(&mut self, frame: &LeniaLineFrameView<'_>) -> Result<(), &'static str> {
        let id = frame.identity;
        if id.plan_id != crate::generated::PLAN_ID
            || id.play_id != crate::generated::LENIA_WORK_PLAY_ID
            || id.line_id != crate::generated::LENIA_WORK_LINE_ID
            || id.source_host_id != crate::generated::LENIA_WORK_SOURCE_HOST_ID
            || id.source_boot_id != crate::generated::LENIA_WORK_SOURCE_BOOT_ID
            || id.sink_host_id != crate::generated::LENIA_WORK_SINK_HOST_ID
            || id.sink_boot_id != self.boot_id
        {
            return Err("wrong-line-identity");
        }
        match self.session_id {
            Some(expected) if expected != id.session_id => Err("wrong-session"),
            None => {
                self.session_id = Some(id.session_id);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn encode_results(&mut self) -> Result<ReplyPackets, &'static str> {
        let identity = self
            .worker
            .result_identity()
            .map_err(|_| "result-incomplete")?;
        let session_id = self.session_id.ok_or("missing-session")?;
        let mut replies = ReplyPackets::new();
        let mut offset = 0;
        let mut chunk = [0; conduit_core::LENIA_REGION_CHUNK_MAX_BYTES];
        let mut frame = [0; LENIA_LINE_FRAME_MAX_BYTES];
        while offset < identity.total_cells {
            let chunk_len = self
                .worker
                .encode_result_chunk(offset, &mut chunk)
                .map_err(|_| "result-encode")?;
            let frame_len = LeniaLineFrameIdentity {
                plan_id: crate::generated::PLAN_ID,
                play_id: crate::generated::LENIA_RESULT_PLAY_ID,
                line_id: crate::generated::LENIA_RESULT_LINE_ID,
                source_host_id: crate::generated::HOST_ID,
                source_boot_id: &self.boot_id,
                sink_host_id: crate::generated::LENIA_RESULT_SINK_HOST_ID,
                sink_boot_id: crate::generated::LENIA_RESULT_SINK_BOOT_ID,
                session_id,
            }
            .encode(&chunk[..chunk_len], &mut frame)
            .map_err(|_| "line-encode")?;
            self.push_frame(&frame[..frame_len], &mut replies)?;
            let view = conduit_core::LeniaRegionChunkView::decode(&chunk[..chunk_len])
                .map_err(|_| "result-decode")?;
            offset += u32::from(view.header.cell_count);
        }
        Ok(replies)
    }

    fn push_frame(&mut self, frame: &[u8], replies: &mut ReplyPackets) -> Result<(), &'static str> {
        let count =
            fragment_count(frame.len(), BleGattProfile::FIRST).map_err(|_| "fragment-count")?;
        if replies.len() + usize::from(count) > replies.capacity() {
            return Err("reply-pressure");
        }
        for index in 0..count {
            let mut bytes = [0; MAXIMUM_BLE_GATT_PACKET_BYTES];
            let length = encode_fragment(
                frame,
                self.send_sequence,
                index,
                BleGattProfile::FIRST,
                &mut bytes,
            )
            .map_err(|_| "fragment-encode")?;
            let mut packet = ReplyPacket::new();
            packet
                .extend_from_slice(&bytes[..length])
                .map_err(|_| "reply-pressure")?;
            replies.push(packet).map_err(|_| "reply-pressure")?;
        }
        self.send_sequence = self.send_sequence.wrapping_add(1);
        Ok(())
    }
}
