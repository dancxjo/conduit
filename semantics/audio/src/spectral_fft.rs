//! Allocation-free radix-2 transform over caller-owned finite scratch.
#[derive(Clone, Copy, Default)]
pub(crate) struct Complex {
    pub real: f64,
    pub imaginary: f64,
}

pub(crate) fn forward(values: &mut [Complex]) -> Result<(), ()> {
    let n = values.len();
    if !(16..=1024).contains(&n) || !n.is_power_of_two() {
        return Err(());
    }
    let mut reversed = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while reversed & bit != 0 {
            reversed ^= bit;
            bit >>= 1;
        }
        reversed ^= bit;
        if i < reversed {
            values.swap(i, reversed);
        }
    }
    let mut width = 2;
    while width <= n {
        let half = width / 2;
        for j in 0..half {
            let angle = -2.0 * core::f64::consts::PI * j as f64 / width as f64;
            let cosine = libm::cos(angle);
            let sine = libm::sin(angle);
            for start in (0..n).step_by(width) {
                let left = values[start + j];
                let right = values[start + j + half];
                let real = cosine * right.real - sine * right.imaginary;
                let imaginary = sine * right.real + cosine * right.imaginary;
                let a = Complex {
                    real: left.real + real,
                    imaginary: left.imaginary + imaginary,
                };
                let b = Complex {
                    real: left.real - real,
                    imaginary: left.imaginary - imaginary,
                };
                if ![a.real, a.imaginary, b.real, b.imaginary]
                    .iter()
                    .all(|x| x.is_finite())
                {
                    return Err(());
                }
                values[start + j] = a;
                values[start + j + half] = b;
            }
        }
        width *= 2;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arithmetic_overflow_and_unsupported_length_refuse() {
        let mut values = [Complex {
            real: f64::MAX,
            imaginary: 0.0,
        }; 16];
        assert!(forward(&mut values).is_err());
        assert!(forward(&mut values[..15]).is_err());
    }
}
