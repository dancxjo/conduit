//! Expected schema-8 structure from an independently checked installed template.
//! Fresh placement facts and Line pins are descriptive caller inputs. This
//! codec grants no membership, custody, attribution or execution authority.
use super::*;

pub struct AssignedLineLayoutRecipe<'a> {
    bytes: &'a [u8],
    fresh: &'a [FreshAssignedFore<'a>],
    profile: AssignedLineForeProfile<'a>,
}
impl AssignedLocalProjectionTemplate<'_> {
    /// The caller establishes fresh placements through canonical checked Source
    /// and selected Host/capabilities, never by copying the received assignment.
    /// Exact Fore ABI equality includes finite queue capacity. All validation
    /// precedes exposing the lazy inventory; no heap or duplicate table is used.
    pub fn line_layout<'a>(
        &'a self,
        fresh: &'a [FreshAssignedFore<'a>],
        profile: AssignedLineForeProfile<'a>,
    ) -> Result<AssignedLineLayoutRecipe<'a>, ProjectionTemplateRefusal> {
        use ProjectionTemplateRefusal as E;
        if crate::read_u16(self.bytes, 8)? != ASSIGNED_FORE_NATIVE_PLAN_SCHEMA
            || fresh.len() > 16
            || profile.local.len() + profile.lines.len() != fresh.len()
            || profile.lines.is_empty()
        {
            return Err(E::Bindings);
        }
        let recipe = AssignedLineLayoutRecipe {
            bytes: self.bytes,
            fresh,
            profile,
        };
        let mut count = 0;
        for record in AssignedPlanRecords::new(self.bytes)? {
            let record = record?;
            if external(record) {
                fore_slot(self.bytes, record.payload, fresh)?;
            }
            if record.tag == ASSIGNED_LOCAL_FORE {
                count += 1;
                if record.payload.len() != ASSIGNED_LOCAL_FORE_BYTES {
                    return Err(E::Bindings);
                }
                recipe.pin(record.payload)?;
            }
        }
        if count != fresh.len() {
            return Err(E::Bindings);
        }
        // Every supplied slot and profile row must occur once, not merely have
        // matching aggregate counts. Endpoint and Cord uniqueness are explicit.
        for (i, slot) in fresh.iter().enumerate() {
            if fresh[..i]
                .iter()
                .any(|p| p.cord == slot.cord || p.endpoint == slot.endpoint)
            {
                return Err(E::Bindings);
            }
            let mut found = 0;
            for record in AssignedPlanRecords::new(self.bytes)? {
                let record = record?;
                if external(record) && crate::read_u16(record.payload, 0)? == slot.cord {
                    found += 1;
                }
            }
            if found != 1 {
                return Err(E::Bindings);
            }
        }
        let pins = || {
            profile
                .local
                .iter()
                .chain(profile.lines.iter().map(|p| p.fore()))
        };
        for pin in pins() {
            if pins()
                .filter(|p| p.endpoint() == pin.endpoint() || p.cord() == pin.cord())
                .count()
                != 1
            {
                return Err(E::Bindings);
            }
        }
        for line in profile.lines {
            // Exact peer/session identities stay in the expected record. Empty
            // descriptive pins cannot masquerade as an independently selected
            // installation. Their authorization remains outside this codec.
            if line.payload()[54..166]
                .chunks_exact(16)
                .any(|id| id == [0; 16])
                || line.payload()[166..].iter().all(|b| *b == 0)
            {
                return Err(E::Bindings);
            }
        }
        Ok(recipe)
    }
}
impl<'a> AssignedLineLayoutRecipe<'a> {
    fn pin(
        &self,
        bytes: &[u8],
    ) -> Result<Option<&AssignedLineForeRequirement>, ProjectionTemplateRefusal> {
        let mut found = None;
        let mut count = 0;
        for local in self.profile.local {
            if local.payload().as_slice() == bytes {
                count += 1;
            }
        }
        for line in self.profile.lines {
            if line.fore().payload().as_slice() == bytes {
                count += 1;
                found = Some(line);
            }
        }
        if count != 1 {
            return Err(ProjectionTemplateRefusal::Bindings);
        }
        Ok(found)
    }
    /// Lazy expected fingerprints, preserving multiplicity and exact selected
    /// Cord identities. Incoming assignment bytes are never an input.
    pub fn records(
        &self,
    ) -> impl Iterator<Item = Result<AssignedRecordRequirement, ProjectionTemplateRefusal>> + '_
    {
        // Constructor checked the complete immutable envelope.
        AssignedPlanRecords::new(self.bytes)
            .expect("checked template")
            .filter_map(move |record| {
                let record = match record {
                    Ok(r) => r,
                    Err(e) => return Some(Err(e.into())),
                };
                if matches!(
                    record.tag,
                    ASSIGNED_CONFIGURATION | ASSIGNED_SIGN | ASSIGNED_TERMINAL
                ) {
                    return None;
                }
                Some(self.fingerprint(record))
            })
    }
    fn fingerprint(
        &self,
        record: AssignedRecord<'_>,
    ) -> Result<AssignedRecordRequirement, ProjectionTemplateRefusal> {
        if record.tag == ASSIGNED_LOCAL_FORE {
            return Ok(match self.pin(record.payload)? {
                Some(line) => AssignedRecordRequirement::from_record(AssignedRecord {
                    tag: ASSIGNED_REMOTE_ENDPOINT,
                    payload: line.payload(),
                }),
                None => {
                    let mut local = [0; ASSIGNED_LINE_FORE_LOCAL_BYTES];
                    local[1..].copy_from_slice(record.payload);
                    AssignedRecordRequirement::from_record(AssignedRecord {
                        tag: ASSIGNED_REMOTE_ENDPOINT,
                        payload: &local,
                    })
                }
            });
        }
        if external(record) {
            let pin = matching_fore(self.bytes, record.payload)?;
            let slot = fore_slot(self.bytes, record.payload, self.fresh)?;
            let mut payload = [0; 42];
            payload.copy_from_slice(record.payload);
            if self.pin(&pin)?.is_some() {
                payload[2..18].copy_from_slice(
                    &assigned_local_fore_connection(
                        slot.direction,
                        slot.front_port,
                        slot.placement,
                        slot.gear_port,
                    )
                    .0,
                );
            }
            return Ok(
                AssignedRecordRequirement::from_kernel_record_with_line_fore(
                    AssignedRecord {
                        tag: record.tag,
                        payload: &payload,
                    },
                    self.profile,
                ),
            );
        }
        Ok(AssignedRecordRequirement::from_kernel_record(
            record,
            ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA,
        ))
    }
}

