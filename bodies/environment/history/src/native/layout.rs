use conduit_face::GraphicsError;

pub(super) struct ChartLayout {
    pub left: i16,
    pub right: i16,
    pub top: [i16; 2],
    pub bottom: [i16; 2],
    pub heading: [i16; 2],
}
impl ChartLayout {
    pub fn new(width: u16, height: u16) -> Result<Self, GraphicsError> {
        if width < 320 || height < 360 || width > i16::MAX as u16 || height > i16::MAX as u16 {
            return Err(GraphicsError::InvalidGeometry);
        }
        let panel = (height as i16 - 88) / 2;
        Ok(Self {
            left: if width >= 900 { 100 } else { 84 },
            right: width as i16 - 28,
            heading: [12, 12 + panel],
            top: [58, 58 + panel],
            bottom: [panel - 10, panel * 2 - 10],
        })
    }
    pub fn x(&self, time: u64, first: u64, last: u64) -> i16 {
        if last == first {
            self.left
        } else {
            self.left
                + (u128::from(time.saturating_sub(first).min(last - first))
                    * (self.right - self.left) as u128
                    / u128::from(last - first)) as i16
        }
    }
    pub fn y(&self, value: impl Into<i128>, range: NiceScale, panel: usize) -> i16 {
        let offset = (value.into().clamp(range.min, range.max) - range.min)
            * i128::from(self.bottom[panel] - self.top[panel])
            / (range.max - range.min);
        self.bottom[panel] - offset as i16
    }
}
#[derive(Clone, Copy)]
pub(super) struct NiceScale {
    pub min: i128,
    pub max: i128,
}
impl NiceScale {
    pub fn from_values(min: i64, max: i64) -> Result<Self, GraphicsError> {
        let (min, max) = if min == max {
            (i128::from(min) - 500, i128::from(max) + 500)
        } else {
            (i128::from(min), i128::from(max))
        };
        // Labels show tenths, so the step must never imply finer precision.
        let desired = (((max - min) + 1) / 2).max(100);
        let mut power = 1i128;
        while power * 10 < desired {
            power *= 10;
        }
        let mut step = [1, 2, 5, 10]
            .into_iter()
            .map(|n| n * power)
            .find(|s| *s >= desired)
            .ok_or(GraphicsError::InvalidGeometry)?;
        let (lower, upper) = loop {
            let lower = min.div_euclid(step) * step;
            let upper = lower + 2 * step;
            if upper >= max {
                break (lower, upper);
            }
            let mut next_power = 1i128;
            while next_power * 10 <= step {
                next_power *= 10;
            }
            step = [1, 2, 5, 10]
                .into_iter()
                .map(|n| n * next_power)
                .find(|s| *s > step)
                .ok_or(GraphicsError::InvalidGeometry)?;
        };
        Ok(Self {
            min: lower,
            max: upper,
        })
    }
    pub fn midpoint(self) -> i128 {
        (self.min + self.max) / 2
    }
}
pub(super) fn decimal(value: impl Into<i128>) -> alloc::string::String {
    let value = value.into();
    let rounded = (value.abs() + 50) / 100;
    alloc::format!(
        "{}{}.{:01}",
        if value < 0 && rounded != 0 { "-" } else { "" },
        rounded / 10,
        rounded % 10
    )
}
pub(super) fn elapsed(time: u64) -> alloc::string::String {
    let s = time / 1000;
    alloc::format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scales_are_nice_finite_and_cover_actual_flat_negative_and_zero_values() {
        for (lo, hi) in [
            (64040, 64760),
            (0, 0),
            (-40300, -40100),
            (-1, 1),
            (64500, 64500),
            (i64::MAX, i64::MAX),
            (i64::MIN, i64::MIN),
        ] {
            let scale = NiceScale::from_values(lo, hi).unwrap();
            assert!(
                scale.min <= i128::from(lo) && scale.max >= i128::from(hi) && scale.min < scale.max
            );
            assert!(scale.midpoint() >= scale.min && scale.midpoint() <= scale.max);
        }
        let scale = NiceScale::from_values(64040, 64760).unwrap();
        assert_eq!(
            (scale.min, scale.midpoint(), scale.max),
            (64000, 64500, 65000)
        );
        let flat = NiceScale::from_values(64760, 64760).unwrap();
        assert_eq!((flat.min, flat.midpoint(), flat.max), (64000, 65000, 66000));
        let near_zero = NiceScale::from_values(-1, 1).unwrap();
        assert_eq!(
            (near_zero.min, near_zero.midpoint(), near_zero.max),
            (-100, 0, 100)
        );
        assert_eq!(decimal(-1), "0.0");
        assert_eq!(decimal(-40550), "-40.6");
    }
}
