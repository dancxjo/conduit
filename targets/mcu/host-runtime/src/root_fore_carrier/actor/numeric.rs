//! Original kernel schema-8 numeric fixture, with authenticated Body identities.
use conduit_core as a;
use conduit_kernel::assigned::*;
use std::{vec, vec::Vec};
type Tables = AssignedKernelTables<1, 2, 1, 1, 1, 1>;
pub struct TablesFixture {
    pub bytes: Vec<u8>,
    pub records: Vec<(u8, Vec<u8>)>,
    pub pins: Vec<a::AssignedRecordRequirement>,
    pub counts: [u8; 13],
    pub local: Vec<a::AssignedForeRequirement>,
    pub lines: Vec<a::AssignedLineForeRequirement>,
    pub bindings: Vec<a::AssignedRemoteBinding>,
}
impl TablesFixture {
    pub fn new(selected: [bool; 2]) -> Self {
        let host = a::AssignedIdentity::from_text("host/1");
        let boot = a::AssignedIdentity::from_text("boot/1");
        let binding = a::AssignedRemoteBinding {
            line: a::AssignedIdentity::from_text("line"),
            local_host: host,
            local_boot: boot,
            peer_host: a::AssignedIdentity::from_text("peer"),
            peer_boot: a::AssignedIdentity::from_text("peer-boot"),
        };
        let fore = [
            a::AssignedForeRequirement::new(
                0,
                0,
                a::AssignedForeDirection::Input,
                a::AssignedIdentity::from_text("input"),
                [1; 32],
            ),
            a::AssignedForeRequirement::new(
                1,
                1,
                a::AssignedForeDirection::Output,
                a::AssignedIdentity::from_text("output"),
                [2; 32],
            ),
        ];
        let mut node = vec![0; 52];
        node[2..4].copy_from_slice(&8u16.to_le_bytes());
        let mut records = vec![(a::ASSIGNED_NODE, node)];
        for i in 0..2u16 {
            let mut cord = vec![0; 42];
            cord[..2].copy_from_slice(&i.to_le_bytes());
            cord[28..30].copy_from_slice(&i.to_le_bytes());
            cord[30..32].copy_from_slice(&1u16.to_le_bytes());
            cord[32..36].copy_from_slice(&4u32.to_le_bytes());
            cord[37..41].copy_from_slice(&4u32.to_le_bytes());
            let offset = if i == 0 { 18 } else { 23 };
            cord[offset] = 1;
            cord[offset + 1..offset + 3].copy_from_slice(&i.to_le_bytes());
            records.push((a::ASSIGNED_CORD, cord));
        }
        records.push((a::ASSIGNED_ROUTE, vec![0, 0, 0, 0, 0, 0, 1, 0]));
        records.push((a::ASSIGNED_ROUTE_TARGET, vec![1, 0, 1, 1, 0, 0, 0]));
        let mut local = Vec::new();
        let mut lines = Vec::new();
        for (i, fore) in fore.into_iter().enumerate() {
            let payload = if selected[i] {
                let line = a::AssignedLineForeRequirement::new(
                    fore,
                    binding,
                    a::AssignedIdentity::from_text("session"),
                    a::AssignedIdentity::from_text("session-profile"),
                    [3; 32],
                );
                let payload = line.payload().to_vec();
                lines.push(line);
                payload
            } else {
                local.push(fore);
                a::assigned_line_fore_local_payload(&fore).to_vec()
            };
            records.push((a::ASSIGNED_REMOTE_ENDPOINT, payload));
        }
        records.push((a::ASSIGNED_STARTUP, vec![1, 0, 0]));
        let mut result = Self {
            bytes: Vec::new(),
            records,
            pins: Vec::new(),
            counts: [0; 13],
            local,
            lines,
            bindings: if selected.iter().any(|line| *line) {
                vec![binding]
            } else {
                vec![]
            },
        };
        result.reseal();
        result
    }
    pub fn reseal(&mut self) {
        self.bytes = vec![0; 125];
        self.bytes[..8].copy_from_slice(&a::assigned_plan_magic());
        self.bytes[8..10].copy_from_slice(&8u16.to_le_bytes());
        self.bytes[12..14].copy_from_slice(&128u16.to_le_bytes());
        self.bytes[16..32].copy_from_slice(&a::AssignedIdentity::from_text("plot-plan").0);
        self.bytes[32..48].copy_from_slice(&a::AssignedIdentity::from_text("fragment").0);
        self.bytes[48..64].copy_from_slice(&a::AssignedIdentity::from_text("host/1").0);
        self.bytes[64..80].copy_from_slice(&a::AssignedIdentity::from_text("boot/1").0);
        self.counts = [0; 13];
        self.pins.clear();
        for (tag, payload) in &self.records {
            self.counts[usize::from(*tag - 1)] += 1;
            self.pins
                .push(a::AssignedRecordRequirement::from_kernel_record(
                    a::AssignedRecord { tag: *tag, payload },
                    8,
                ));
            self.bytes.push(*tag);
            self.bytes
                .extend_from_slice(&(payload.len() as u16).to_le_bytes());
            self.bytes.extend_from_slice(payload);
        }
        let len = self.bytes.len() as u16;
        self.bytes[10..12].copy_from_slice(&len.to_le_bytes());
        self.bytes[80..93].copy_from_slice(&self.counts);
        let digest = a::assigned_plan_payload_digest(&self.bytes[125..]);
        self.bytes[93..125].copy_from_slice(&digest);
    }
    pub fn required(&self) -> a::AssignedPlanRequirements<'_> {
        a::AssignedPlanRequirements {
            host: a::AssignedIdentity::from_text("host/1"),
            boot: a::AssignedIdentity::from_text("boot/1"),
            host_calls: &[],
            resources: &[],
            remote_bindings: &self.bindings,
        }
    }
    pub fn computations(&self) -> a::AssignedComputationProfile<'static> {
        a::AssignedComputationProfile {
            maximum_text_bytes: 0,
            maximum_total_text_bytes: 0,
            requirements: &[],
        }
    }
    pub fn layout(&self) -> ResidentLayout<'_> {
        ResidentLayout {
            records: &self.pins,
            counts: self.counts,
        }
    }
    pub fn profile(&self) -> a::AssignedLineForeProfile<'_> {
        a::AssignedLineForeProfile {
            local: &self.local,
            lines: &self.lines,
        }
    }
    pub fn tables(&self) -> Result<Tables, AssignedKernelRefusal> {
        Tables::prepare_with_line_fore(
            &self.bytes,
            a::AssignedPlanMaxima::TINY_HOST,
            self.required(),
            self.computations(),
            &self.layout(),
            self.profile(),
        )
    }
}
impl TablesFixture {
    pub fn preparation(&self) -> conduit_body::AssignedBodyLineForePreparationProfile<'_> {
        conduit_body::AssignedBodyLineForePreparationProfile {
            preparation: conduit_body::AssignedBodyPreparationProfile {
                maxima: a::AssignedPlanMaxima::TINY_HOST,
                requirements: self.required(),
                computations: self.computations(),
                fore: None,
            },
            line_fore: self.profile(),
        }
    }
}