/// Sparse expected-row replacement derived from checked construction facts.
#[derive(Clone, Copy)]
pub struct AssignedLayoutOverride {
    pub ordinal: u16,
    pub requirement: AssignedRecordRequirement,
}
impl AssignedLayoutOverride {
    pub const EMPTY: Self = Self {
        ordinal: u16::MAX,
        requirement: AssignedRecordRequirement {
            tag: 0,
            payload_digest: [0; 32],
        },
    };
}
impl AssignedLineLayoutRecipe<'_> {
    /// Preflight the original immutable inventory once, then retain only changed
    /// boundary rows. Repeated kernel comparisons perform no expected-row SHA.
    /// Scratch is caller-owned, at most two rows per independently pinned Fore.
    pub fn write_overrides<'s>(
        &self,
        installed: &[AssignedRecordRequirement],
        scratch: &'s mut [AssignedLayoutOverride],
    ) -> Result<&'s [AssignedLayoutOverride], ProjectionTemplateRefusal> {
        use ProjectionTemplateRefusal as E;
        if scratch.len() < self.fresh.len() * 2 {
            return Err(E::Bounds);
        }
        let mut ordinal = 0;
        for record in AssignedPlanRecords::new(self.bytes)? {
            let record = record?;
            if matches!(
                record.tag,
                ASSIGNED_CONFIGURATION | ASSIGNED_SIGN | ASSIGNED_TERMINAL
            ) {
                continue;
            }
            let original = AssignedRecordRequirement::from_kernel_record(
                record,
                ASSIGNED_FORE_NATIVE_PLAN_SCHEMA,
            );
            if installed.get(ordinal) != Some(&original) {
                return Err(E::Bindings);
            }
            ordinal += 1;
        }
        if ordinal != installed.len() || ordinal > usize::from(u16::MAX) {
            return Err(E::Bindings);
        }
        let mut ordinal = 0;
        let mut used = 0;
        for record in AssignedPlanRecords::new(self.bytes)? {
            let record = record?;
            if matches!(
                record.tag,
                ASSIGNED_CONFIGURATION | ASSIGNED_SIGN | ASSIGNED_TERMINAL
            ) {
                continue;
            }
            if record.tag == ASSIGNED_LOCAL_FORE || external(record) {
                let expected = self.fingerprint(record)?;
                if installed[ordinal] != expected {
                    scratch[used] = AssignedLayoutOverride {
                        ordinal: ordinal as u16,
                        requirement: expected,
                    };
                    used += 1;
                }
            }
            ordinal += 1;
        }
        Ok(&scratch[..used])
    }
}
