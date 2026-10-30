//! Linux event-device acquisition only. The Mask owns all control and gesture meaning.
//! One prepared descriptor, five contacts, at most128 kernel events per poll.
use std::{
    fs::File,
    io::Read,
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::Path,
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contact {
    pub active: bool,
    pub x: u16,
    pub y: u16,
}
#[derive(Clone, Copy, Default)]
struct Slot {
    active: bool,
    x: Option<i32>,
    y: Option<i32>,
}
pub struct TouchInput {
    file: File,
    slots: [Slot; 5],
    slot: Option<usize>,
    ranges: [(i32, i32); 2],
    size: (u16, u16),
    multitouch: bool,
    needs_release: bool,
}
fn range(fd: i32, code: u32) -> Result<(i32, i32), String> {
    let mut info = [0i32; 6];
    // SAFETY: EVIOCGABS writes exactly six i32 fields into this valid buffer.
    if unsafe {
        libc::ioctl(
            fd,
            (0x8018_4540u32 + code) as libc::c_ulong,
            info.as_mut_ptr(),
        )
    } < 0
        || (info[2] < info[1] || (code != 47 && info[2] == info[1]))
    {
        return Err("unsupported touch axis range".into());
    }
    Ok((info[1], info[2]))
}
fn coordinate(value: i32, range: (i32, i32), extent: u16) -> u16 {
    let value = i64::from(value.clamp(range.0, range.1)) - i64::from(range.0);
    (value * i64::from(extent.saturating_sub(1)) / (i64::from(range.1) - i64::from(range.0))) as u16
}
impl TouchInput {
    pub fn open(path: &Path, width: u16, height: u16) -> Result<Self, String> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(path)
            .map_err(|e| e.to_string())?;
        use std::os::unix::fs::FileTypeExt;
        if !file
            .metadata()
            .map_err(|e| e.to_string())?
            .file_type()
            .is_char_device()
        {
            return Err("input must be a character device".into());
        }
        let fd = file.as_raw_fd();
        let multitouch = range(fd, 53).is_ok() && range(fd, 54).is_ok();
        if multitouch {
            let slots = range(fd, 47)?;
            if slots.0 != 0 || slots.1 > 4 {
                return Err("touch contact capacity exceeded".into());
            }
        }
        let ranges = if multitouch {
            [range(fd, 53)?, range(fd, 54)?]
        } else {
            [range(fd, 0)?, range(fd, 1)?]
        };
        let mut keys = [0u8; 96];
        // SAFETY: EVIOCGKEY writes at most the declared 96 bytes.
        if unsafe { libc::ioctl(fd, 0x8060_4518u32 as libc::c_ulong, keys.as_mut_ptr()) } < 0 {
            return Err("touch button state unavailable".into());
        }
        let needs_release = keys[330 / 8] & (1 << (330 % 8)) != 0;
        let mut slots = [Slot::default(); 5];
        for (axis, field) in [
            (if multitouch { 53 } else { 0 }, 0),
            (if multitouch { 54 } else { 1 }, 1),
        ] {
            let mut values = [0i32; 6];
            values[0] = axis;
            if multitouch {
                // SAFETY: EVIOCGMTSLOTS writes the six declared i32 fields.
                if unsafe { libc::ioctl(fd, 0x8018_450au32 as libc::c_ulong, values.as_mut_ptr()) }
                    < 0
                {
                    return Err("touch slot coordinates unavailable".into());
                }
            } else {
                // SAFETY: EVIOCGABS writes six i32 fields.
                if unsafe {
                    libc::ioctl(
                        fd,
                        (0x8018_4540u32 + axis as u32) as libc::c_ulong,
                        values.as_mut_ptr(),
                    )
                } < 0
                {
                    return Err("touch coordinates unavailable".into());
                }
                let current = values[0];
                values[1..].fill(current);
            }
            for (slot, value) in slots.iter_mut().zip(&values[1..]) {
                if field == 0 {
                    slot.x = Some(*value);
                } else {
                    slot.y = Some(*value);
                }
            }
        }
        Ok(Self {
            file,
            slots,
            slot: Some(0),
            ranges,
            size: (width, height),
            multitouch,
            needs_release,
        })
    }
    /// Auto-selection is bounded and only admits absolute-position devices.
    pub fn discover(width: u16, height: u16) -> Option<Self> {
        if let Some(path) = std::env::var_os("CONDUIT_DISPLAY_INPUT_DEVICE") {
            return Self::open(Path::new(&path), width, height).ok();
        }
        (0..32).find_map(|index| {
            Self::open(
                Path::new(&format!("/dev/input/event{index}")),
                width,
                height,
            )
            .ok()
        })
    }
    pub fn poll(&mut self, mut frame: impl FnMut([Contact; 5])) -> Result<(), String> {
        let mut raw = [0u8; std::mem::size_of::<libc::input_event>()];
        for _ in 0..128 {
            match self.file.read(&mut raw) {
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error.to_string()),
                Ok(0) => return Err("touch device disconnected".into()),
                Ok(n) if n != raw.len() => return Err("partial Linux input event refused".into()),
                Ok(_) => {}
            }
            // SAFETY: the byte array has exactly input_event size; unaligned read copies it.
            let event =
                unsafe { std::ptr::read_unaligned(raw.as_ptr().cast::<libc::input_event>()) };
            if let Some(contacts) = self.apply_event(event)? {
                frame(contacts);
            }
        }
        Ok(())
    }
    fn apply_event(&mut self, event: libc::input_event) -> Result<Option<[Contact; 5]>, String> {
        if event.type_ == 0 && event.code == 3 {
            return Err("touch queue overrun; reconnect required".into());
        }
        if event.type_ == 3 {
            if self.multitouch && event.code == 47 {
                self.slot = usize::try_from(event.value).ok().filter(|i| *i < 5);
            }
            if let Some(index) = self.slot {
                let slot = &mut self.slots[index];
                match event.code {
                    57 if self.multitouch => {
                        slot.active = event.value >= 0;
                    }
                    53 if self.multitouch => slot.x = Some(event.value),
                    54 if self.multitouch => slot.y = Some(event.value),
                    0 if !self.multitouch => slot.x = Some(event.value),
                    1 if !self.multitouch => slot.y = Some(event.value),
                    _ => {}
                }
            }
        }
        if event.type_ == 1 && event.code == 330 && !self.multitouch {
            self.slots[0].active = event.value != 0;
        }
        if event.type_ == 0 && event.code == 0 {
            if self.needs_release {
                if self.slots.iter().all(|s| !s.active) {
                    self.needs_release = false;
                }
                return Ok(None);
            }
            let contacts = self.slots.map(|slot| Contact {
                active: slot.active && slot.x.is_some() && slot.y.is_some(),
                x: coordinate(
                    slot.x.unwrap_or(self.ranges[0].0),
                    self.ranges[0],
                    self.size.0,
                ),
                y: coordinate(
                    slot.y.unwrap_or(self.ranges[1].0),
                    self.ranges[1],
                    self.size.1,
                ),
            });
            return Ok(Some(contacts));
        }
        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event(type_: u16, code: u16, value: i32) -> libc::input_event {
        libc::input_event {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            type_,
            code,
            value,
        }
    }
    fn device() -> TouchInput {
        TouchInput {
            file: File::open("/dev/null").unwrap(),
            slots: [Slot {
                x: Some(0),
                y: Some(0),
                active: false,
            }; 5],
            slot: Some(0),
            ranges: [(0, 1024), (0, 600)],
            size: (1024, 600),
            multitouch: true,
            needs_release: false,
        }
    }
    #[test]
    fn repeated_tap_preserves_unchanged_kernel_slot_coordinates_and_syn_is_atomic() {
        let mut d = device();
        assert!(d.apply_event(event(3, 57, 17)).unwrap().is_none());
        d.apply_event(event(3, 53, 500)).unwrap();
        d.apply_event(event(3, 54, 300)).unwrap();
        let first = d.apply_event(event(0, 0, 0)).unwrap().unwrap()[0];
        assert!(first.active);
        d.apply_event(event(3, 57, -1)).unwrap();
        assert!(!d.apply_event(event(0, 0, 0)).unwrap().unwrap()[0].active);
        d.apply_event(event(3, 57, 18)).unwrap();
        assert_eq!(d.apply_event(event(0, 0, 0)).unwrap().unwrap()[0], first);
        assert!(d.apply_event(event(0, 3, 0)).is_err());
    }
    #[test]
    fn held_touch_after_reconnect_cannot_become_a_control_press() {
        let mut d = device();
        d.needs_release = true;
        d.apply_event(event(3, 57, 17)).unwrap();
        assert!(d.apply_event(event(0, 0, 0)).unwrap().is_none());
        d.apply_event(event(3, 57, -1)).unwrap();
        assert!(d.apply_event(event(0, 0, 0)).unwrap().is_none());
        d.apply_event(event(3, 57, 18)).unwrap();
        assert!(d.apply_event(event(0, 0, 0)).unwrap().unwrap()[0].active);
    }
    #[test]
    fn device_extents_map_exactly_to_pixels_and_clamp_without_wrapping() {
        assert_eq!(coordinate(1024, (0, 1024), 1024), 1023);
        assert_eq!(coordinate(600, (0, 600), 600), 599);
        assert_eq!(coordinate(i32::MIN, (-10, 100), 1024), 0);
        assert_eq!(coordinate(i32::MAX, (-10, 100), 1024), 1023);
    }
}
