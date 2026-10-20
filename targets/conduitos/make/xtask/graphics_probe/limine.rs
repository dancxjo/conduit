//! Decode immutable debugger observations. Discovery is never display admission.
use super::elf;
pub(super) const MAX_FRAMEBUFFERS: usize = 4;
pub(super) const MAX_MODES: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mode {
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bits_per_pixel: u16,
    pub memory_model: u8,
    pub red: (u8, u8),
    pub green: (u8, u8),
    pub blue: (u8, u8),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Framebuffer {
    pub virtual_address: u64,
    pub current: Mode,
    pub modes: Vec<Mode>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub response_present: bool,
    pub response_revision: Option<u64>,
    pub framebuffers: Vec<Framebuffer>,
}

fn mode(width: u64, height: u64, pitch: u64, format: &[u8]) -> Mode {
    Mode {
        width,
        height,
        pitch,
        bits_per_pixel: u16::from_le_bytes([format[0], format[1]]),
        memory_model: format[2],
        red: (format[3], format[4]),
        green: (format[5], format[6]),
        blue: (format[7], format[8]),
    }
}
fn encoded(words: &[u64]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

/// The supplied reader performs read-only debugger operations. Every pointer
/// extent, list count, returned word count and response version is fenced here.
/// Unsupported raw pixel formats remain observations for the canonical display
/// validator; this tool cannot grant a resource, publish a Face or acknowledge Show.
pub fn observe(
    request_address: u64,
    mut read: impl FnMut(u64, usize) -> Result<Vec<u64>, &'static str>,
) -> Result<Observation, &'static str> {
    let mut words = |address: u64, count: usize| {
        if count > MAX_MODES {
            return Err("guest-memory-word-bound");
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        if address == 0
            || !address.is_multiple_of(8)
            || address.checked_add((count as u64) * 8).is_none()
        {
            return Err("guest-memory-address-invalid");
        }
        let data = read(address, count)?;
        if data.len() != count {
            return Err("guest-memory-word-count");
        }
        Ok(data)
    };
    let request = words(request_address, 6)?;
    if request[..4] != elf::FRAMEBUFFER_REQUEST_ID || request[4] != 0 {
        return Err("guest-request-image-mismatch");
    }
    let response_address = request[5];
    if response_address == 0 {
        return Ok(Observation {
            response_present: false,
            response_revision: None,
            framebuffers: Vec::new(),
        });
    }
    let response = words(response_address, 3)?;
    if response[0] > 1 {
        return Err("framebuffer-response-revision-unsupported");
    }
    let count = usize::try_from(response[1]).map_err(|_| "framebuffer-count-bound")?;
    if count > MAX_FRAMEBUFFERS {
        return Err("framebuffer-count-bound");
    }
    let mut framebuffers = Vec::with_capacity(count);
    for pointer in words(response[2], count)? {
        let raw = words(pointer, if response[0] == 0 { 8 } else { 10 })?;
        let data = encoded(&raw);
        let current = mode(raw[1], raw[2], raw[3], &data[32..41]);
        let mut modes = Vec::new();
        if response[0] == 1 {
            let count = usize::try_from(raw[8]).map_err(|_| "framebuffer-mode-count-bound")?;
            if count > MAX_MODES {
                return Err("framebuffer-mode-count-bound");
            }
            for pointer in words(raw[9], count)? {
                let raw = words(pointer, 5)?;
                let data = encoded(&raw);
                modes.push(mode(raw[1], raw[2], raw[0], &data[24..33]));
            }
        }
        framebuffers.push(Framebuffer {
            virtual_address: raw[0],
            current,
            modes,
        });
    }
    Ok(Observation {
        response_present: true,
        response_revision: Some(response[0]),
        framebuffers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn request(response: u64) -> Vec<u64> {
        let mut data = elf::FRAMEBUFFER_REQUEST_ID.to_vec();
        data.extend([0, response]);
        data
    }
    fn memory() -> BTreeMap<u64, Vec<u64>> {
        let mut bytes = [0; 80];
        bytes[32..41].copy_from_slice(&[32, 0, 1, 8, 16, 8, 8, 8, 0]);
        let mut raw: Vec<u64> = bytes
            .as_chunks::<8>()
            .0
            .iter()
            .map(|x| u64::from_le_bytes(*x))
            .collect();
        raw[..4].copy_from_slice(&[0xffff_0000_5f0f_0000, 800, 600, 3200]);
        [
            (0x1000, request(0x2000)),
            (0x2000, vec![1, 1, 0x3000]),
            (0x3000, vec![0x4000]),
            (0x4000, raw),
        ]
        .into()
    }
    fn inspect(memory: BTreeMap<u64, Vec<u64>>) -> Result<Observation, &'static str> {
        observe(0x1000, |address, _| {
            memory
                .get(&address)
                .cloned()
                .ok_or("guest-memory-unreadable")
        })
    }
    #[test]
    fn records_real_shape_without_inventing_a_graphical_show() {
        let observation = inspect(memory()).unwrap();
        let display = &observation.framebuffers[0];
        assert_eq!(display.current.width, 800);
        assert_eq!(display.current.pitch, 3200);
        assert_eq!(display.current.red, (8, 16));
        assert!(display.modes.is_empty());
    }
    #[test]
    fn absent_response_empty_list_and_wrong_image_remain_distinct() {
        assert!(
            !inspect([(0x1000, request(0))].into())
                .unwrap()
                .response_present
        );
        let mut empty = memory();
        empty.insert(0x2000, vec![1, 0, 0]);
        let observation = inspect(empty).unwrap();
        assert!(observation.response_present);
        assert!(observation.framebuffers.is_empty());
        let mut wrong = memory();
        wrong.get_mut(&0x1000).unwrap()[0] = 0;
        assert_eq!(inspect(wrong), Err("guest-request-image-mismatch"));
    }
    #[test]
    fn excessive_frame_and_mode_counts_refuse_before_reading_their_lists() {
        let mut excessive = memory();
        excessive.insert(0x2000, vec![1, 5, 0]);
        assert_eq!(inspect(excessive), Err("framebuffer-count-bound"));
        let mut excessive = memory();
        excessive.get_mut(&0x4000).unwrap()[8] = 17;
        assert_eq!(inspect(excessive), Err("framebuffer-mode-count-bound"));
    }
    #[test]
    fn bad_revision_addresses_and_truncated_reads_are_not_absence() {
        let mut revision = memory();
        revision.insert(0x2000, vec![2, 1, 0x3000]);
        assert_eq!(
            inspect(revision),
            Err("framebuffer-response-revision-unsupported")
        );
        let mut short = memory();
        short.insert(0x2000, vec![1]);
        assert_eq!(inspect(short), Err("guest-memory-word-count"));
        assert_eq!(
            observe(u64::MAX - 7, |_, _| panic!("overflow must not read")),
            Err("guest-memory-address-invalid")
        );
        assert_eq!(
            observe(0x1001, |_, _| panic!("unaligned must not read")),
            Err("guest-memory-address-invalid")
        );
    }
    #[test]
    fn mode_pointer_layout_preserves_each_selected_format() {
        let mut map = memory();
        map.get_mut(&0x4000).unwrap()[8..10].copy_from_slice(&[1, 0x5000]);
        map.insert(0x5000, vec![0x6000]);
        let mut data = [0; 40];
        data[24..33].copy_from_slice(&[32, 0, 1, 8, 16, 8, 8, 8, 0]);
        let mut raw: Vec<_> = data
            .as_chunks::<8>()
            .0
            .iter()
            .map(|x| u64::from_le_bytes(*x))
            .collect();
        raw[..3].copy_from_slice(&[4096, 1024, 768]);
        map.insert(0x6000, raw);
        let observation = inspect(map).unwrap();
        assert_eq!(observation.framebuffers[0].modes[0].height, 768);
        assert_eq!(observation.framebuffers[0].modes[0].blue, (8, 0));
    }
}
