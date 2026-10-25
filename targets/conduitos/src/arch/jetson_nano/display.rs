//! Retained Tegra210 scanout admission. No display modes, clocks, DMA mappings,
//! register selectors or controller state are changed here.
//!
//! Register units and layout follow Linux v6.12 drivers/gpu/drm/tegra/dc.h
//! and dc.c; color ordering follows plane.c (DRM ARGB8888/ABGR8888).
//! Global SMMU_CONFIG follows drivers/iommu/tegra-smmu.c.
//! Sources:
//! https://github.com/torvalds/linux/blob/v6.12/drivers/gpu/drm/tegra/dc.h
//! https://github.com/torvalds/linux/blob/v6.12/drivers/gpu/drm/tegra/dc.c
//! https://github.com/torvalds/linux/blob/v6.12/drivers/gpu/drm/tegra/plane.c
//! https://github.com/torvalds/linux/blob/v6.12/drivers/iommu/tegra-smmu.c
//! A tegra_fbmem boot argument supplies only an address/extent, not a mode.
//! The caller must retain firmware clocks/power and obtain active-window reads.
//! This is not proof that an HDMI sink is attached, nor a modesetting driver.

use super::memory::{Region, Regions};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    InactiveBank,
    WindowSelection,
    NotContinuous,
    Translation,
    Layout,
    Geometry,
    Format,
    Address,
    Reserved,
    Unstable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelFormat {
    /// Little-endian bytes B, G, R, A.
    Bgra8888,
    /// Little-endian bytes R, G, B, A.
    Rgba8888,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Descriptor {
    pub memory: Region,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
}

/// Only the already selected window is observed. DC register offsets are word
/// indices (multiply by four for MMIO); MC offsets are bytes. These callbacks
/// permit host conformance without touching hardware. The caller owns physical
/// MMIO admission and must not return cached/shadow values as active values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub state_access: u32,
    pub selected_window: u32,
    pub display_command: u32,
    pub smmu_config: u32,
    pub active_size: u32,
    pub interlace_control: u32,
    pub options: u32,
    pub byte_swap: u32,
    pub buffer_control: u32,
    pub color_depth: u32,
    pub position: u32,
    pub size: u32,
    pub prescaled_size: u32,
    pub initial_dda_h: u32,
    pub initial_dda_v: u32,
    pub dda_inc: u32,
    pub stride: u32,
    pub buffer_stride: u32,
    pub uv_stride: u32,
    pub address_mode: u32,
    pub address: u32,
    pub address_high: u32,
    pub horizontal_offset: u32,
    pub vertical_offset: u32,
    pub surface_kind: u32,
}

impl Snapshot {
    fn read(dc: &mut impl FnMut(u32) -> u32, mc: &mut impl FnMut(u32) -> u32) -> Self {
        Self {
            state_access: dc(0x040),
            selected_window: dc(0x042),
            display_command: dc(0x032),
            smmu_config: mc(0x010),
            active_size: dc(0x409),
            interlace_control: dc(0x4e5),
            options: dc(0x700),
            byte_swap: dc(0x701),
            buffer_control: dc(0x702),
            color_depth: dc(0x703),
            position: dc(0x704),
            size: dc(0x705),
            prescaled_size: dc(0x706),
            initial_dda_h: dc(0x707),
            initial_dda_v: dc(0x708),
            dda_inc: dc(0x709),
            stride: dc(0x70a),
            buffer_stride: dc(0x70b),
            uv_stride: dc(0x70c),
            address_mode: dc(0x70d),
            address: dc(0x800),
            address_high: dc(0x80d),
            horizontal_offset: dc(0x806),
            vertical_offset: dc(0x808),
            surface_kind: dc(0x80b),
        }
    }

    /// Bounded double observation rejects a changed mode/selection. Firmware
    /// and secondary cores must already be quiescent; equality is not a lock.
    pub fn capture(
        mut dc: impl FnMut(u32) -> u32,
        mut mc: impl FnMut(u32) -> u32,
    ) -> Result<Self, Refusal> {
        let first = Self::read(&mut dc, &mut mc);
        let second = Self::read(&mut dc, &mut mc);
        if first != second {
            return Err(Refusal::Unstable);
        }
        Ok(first)
    }

    /// Admit a linear, unscaled, uncropped 32-bit retained window only when it
    /// exactly begins at firmware's declared framebuffer. `protected` contains
    /// image, stack, DTB, page tables and live firmware other than this exact
    /// framebuffer allocation. Never remove arbitrary reservations to use it.
    /// Caller must establish this region is CPU-accessible physical memory.
    pub fn descriptor(
        self,
        firmware_framebuffer: Region,
        protected: &Regions,
    ) -> Result<Descriptor, Refusal> {
        if self.state_access & 1 == 0 {
            return Err(Refusal::InactiveBank);
        }
        if !matches!(self.selected_window, 0x10 | 0x20 | 0x40) {
            return Err(Refusal::WindowSelection);
        }
        if self.display_command & (3 << 5) != 1 << 5 {
            return Err(Refusal::NotContinuous);
        }
        // Deliberately require global bypass. A programmed ASID/client map
        // cannot establish an identity translation without walking its tables.
        if self.smmu_config & 1 != 0 {
            return Err(Refusal::Translation);
        }
        if self.options != 1 << 30
            || self.byte_swap != 0
            || self.buffer_control != 0
            || self.interlace_control != 0
            || self.position != 0
            || self.initial_dda_h != 0
            || self.initial_dda_v != 0
            || self.dda_inc != 0x1000_1000
            || self.buffer_stride != 0
            || self.uv_stride != 0
            || self.address_mode != 0
            || self.surface_kind != 0
            || self.horizontal_offset != 0
            || self.vertical_offset != 0
        {
            return Err(Refusal::Layout);
        }
        let format = match self.color_depth {
            12 => PixelFormat::Bgra8888,
            13 => PixelFormat::Rgba8888,
            _ => return Err(Refusal::Format),
        };
        let width = self.size & 0x1fff;
        let height = (self.size >> 16) & 0x1fff;
        if self.size != self.active_size
            || self.size & !0x1fff_1fff != 0
            || width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || self.prescaled_size != ((height << 16) | (width * 4))
            || self.stride < width * 4
            || self.stride > 65532
            || self.stride & 3 != 0
        {
            return Err(Refusal::Geometry);
        }
        if self.address_high != 0
            || u64::from(self.address) != firmware_framebuffer.start()
            || self.address & 3 != 0
            || u64::from(self.stride) * u64::from(height) > firmware_framebuffer.bytes()
        {
            return Err(Refusal::Address);
        }
        if protected.iter().any(|entry| {
            entry.start() < firmware_framebuffer.start() + firmware_framebuffer.bytes()
                && firmware_framebuffer.start() < entry.start() + entry.bytes()
        }) {
            return Err(Refusal::Reserved);
        }
        Ok(Descriptor {
            memory: firmware_framebuffer,
            width,
            height,
            stride: self.stride,
            format,
        })
    }
}

#[cfg(test)]
#[path = "display_tests.rs"]
mod tests;
