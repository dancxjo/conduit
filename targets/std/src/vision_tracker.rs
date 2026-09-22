//! Finite local continuity over provenance-bearing Vision object observations.

use conduit_core::{PreparedStructuredValueValidator, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use core::fmt::Write;

mod wire;
use wire::*;
#[cfg(test)]
mod tests;

const MAXIMUM_TRACKS: usize = 4;
const MAXIMUM_HISTORY: usize = 16;
const MAXIMUM_IDENTITY_BYTES: usize = conduit_human::MAXIMUM_VISUAL_IDENTITY_BYTES;

#[derive(Clone, Copy)]
struct BoundedIdentity {
    bytes: [u8; MAXIMUM_IDENTITY_BYTES],
    len: u8,
}

impl Default for BoundedIdentity {
    fn default() -> Self {
        Self {
            bytes: [0; MAXIMUM_IDENTITY_BYTES],
            len: 0,
        }
    }
}

impl BoundedIdentity {
    fn set(&mut self, value: &[u8]) -> Result<(), ()> {
        if value.is_empty() || value.len() > self.bytes.len() {
            return Err(());
        }
        self.bytes[..value.len()].copy_from_slice(value);
        self.len = value.len() as u8;
        Ok(())
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

#[derive(Clone, Copy, Default)]
struct Region {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

#[derive(Clone, Copy)]
struct TrackState {
    active: bool,
    label: BoundedIdentity,
    region: Region,
    history: [BoundedIdentity; MAXIMUM_HISTORY],
    history_len: u8,
    id: BoundedIdentity,
}

impl Default for TrackState {
    fn default() -> Self {
        Self {
            active: false,
            label: BoundedIdentity::default(),
            region: Region::default(),
            history: [BoundedIdentity::default(); MAXIMUM_HISTORY],
            history_len: 0,
            id: BoundedIdentity::default(),
        }
    }
}

#[derive(Clone, Copy)]
struct ParsedObject<'a> {
    label: &'a [u8],
    observation_sign: &'a [u8],
    region: Region,
    source_image_node: &'a [u8],
}

pub(crate) struct LocalVisionTracker {
    validator: PreparedStructuredValueValidator,
    type_prefix: Vec<u8>,
    output_type_prefix: Vec<u8>,
    implementation: String,
    provider: String,
    artifact: String,
    context: String,
    tracks: [TrackState; MAXIMUM_TRACKS],
    sign: String,
    output: Vec<u8>,
}

impl LocalVisionTracker {
    pub(crate) fn prepare(provider: String, context: String) -> Result<Self, String> {
        validate_identity(&provider)?;
        validate_identity(&context)?;
        let input_type = conduit_semantic_catalog::vision_objects_type();
        let mut tracks = [TrackState::default(); MAXIMUM_TRACKS];
        for (index, track) in tracks.iter_mut().enumerate() {
            let id = format!("{context}/track-{index}");
            track
                .id
                .set(id.as_bytes())
                .map_err(|_| "local Vision track identity exceeds its bound".to_string())?;
        }
        Ok(Self {
            validator: PreparedStructuredValueValidator::new(
                &input_type,
                MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            )
            .map_err(|error| format!("prepare local Vision tracker input: {error:?}"))?,
            type_prefix: input_type
                .canonical_bytes()
                .map_err(|error| format!("encode local Vision tracker input type: {error:?}"))?,
            output_type_prefix: conduit_semantic_catalog::vision_tracks_type()
                .canonical_bytes()
                .map_err(|error| format!("encode local Vision tracker output type: {error:?}"))?,
            implementation: conduit_std_offers::LOCAL_VISION_TRACK_IMPLEMENTATION.to_string(),
            provider,
            artifact: conduit_std_offers::LOCAL_VISION_TRACK_ARTIFACT.to_string(),
            context,
            tracks,
            sign: String::with_capacity(MAXIMUM_IDENTITY_BYTES),
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        })
    }

    pub(crate) fn process(
        &mut self,
        encoded: &[u8],
        observed_at_micros: u64,
        clock_basis: &str,
        run: &str,
    ) -> Result<&[u8], &'static str> {
        self.validator
            .validate(encoded)
            .map_err(|_| "invalid local Vision object value")?;
        validate_identity(clock_basis).map_err(|_| "invalid Vision clock identity")?;
        validate_identity(run).map_err(|_| "invalid Vision run identity")?;
        let node = encoded
            .strip_prefix(self.type_prefix.as_slice())
            .ok_or("wrong local Vision object type")?;
        let (objects, object_count) = parse_objects(node)?;
        let required = self
            .output_type_prefix
            .len()
            .checked_add(encoded.len())
            .and_then(|bytes| bytes.checked_add(object_count.saturating_mul(4_096)))
            .ok_or("local Vision track output bound overflow")?;
        if required > self.output.capacity() {
            return Err("local Vision track output exceeds admitted storage");
        }
        let mut selected = [usize::MAX; MAXIMUM_TRACKS];
        let mut confidence = [0_u16; MAXIMUM_TRACKS];
        let mut used = [false; MAXIMUM_TRACKS];

        for index in 0..object_count {
            let object = objects[index].ok_or("missing parsed Vision object")?;
            let mut best = None;
            for (slot, track) in self.tracks.iter().enumerate() {
                if !track.active || used[slot] || track.label.as_bytes() != object.label {
                    continue;
                }
                let score = overlap_permille(track.region, object.region);
                if score != 0 && best.is_none_or(|(_, prior)| score > prior) {
                    best = Some((slot, score));
                }
            }
            let (slot, score) = best.unwrap_or_else(|| {
                let slot = self
                    .tracks
                    .iter()
                    .enumerate()
                    .find(|(slot, track)| !used[*slot] && !track.active)
                    .map(|(slot, _)| slot)
                    .or_else(|| used.iter().position(|used| !*used))
                    .unwrap_or(index);
                (slot, 0)
            });
            used[slot] = true;
            selected[index] = slot;
            confidence[index] = score;
            let track = &mut self.tracks[slot];
            if !track.active || score == 0 {
                track.history_len = 0;
            }
            track.active = true;
            track
                .label
                .set(object.label)
                .map_err(|_| "object label bound")?;
            track.region = object.region;
            push_history(track, object.observation_sign)?;
        }
        for (slot, track) in self.tracks.iter_mut().enumerate() {
            if !used[slot] {
                track.active = false;
                track.history_len = 0;
            }
        }

        self.output.clear();
        self.output.extend_from_slice(&self.output_type_prefix);
        wire_collection(&mut self.output, object_count as u16);
        for index in 0..object_count {
            let object = objects[index].ok_or("missing parsed Vision object")?;
            let slot = selected[index];
            let track = &self.tracks[slot];
            let suffix_bytes = "/track-".len() + decimal_digits(slot);
            if run.len().saturating_add(suffix_bytes) > self.sign.capacity() {
                return Err("track Sign bound");
            }
            self.sign.clear();
            write!(self.sign, "{run}/track-{slot}").map_err(|_| "track Sign bound")?;
            validate_identity(&self.sign).map_err(|_| "track Sign bound")?;
            encode_track(
                &mut self.output,
                confidence[index],
                object,
                track,
                &self.artifact,
                &self.implementation,
                &self.sign,
                observed_at_micros,
                clock_basis,
                &self.provider,
                run,
                &self.context,
            );
        }
        Ok(&self.output)
    }
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn push_history(track: &mut TrackState, sign: &[u8]) -> Result<(), &'static str> {
    if track.history_len as usize == MAXIMUM_HISTORY {
        track.history.copy_within(1.., 0);
        track.history_len -= 1;
    }
    track.history[usize::from(track.history_len)]
        .set(sign)
        .map_err(|_| "observation Sign bound")?;
    track.history_len += 1;
    Ok(())
}

fn overlap_permille(left: Region, right: Region) -> u16 {
    let left_right = u32::from(left.x) + u32::from(left.width);
    let right_right = u32::from(right.x) + u32::from(right.width);
    let left_bottom = u32::from(left.y) + u32::from(left.height);
    let right_bottom = u32::from(right.y) + u32::from(right.height);
    let width = left_right
        .min(right_right)
        .saturating_sub(u32::from(left.x.max(right.x)));
    let height = left_bottom
        .min(right_bottom)
        .saturating_sub(u32::from(left.y.max(right.y)));
    let intersection = width * height;
    let union = u32::from(left.width) * u32::from(left.height)
        + u32::from(right.width) * u32::from(right.height)
        - intersection;
    (intersection * 1_000)
        .checked_div(union)
        .map_or(0, |score| score as u16)
}

fn parse_objects(
    node: &[u8],
) -> Result<([Option<ParsedObject<'_>>; MAXIMUM_TRACKS], usize), &'static str> {
    let mut cursor = Cursor::new(node);
    cursor.expect(1)?;
    let count = cursor.length()?;
    if count > MAXIMUM_TRACKS {
        return Err("local Vision object capacity exceeded");
    }
    let mut objects = [None; MAXIMUM_TRACKS];
    for slot in objects.iter_mut().take(count) {
        *slot = Some(parse_object(&mut cursor)?);
    }
    if !cursor.remaining.is_empty() {
        return Err("trailing local Vision object bytes");
    }
    Ok((objects, count))
}

fn parse_object<'a>(cursor: &mut Cursor<'a>) -> Result<ParsedObject<'a>, &'static str> {
    cursor.record(5)?;
    cursor.field("candidate_label")?;
    let label = cursor.leaf()?;
    cursor.field("confidence_permille")?;
    cursor.leaf()?;
    cursor.field("provenance")?;
    cursor.record(7)?;
    cursor.field("artifact")?;
    cursor.leaf()?;
    cursor.field("evidence_class")?;
    cursor.variant()?;
    cursor.leaf()?;
    cursor.field("implementation")?;
    cursor.leaf()?;
    cursor.field("observation_sign")?;
    let observation_sign = cursor.leaf()?;
    cursor.field("observed_at")?;
    cursor.record(5)?;
    for field in [
        "clock_basis",
        "resolution_ticks",
        "scale",
        "ticks",
        "uncertainty_ticks",
    ] {
        cursor.field(field)?;
        cursor.leaf()?;
    }
    cursor.field("provider_instance")?;
    cursor.leaf()?;
    cursor.field("run")?;
    cursor.leaf()?;
    cursor.field("region")?;
    let region = parse_region(cursor)?;
    cursor.field("source_image")?;
    let start = cursor.remaining;
    cursor.record(3)?;
    for field in ["content", "height", "width"] {
        cursor.field(field)?;
        cursor.leaf()?;
    }
    let consumed = start.len() - cursor.remaining.len();
    Ok(ParsedObject {
        label,
        observation_sign,
        region,
        source_image_node: &start[..consumed],
    })
}

fn parse_region(cursor: &mut Cursor<'_>) -> Result<Region, &'static str> {
    cursor.record(4)?;
    let mut values = [0_u16; 4];
    for (index, field) in ["height", "width", "x", "y"].iter().enumerate() {
        cursor.field(field)?;
        values[index] = u16::try_from(
            conduit_core::decode_count(cursor.leaf()?).map_err(|_| "invalid region count")?,
        )
        .map_err(|_| "region count exceeds u16")?;
    }
    Ok(Region {
        height: values[0],
        width: values[1],
        x: values[2],
        y: values[3],
    })
}

#[allow(clippy::too_many_arguments)]
fn encode_track(
    output: &mut Vec<u8>,
    confidence: u16,
    object: ParsedObject<'_>,
    track: &TrackState,
    artifact: &str,
    implementation: &str,
    sign: &str,
    observed_at_micros: u64,
    clock_basis: &str,
    provider: &str,
    run: &str,
    context: &str,
) {
    wire_record(output, 7);
    count_field(
        output,
        "continuity_confidence_permille",
        u64::from(confidence),
    );
    wire_text(output, "contributing_observation_signs");
    wire_collection(output, u16::from(track.history_len));
    for sign in track.history.iter().take(usize::from(track.history_len)) {
        wire_leaf(output, sign.as_bytes());
    }
    wire_text(output, "current_region");
    encode_region(output, object.region);
    wire_text(output, "provenance");
    encode_provenance(
        output,
        artifact,
        implementation,
        sign,
        observed_at_micros,
        clock_basis,
        provider,
        run,
    );
    wire_text(output, "source_image");
    output.extend_from_slice(object.source_image_node);
    wire_text(output, "track");
    wire_leaf(output, track.id.as_bytes());
    text_field(output, "tracking_context", context);
}

fn encode_region(output: &mut Vec<u8>, region: Region) {
    wire_record(output, 4);
    count_field(output, "height", u64::from(region.height));
    count_field(output, "width", u64::from(region.width));
    count_field(output, "x", u64::from(region.x));
    count_field(output, "y", u64::from(region.y));
}

#[allow(clippy::too_many_arguments)]
fn encode_provenance(
    output: &mut Vec<u8>,
    artifact: &str,
    implementation: &str,
    sign: &str,
    observed_at_micros: u64,
    clock_basis: &str,
    provider: &str,
    run: &str,
) {
    wire_record(output, 7);
    text_field(output, "artifact", artifact);
    wire_text(output, "evidence_class");
    wire_variant(output, "statistical_candidate");
    wire_leaf(output, &[]);
    text_field(output, "implementation", implementation);
    text_field(output, "observation_sign", sign);
    wire_text(output, "observed_at");
    wire_record(output, 5);
    text_field(output, "clock_basis", clock_basis);
    count_field(output, "resolution_ticks", 1);
    text_field(output, "scale", "microseconds");
    count_field(output, "ticks", observed_at_micros);
    count_field(output, "uncertainty_ticks", 0);
    text_field(output, "provider_instance", provider);
    text_field(output, "run", run);
}

fn validate_identity(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAXIMUM_IDENTITY_BYTES {
        Err("local Vision identity exceeds its bound".into())
    } else {
        Ok(())
    }
}
