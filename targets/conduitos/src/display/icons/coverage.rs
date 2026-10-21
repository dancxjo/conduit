//! Eighth-pixel geometry; fixed arithmetic, no raster-sized allocation.
use super::Icon;
const fn line(x: i64, y: i64, ax: i64, ay: i64, bx: i64, by: i64) -> bool {
    let dx = bx - ax;
    let dy = by - ay;
    let px = x - ax;
    let py = y - ay;
    let length = dx * dx + dy * dy;
    let dot = px * dx + py * dy;
    let cross = px * dy - py * dx;
    dot >= 0 && dot <= length && cross * cross <= length * 64
}

const fn ring(x: i64, y: i64, cx: i64, cy: i64, radius: i64) -> bool {
    let dx = x - cx;
    let dy = y - cy;
    let distance = dx * dx + dy * dy;
    distance >= (radius - 8) * (radius - 8) && distance <= (radius + 8) * (radius + 8)
}

const fn frame(x: i64, y: i64, left: i64, top: i64, right: i64, bottom: i64) -> bool {
    line(x, y, left, top, right, top)
        || line(x, y, right, top, right, bottom)
        || line(x, y, right, bottom, left, bottom)
        || line(x, y, left, bottom, left, top)
}

const fn letter_a(x: i64, y: i64) -> bool {
    line(x, y, 40, 160, 96, 24) || line(x, y, 96, 24, 152, 160) || line(x, y, 64, 112, 128, 112)
}

pub(super) const fn ink(icon: Icon, x: i64, y: i64) -> bool {
    use Icon::*;
    match icon {
        Clock => {
            ring(x, y, 96, 96, 72) || line(x, y, 96, 48, 96, 96) || line(x, y, 96, 96, 136, 120)
        }
        Repeat2 => {
            line(x, y, 40, 48, 152, 48)
                || line(x, y, 152, 48, 152, 88)
                || line(x, y, 152, 48, 128, 24)
                || line(x, y, 40, 144, 152, 144)
                || line(x, y, 40, 104, 40, 144)
                || line(x, y, 40, 144, 64, 168)
        }
        Face => {
            frame(x, y, 24, 24, 168, 128)
                || line(x, y, 96, 128, 96, 168)
                || line(x, y, 56, 168, 136, 168)
        }
        Type => {
            line(x, y, 32, 32, 160, 32)
                || line(x, y, 96, 32, 96, 160)
                || line(x, y, 64, 160, 128, 160)
        }
        CaseUpper => letter_a(x, y),
        Combine => {
            line(x, y, 24, 40, 64, 40)
                || line(x, y, 64, 40, 104, 96)
                || line(x, y, 24, 152, 64, 152)
                || line(x, y, 64, 152, 104, 96)
                || line(x, y, 104, 96, 168, 96)
                || line(x, y, 136, 64, 168, 96)
                || line(x, y, 136, 128, 168, 96)
        }
        Tally5 => {
            line(x, y, 32, 32, 32, 160)
                || line(x, y, 72, 32, 72, 160)
                || line(x, y, 112, 32, 112, 160)
                || line(x, y, 152, 32, 152, 160)
                || line(x, y, 16, 136, 168, 56)
        }
        ChartColumnsIncreasing => {
            frame(x, y, 24, 120, 56, 168)
                || frame(x, y, 80, 80, 112, 168)
                || frame(x, y, 136, 24, 168, 168)
        }
        FileOutput => {
            line(x, y, 32, 24, 112, 24)
                || line(x, y, 32, 24, 32, 168)
                || line(x, y, 32, 168, 112, 168)
                || line(x, y, 112, 24, 144, 56)
                || line(x, y, 144, 56, 144, 72)
                || line(x, y, 80, 112, 176, 112)
                || line(x, y, 144, 80, 176, 112)
                || line(x, y, 144, 144, 176, 112)
        }
        Keyboard => {
            frame(x, y, 16, 40, 176, 152)
                || line(x, y, 56, 128, 136, 128)
                || (y >= 64
                    && y <= 96
                    && (y + 4) % 32 <= 8
                    && x >= 36
                    && x <= 156
                    && (x + 28) % 32 <= 8)
        }
        GenericGear => {
            ring(x, y, 96, 96, 64)
                || ring(x, y, 96, 96, 24)
                || line(x, y, 96, 8, 96, 40)
                || line(x, y, 96, 152, 96, 184)
                || line(x, y, 8, 96, 40, 96)
                || line(x, y, 152, 96, 184, 96)
        }
        Body => ring(x, y, 96, 48, 24) || frame(x, y, 40, 104, 152, 168),
        Wake => {
            ring(x, y, 96, 96, 48)
                || line(x, y, 96, 8, 96, 32)
                || line(x, y, 96, 160, 96, 184)
                || line(x, y, 8, 96, 32, 96)
                || line(x, y, 160, 96, 184, 96)
        }
        Plan => {
            frame(x, y, 24, 24, 64, 64)
                || frame(x, y, 128, 128, 168, 168)
                || line(x, y, 64, 48, 144, 48)
                || line(x, y, 144, 48, 144, 128)
        }
        Play => x >= 48 && x <= 160 && y >= 24 && y <= 168 && (y - 96).abs() * 14 <= (160 - x) * 9,
        Host => {
            frame(x, y, 24, 24, 168, 128)
                || line(x, y, 96, 128, 96, 168)
                || line(x, y, 56, 168, 136, 168)
        }
        Port => {
            ring(x, y, 64, 96, 40)
                || line(x, y, 112, 96, 176, 96)
                || line(x, y, 176, 96, 144, 64)
                || line(x, y, 176, 96, 144, 128)
        }
        Line => {
            frame(x, y, 8, 104, 56, 152)
                || frame(x, y, 136, 40, 184, 88)
                || line(x, y, 56, 128, 136, 64)
        }
        Status => ring(x, y, 96, 96, 80) || line(x, y, 96, 80, 96, 144) || ring(x, y, 96, 48, 8),
        Warning => {
            line(x, y, 96, 16, 16, 168)
                || line(x, y, 16, 168, 176, 168)
                || line(x, y, 176, 168, 96, 16)
                || line(x, y, 96, 64, 96, 112)
                || ring(x, y, 96, 144, 8)
        }
        Close => line(x, y, 40, 40, 152, 152) || line(x, y, 152, 40, 40, 152),
        Back => {
            line(x, y, 24, 96, 168, 96) || line(x, y, 24, 96, 80, 40) || line(x, y, 24, 96, 80, 152)
        }
        Confirm => line(x, y, 24, 96, 72, 144) || line(x, y, 72, 144, 168, 40),
    }
}
