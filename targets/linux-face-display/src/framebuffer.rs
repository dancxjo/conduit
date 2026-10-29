//! Linux framebuffer ownership and checked scanout; no environmental semantics.
use std::{
    fs::{File, OpenOptions},
    io,
    os::{fd::AsRawFd, unix::fs::FileExt},
    path::Path,
};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Bitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}
#[repr(C)]
#[derive(Default)]
struct Variable {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: Bitfield,
    green: Bitfield,
    blue: Bitfield,
    transp: Bitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}
#[repr(C)]
#[derive(Default)]
struct Fixed {
    id: [u8; 16],
    smem_start: libc::c_ulong,
    smem_len: u32,
    kind: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: libc::c_ulong,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

#[derive(Debug, PartialEq, Eq)]
pub struct Geometry {
    pub width: usize,
    pub height: usize,
    stride: usize,
    offset: usize,
    bytes: usize,
}
impl Geometry {
    fn checked(v: &Variable, f: &Fixed) -> Result<Self, String> {
        let rgb565 = [
            (v.red.offset, v.red.length),
            (v.green.offset, v.green.length),
            (v.blue.offset, v.blue.length),
        ] == [(11, 5), (5, 6), (0, 5)];
        if v.bits_per_pixel != 16
            || !rgb565
            || v.red.msb_right != 0
            || v.green.msb_right != 0
            || v.blue.msb_right != 0
            || v.grayscale != 0
            || v.nonstd != 0
            || v.rotate != 0
            || f.kind != 0
            || f.visual != 2
        {
            return Err("framebuffer requires packed true-color RGB565".into());
        }
        if !(240..=1920).contains(&v.xres)
            || !(240..=1200).contains(&v.yres)
            || v.xoffset
                .checked_add(v.xres)
                .is_none_or(|end| end > v.xres_virtual)
            || v.yoffset
                .checked_add(v.yres)
                .is_none_or(|end| end > v.yres_virtual)
        {
            return Err("framebuffer extent or offsets unsupported".into());
        }
        let width = v.xres as usize;
        let height = v.yres as usize;
        let stride = f.line_length as usize;
        let row_end = (v.xoffset as usize + width)
            .checked_mul(2)
            .ok_or("row overflow")?;
        if row_end > stride {
            return Err("framebuffer stride too short".into());
        }
        let offset = (v.yoffset as usize)
            .checked_mul(stride)
            .and_then(|n| n.checked_add(v.xoffset as usize * 2))
            .ok_or("offset overflow")?;
        let end = (height - 1)
            .checked_mul(stride)
            .and_then(|n| n.checked_add(offset))
            .and_then(|n| n.checked_add(width * 2))
            .ok_or("framebuffer size overflow")?;
        if end > f.smem_len as usize {
            return Err("framebuffer memory too short".into());
        }
        Ok(Self {
            width,
            height,
            stride,
            offset,
            bytes: width * height * 2,
        })
    }
}

pub struct Framebuffer {
    file: File,
    pub geometry: Geometry,
    encoded: Vec<u8>,
}
impl Framebuffer {
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        // The same open file descriptor owns all scanout; concurrent adapters refuse.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        let mut v = Variable::default();
        let mut f = Fixed::default();
        // Linux UAPI fb_var_screeninfo/fb_fix_screeninfo, including native ulong ABI.
        if unsafe { libc::ioctl(file.as_raw_fd(), 0x4600, &mut v) } < 0
            || unsafe { libc::ioctl(file.as_raw_fd(), 0x4602, &mut f) } < 0
        {
            return Err(io::Error::last_os_error().to_string());
        }
        let geometry = Geometry::checked(&v, &f)?;
        let encoded = vec![0; geometry.bytes];
        Ok(Self {
            file,
            geometry,
            encoded,
        })
    }
    pub fn present(&mut self, pixels: &[u32]) -> Result<(), String> {
        let mut current = Variable::default();
        let mut fixed = Fixed::default();
        if unsafe { libc::ioctl(self.file.as_raw_fd(), 0x4600, &mut current) } < 0
            || unsafe { libc::ioctl(self.file.as_raw_fd(), 0x4602, &mut fixed) } < 0
        {
            return Err(io::Error::last_os_error().to_string());
        }
        if Geometry::checked(&current, &fixed)? != self.geometry {
            return Err("framebuffer geometry changed; prepare again".into());
        }
        if pixels.len() != self.geometry.width * self.geometry.height {
            return Err("raster extent changed".into());
        }
        for (pixel, pair) in pixels.iter().zip(self.encoded.chunks_exact_mut(2)) {
            let rgb = (*pixel >> 19 & 31) << 11 | (*pixel >> 10 & 63) << 5 | (*pixel >> 3 & 31);
            pair.copy_from_slice(&(rgb as u16).to_ne_bytes());
        }
        for (y, row) in self
            .encoded
            .chunks_exact(self.geometry.width * 2)
            .enumerate()
        {
            self.file
                .write_all_at(
                    row,
                    (self.geometry.offset + y * self.geometry.stride) as u64,
                )
                .map_err(|e| e.to_string())?;
        }
        // Raspberry Pi OS Lite can leave the HDMI console powered down.
        // FBIOBLANK is owned through the same writable framebuffer descriptor.
        if unsafe { libc::ioctl(self.file.as_raw_fd(), 0x4611, 0 as libc::c_ulong) } < 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mode() -> (Variable, Fixed) {
        (
            Variable {
                xres: 1024,
                yres: 600,
                xres_virtual: 1040,
                yres_virtual: 620,
                xoffset: 4,
                yoffset: 2,
                bits_per_pixel: 16,
                red: Bitfield {
                    offset: 11,
                    length: 5,
                    msb_right: 0,
                },
                green: Bitfield {
                    offset: 5,
                    length: 6,
                    msb_right: 0,
                },
                blue: Bitfield {
                    offset: 0,
                    length: 5,
                    msb_right: 0,
                },
                ..Default::default()
            },
            Fixed {
                line_length: 2080,
                smem_len: 2080 * 620,
                visual: 2,
                ..Default::default()
            },
        )
    }
    #[test]
    fn offsets_and_padded_rows_are_checked() {
        let (v, f) = mode();
        let g = Geometry::checked(&v, &f).unwrap();
        assert_eq!(g.offset, 4168);
        assert_eq!(g.stride, 2080);
        assert_eq!(g.bytes, 1228800);
    }
    #[test]
    fn short_memory_and_wrong_color_order_refuse() {
        let (mut v, mut f) = mode();
        f.smem_len = 100;
        assert!(Geometry::checked(&v, &f).is_err());
        f.smem_len = 2080 * 620;
        v.red.offset = 0;
        assert!(Geometry::checked(&v, &f).is_err());
    }
    #[test]
    fn row_crossing_and_extent_overflow_refuse() {
        let (mut v, f) = mode();
        v.xoffset = 20;
        assert!(Geometry::checked(&v, &f).is_err());
        v.xoffset = u32::MAX;
        assert!(Geometry::checked(&v, &f).is_err());
    }
}
