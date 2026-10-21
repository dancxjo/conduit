use crate::{damage::BoxRect, *};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompletedFrame {
    sequence: u64,
    coverage: [u32; MAX_SURFACES],
}
impl CompletedFrame {
    pub fn sequence(self) -> u64 {
        self.sequence
    }
    pub fn coverage(self) -> [u32; MAX_SURFACES] {
        self.coverage
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub sequence: u64,
    pub x: u32,
    pub y: u32,
    pub pixels: usize,
    pub complete: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct Progress {
    rects: [BoxRect; 16],
    count: usize,
    index: usize,
    x: i32,
    y: i32,
    coverage: [u32; MAX_SURFACES],
}
impl Compositor<'_> {
    pub fn begin_frame(&mut self) -> Result<u64, Error> {
        self.idle()?;
        self.invalidate();
        let sequence = self.sequence.checked_add(1).ok_or(Error::Exhausted)?;
        let mut p = Progress {
            rects: [BoxRect::default(); 16],
            count: 0,
            index: 0,
            x: 0,
            y: 0,
            coverage: [0; MAX_SURFACES],
        };
        for r in &self.damage.rects[..self.damage.count] {
            let clipped = BoxRect {
                left: r.left.max(0),
                top: r.top.max(0),
                right: r.right.min(i32::from(self.width)),
                bottom: r.bottom.min(i32::from(self.height)),
            };
            if clipped.left < clipped.right && clipped.top < clipped.bottom {
                p.rects[p.count] = clipped;
                p.count += 1;
            }
        }
        if p.count == 0 {
            return Err(Error::NoDamage);
        }
        p.x = p.rects[0].left;
        p.y = p.rects[0].top;
        self.sequence = sequence;
        self.frame = Some(p);
        Ok(sequence)
    }
    /// One contiguous horizontal output span, never more than 64KiB. A failed
    /// or cancelled frame keeps all pending damage and invalidates prior input.
    pub fn compose_next(&mut self, sequence: u64, output: &mut [u32]) -> Result<Span, Error> {
        if sequence != self.sequence {
            return Err(Error::Stale);
        }
        if output.is_empty() || output.len() > MAX_CHUNK {
            self.fail_frame();
            return Err(Error::Bounds);
        }
        let mut p = self.frame.take().ok_or(Error::Order)?;
        let r = p.rects[p.index];
        let n = output.len().min((r.right - p.x) as usize);
        let x = p.x as u32;
        let y = p.y as u32;
        let focus = self.focus.and_then(|h| self.index(h).ok()).and_then(|i| {
            let s = self.surfaces[i];
            (s.visible && s.ready).then_some(s.rect)
        });
        for (left, right, selected) in self.intervals(x, x + n as u32, y, focus) {
            let start = (left - x) as usize;
            let len = (right - left) as usize;
            let target = &mut output[start..start + len];
            if let Some(i) = selected {
                let s = self.surfaces[i];
                let offset = s.offset
                    + (i64::from(y) - i64::from(s.rect.y)) as usize * usize::from(s.rect.width)
                    + (i64::from(left) - i64::from(s.rect.x)) as usize;
                target.copy_from_slice(&self.pixels[offset..offset + len]);
            } else {
                target.fill(0);
            }
            let focused = focus.is_some_and(|r| {
                r.contains(left, y)
                    && (i64::from(left) == i64::from(r.x)
                        || i64::from(left) == i64::from(r.x) + i64::from(r.width) - 1
                        || i64::from(y) == i64::from(r.y)
                        || i64::from(y) == i64::from(r.y) + i64::from(r.height) - 1)
            });
            let mut uncovered = if focused {
                target.fill(0x00ffcc33);
                0
            } else {
                len as u32
            };
            if self
                .cursor
                .is_some_and(|(cx, cy)| left >= cx && left < cx + 10 && y >= cy && y < cy + 16)
            {
                for (dx, pixel) in target.iter_mut().enumerate() {
                    if let Some(color) = self.cursor.and_then(|position| {
                        cursor_color(position, left + dx as u32, y, self.hover)
                    }) {
                        *pixel = color;
                        if !focused {
                            uncovered -= 1;
                        }
                    }
                }
            }
            if let Some(i) = selected {
                p.coverage[i] += uncovered;
            }
        }
        p.x += n as i32;
        if p.x == r.right {
            p.x = r.left;
            p.y += 1;
            if p.y == r.bottom {
                p.index += 1;
                if p.index < p.count {
                    p.x = p.rects[p.index].left;
                    p.y = p.rects[p.index].top;
                }
            }
        }
        let complete = p.index == p.count;
        if complete {
            self.completed = Some(CompletedFrame {
                sequence,
                coverage: p.coverage,
            });
            self.damage = Damage::new();
        } else {
            self.frame = Some(p);
        }
        Ok(Span {
            sequence,
            x,
            y,
            pixels: n,
            complete,
        })
    }
    pub fn completed(&self) -> Option<CompletedFrame> {
        self.completed
    }
    /// This is a message from a scanout owner, not evidence obtained by rendering.
    /// The Root client must call it only after validating and copying every span.
    pub fn acknowledge_scanout(&mut self, sequence: u64) -> Result<(), Error> {
        let complete = self
            .completed
            .filter(|frame| frame.sequence == sequence)
            .ok_or(Error::Stale)?;
        for (i, count) in complete.coverage.into_iter().enumerate() {
            self.acknowledged[i] |= count > 0;
        }
        Ok(())
    }
    pub fn fail_frame(&mut self) {
        self.invalidate();
        self.acknowledged.fill(false);
        // Completion may already have consumed damage before Root reports a copy
        // failure. Repaint the complete display to recover without stale coverage.
        self.damage.add(Rect {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        });
    }
}
const OUTLINE: [u16; 16] = [
    0x0001, 0x0003, 0x0005, 0x0009, 0x0011, 0x0021, 0x0041, 0x0081, 0x0101, 0x03e1, 0x0049, 0x0095,
    0x0093, 0x0121, 0x0120, 0x00c0,
];
const FILL: [u16; 16] = [
    0, 0, 0x0002, 0x0006, 0x000e, 0x001e, 0x003e, 0x007e, 0x00fe, 0x001e, 0x0036, 0x0062, 0x0060,
    0x00c0, 0x00c0, 0,
];
fn cursor_color((cx, cy): (u32, u32), x: u32, y: u32, hover: bool) -> Option<u32> {
    let dx = x.checked_sub(cx)?;
    let dy = y.checked_sub(cy)?;
    let row = usize::try_from(dy).ok()?;
    let bit = 1u16.checked_shl(dx)?;
    if FILL.get(row).is_some_and(|m| m & bit != 0) {
        return Some(if hover { 0x00ffcc33 } else { 0x00ffffff });
    }
    OUTLINE.get(row).is_some_and(|m| m & bit != 0).then_some(0)
}
