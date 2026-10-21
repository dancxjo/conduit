use crate::Rect;
#[derive(Clone, Copy, Default)]
pub(crate) struct BoxRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
impl From<Rect> for BoxRect {
    fn from(r: Rect) -> Self {
        Self {
            left: i32::from(r.x),
            top: i32::from(r.y),
            right: i32::from(r.x) + i32::from(r.width),
            bottom: i32::from(r.y) + i32::from(r.height),
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Damage {
    pub rects: [BoxRect; 16],
    pub count: usize,
    pub fallback: bool,
}
impl Damage {
    pub const fn new() -> Self {
        Self {
            rects: [BoxRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            }; 16],
            count: 0,
            fallback: false,
        }
    }
    pub fn add(&mut self, rect: Rect) {
        let mut r = BoxRect::from(rect);
        let mut i = 0;
        while i < self.count {
            let o = self.rects[i];
            if r.left <= o.right && o.left <= r.right && r.top <= o.bottom && o.top <= r.bottom {
                r = union(r, o);
                self.count -= 1;
                self.rects.copy_within(i + 1..self.count + 1, i);
                i = 0;
            } else {
                i += 1;
            }
        }
        if self.count == 16 {
            for old in self.rects {
                r = union(r, old);
            }
            self.count = 0;
            self.fallback = true;
        }
        self.rects[self.count] = r;
        self.count += 1;
    }
}
fn union(a: BoxRect, b: BoxRect) -> BoxRect {
    BoxRect {
        left: a.left.min(b.left),
        top: a.top.min(b.top),
        right: a.right.max(b.right),
        bottom: a.bottom.max(b.bottom),
    }
}
