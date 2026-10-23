use super::{MAX_DEPTH, MAX_WORK, Node, RESOLVE_RESERVE, Refusal, STEP_WORK, Shape};
use core::fmt::Write;
use patchbay_graph::PatchbayGraph;
const VISIT: u8 = 0;
const OBJECT: u8 = 1;
const SEQUENCE: u8 = 2;
const RAW: u8 = 3;
#[derive(Clone, Copy)]
struct Frame {
    next: u32,
    len: u32,
    depth: u16,
    tag: u8,
    stage: u8,
}
const EMPTY: Frame = Frame {
    next: 0,
    len: 0,
    depth: 0,
    tag: VISIT,
    stage: 0,
};
#[derive(Clone, Copy)]
enum Text {
    None,
    Model,
    Static(&'static str),
}
/// All continuation storage is fixed and owned. The graph and output slot MUST
/// remain immutable/exclusive respectively across steps (enforced by State).
pub(crate) struct GraphEncoding {
    frames: [Frame; MAX_DEPTH + 1],
    path: [u32; MAX_DEPTH],
    count: u16,
    text: Text,
    text_depth: u16,
    text_at: u32,
    text_stage: u8,
    literal: [u8; 32],
    literal_len: u8,
    literal_at: u8,
    length: u32,
    work: u64,
    failed: bool,
    done: bool,
}
impl GraphEncoding {
    pub(crate) fn new() -> Self {
        Self::for_node()
    }
    pub(super) fn for_node() -> Self {
        Self {
            frames: [EMPTY; MAX_DEPTH + 1],
            path: [0; MAX_DEPTH],
            count: 1,
            text: Text::None,
            text_depth: 0,
            text_at: 0,
            text_stage: 0,
            literal: [0; 32],
            literal_len: 0,
            literal_at: 0,
            length: 0,
            work: 0,
            failed: false,
            done: false,
        }
    }
    pub(crate) fn length(&self) -> usize {
        self.length as usize
    }
    pub(crate) fn cursor(&self) -> u64 {
        self.work
    }
    pub(crate) fn step(
        &mut self,
        graph: Option<&PatchbayGraph>,
        output: &mut [u8],
    ) -> Result<bool, Refusal> {
        self.step_node(Node::Envelope(graph), output)
    }
    pub(super) fn step_node(&mut self, root: Node<'_>, output: &mut [u8]) -> Result<bool, Refusal> {
        if self.failed || self.done {
            return Err(Refusal::Order);
        }
        let result = self.run(root, output);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn spend(&mut self, budget: &mut u32, amount: u32) -> Result<(), Refusal> {
        *budget = budget.checked_sub(amount).ok_or(Refusal::Bounds)?;
        self.work = self
            .work
            .checked_add(u64::from(amount))
            .filter(|v| *v <= MAX_WORK)
            .ok_or(Refusal::Bounds)?;
        Ok(())
    }
    fn byte(&mut self, output: &mut [u8], budget: &mut u32, value: u8) -> Result<(), Refusal> {
        if self.length() >= super::super::MAX_DECODED_JSON_BYTES {
            return Err(Refusal::Capacity);
        }
        *output.get_mut(self.length()).ok_or(Refusal::Capacity)? = value;
        self.length = self.length.checked_add(1).ok_or(Refusal::Capacity)?;
        self.spend(budget, 1)
    }
    fn resolve<'a>(
        &mut self,
        mut root: Node<'a>,
        depth: usize,
        budget: &mut u32,
    ) -> Result<Node<'a>, Refusal> {
        if depth > MAX_DEPTH {
            return Err(Refusal::Bounds);
        }
        self.spend(budget, 1)?;
        for at in 0..depth {
            // All schema accesses are O(1), except fixed-width physical encode().
            let cost = root.child_work();
            self.spend(budget, cost)?;
            root = root.child(self.path[at] as usize)?;
        }
        Ok(root)
    }
    fn literal(&mut self, value: &str) -> Result<(), Refusal> {
        if value.len() > self.literal.len() {
            return Err(Refusal::Bounds);
        }
        self.literal[..value.len()].copy_from_slice(value.as_bytes());
        self.literal_len = value.len() as u8;
        self.literal_at = 0;
        Ok(())
    }
    fn number(&mut self, value: impl core::fmt::Display, quoted: bool) -> Result<(), Refusal> {
        struct Buffer<'a> {
            bytes: &'a mut [u8],
            len: usize,
        }
        impl Write for Buffer<'_> {
            fn write_str(&mut self, s: &str) -> core::fmt::Result {
                let end = self.len.checked_add(s.len()).ok_or(core::fmt::Error)?;
                self.bytes
                    .get_mut(self.len..end)
                    .ok_or(core::fmt::Error)?
                    .copy_from_slice(s.as_bytes());
                self.len = end;
                Ok(())
            }
        }
        let mut b = Buffer {
            bytes: &mut self.literal,
            len: 0,
        };
        if quoted {
            b.write_char('"').map_err(|_| Refusal::Bounds)?;
        }
        write!(&mut b, "{value}").map_err(|_| Refusal::Bounds)?;
        if quoted {
            b.write_char('"').map_err(|_| Refusal::Bounds)?;
        }
        self.literal_len = b.len as u8;
        self.literal_at = 0;
        Ok(())
    }
    fn string(&mut self, text: Text, depth: u16) {
        self.text = text;
        self.text_depth = depth;
        self.text_at = 0;
        self.text_stage = 0;
    }
    fn push(&mut self, parent: Frame, index: u32) -> Result<(), Refusal> {
        let d = usize::from(parent.depth);
        if d >= MAX_DEPTH || usize::from(self.count) >= self.frames.len() {
            return Err(Refusal::Bounds);
        }
        self.path[d] = index;
        self.frames[usize::from(self.count)] = Frame {
            depth: parent.depth + 1,
            ..EMPTY
        };
        self.count += 1;
        Ok(())
    }
    fn run(&mut self, root: Node<'_>, output: &mut [u8]) -> Result<bool, Refusal> {
        let mut budget = STEP_WORK;
        loop {
            if self.literal_at < self.literal_len {
                if budget == 0 {
                    return Ok(false);
                }
                self.byte(output, &mut budget, self.literal[self.literal_at as usize])?;
                self.literal_at += 1;
                continue;
            }
            if !matches!(self.text, Text::None) {
                if budget < RESOLVE_RESERVE {
                    return Ok(false);
                }
                let text = match self.text {
                    Text::Static(s) => s,
                    Text::Model => match self
                        .resolve(root, self.text_depth as usize, &mut budget)?
                        .shape()?
                    {
                        Shape::String(s) => s,
                        _ => return Err(Refusal::Order),
                    },
                    Text::None => return Err(Refusal::Order),
                };
                // Resolve once per turn/chunk, never once per source byte.
                if self.text_stage == 0 {
                    self.byte(output, &mut budget, b'"')?;
                    self.text_stage = 1;
                }
                while let Some(&b) = text.as_bytes().get(self.text_at as usize) {
                    if budget < 8 {
                        return Ok(false);
                    }
                    self.spend(&mut budget, 1)?; // input scan, including UTF-8 bytes
                    match b {
                        b'"' | b'\\' => {
                            self.byte(output, &mut budget, b'\\')?;
                            self.byte(output, &mut budget, b)?;
                        }
                        8 | 9 | 10 | 12 | 13 => {
                            self.byte(output, &mut budget, b'\\')?;
                            self.byte(
                                output,
                                &mut budget,
                                match b {
                                    8 => b'b',
                                    9 => b't',
                                    10 => b'n',
                                    12 => b'f',
                                    _ => b'r',
                                },
                            )?;
                        }
                        0..=31 => {
                            for c in [
                                b'\\',
                                b'u',
                                b'0',
                                b'0',
                                b"0123456789abcdef"[(b >> 4) as usize],
                                b"0123456789abcdef"[(b & 15) as usize],
                            ] {
                                self.byte(output, &mut budget, c)?;
                            }
                        }
                        _ => self.byte(output, &mut budget, b)?,
                    }
                    self.text_at = self.text_at.checked_add(1).ok_or(Refusal::Bounds)?;
                }
                if budget == 0 {
                    return Ok(false);
                }
                self.byte(output, &mut budget, b'"')?;
                self.text = Text::None;
                continue;
            }
            if self.count == 0 {
                self.done = true;
                return Ok(true);
            }
            if budget < RESOLVE_RESERVE {
                return Ok(false);
            }
            let top = usize::from(self.count - 1);
            let mut frame = self.frames[top];
            self.spend(&mut budget, 1)?; // structural dispatch (even no output)
            if frame.tag == VISIT {
                let shape = self
                    .resolve(root, frame.depth as usize, &mut budget)?
                    .shape()?;
                self.spend(&mut budget, 32)?; // bounded primitive formatting/schema dispatch
                match shape {
                    Shape::Object(keys) => {
                        frame.tag = OBJECT;
                        frame.len = u32::try_from(keys.len()).map_err(|_| Refusal::Bounds)?;
                        self.byte(output, &mut budget, b'{')?;
                        self.frames[top] = frame;
                    }
                    Shape::Sequence(len) | Shape::RawSequence(len) => {
                        frame.tag = if matches!(shape, Shape::RawSequence(_)) {
                            RAW
                        } else {
                            SEQUENCE
                        };
                        frame.len = u32::try_from(len).map_err(|_| Refusal::Bounds)?;
                        self.byte(output, &mut budget, b'[')?;
                        if frame.tag == SEQUENCE {
                            self.number(frame.len, false)?;
                        }
                        self.frames[top] = frame;
                    }
                    Shape::String(_) => {
                        self.count -= 1;
                        self.string(Text::Model, frame.depth);
                    }
                    Shape::Null => {
                        self.count -= 1;
                        self.literal("null")?;
                    }
                    Shape::Bool(v) => {
                        self.count -= 1;
                        self.literal(if v { "true" } else { "false" })?;
                    }
                    Shape::U64(v) => {
                        self.count -= 1;
                        self.number(v, false)?;
                    }
                    Shape::I64(v) => {
                        self.count -= 1;
                        self.number(v, false)?;
                    }
                    Shape::DecimalU64(v) => {
                        self.count -= 1;
                        self.number(v, true)?;
                    }
                    Shape::DecimalI64(v) => {
                        self.count -= 1;
                        self.number(v, true)?;
                    }
                }
                continue;
            }
            if frame.next == frame.len {
                self.byte(
                    output,
                    &mut budget,
                    if frame.tag == OBJECT { b'}' } else { b']' },
                )?;
                self.count -= 1;
                continue;
            }
            if frame.tag == OBJECT && frame.stage == 0 {
                let Shape::Object(keys) = self
                    .resolve(root, frame.depth as usize, &mut budget)?
                    .shape()?
                else {
                    return Err(Refusal::Order);
                };
                if frame.next > 0 {
                    self.byte(output, &mut budget, b',')?;
                }
                self.string(Text::Static(keys[frame.next as usize]), 0);
                frame.stage = 1;
                self.frames[top] = frame;
                continue;
            }
            if frame.tag == OBJECT {
                self.byte(output, &mut budget, b':')?;
            } else if frame.tag == SEQUENCE || frame.next > 0 {
                self.byte(output, &mut budget, b',')?;
            }
            let index = frame.next;
            frame.next += 1;
            frame.stage = 0;
            self.frames[top] = frame;
            self.push(frame, index)?;
        }
    }
}
// A compile-time size gate; actual integrated Retained assertion must also pass.
const _: () = assert!(core::mem::size_of::<GraphEncoding>() <= 2304);
