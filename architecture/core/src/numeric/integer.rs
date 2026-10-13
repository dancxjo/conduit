use super::{MAX_INTEGER_LIMBS, MAX_WORK_LIMBS, NumericRefusal};
use core::cmp::Ordering;
const BASE: u64 = 1_000_000_000;
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Magnitude {
    pub limbs: [u32; MAX_WORK_LIMBS],
    pub len: usize,
}
impl core::fmt::Debug for Magnitude {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(&self.limbs[..self.len]).finish()
    }
}
impl Magnitude {
    pub fn zero() -> Self {
        Self {
            limbs: [0; MAX_WORK_LIMBS],
            len: 0,
        }
    }
    pub fn from_u64(mut n: u64) -> Self {
        let mut out = Self::zero();
        while n != 0 {
            out.limbs[out.len] = (n % BASE) as u32;
            out.len += 1;
            n /= BASE;
        }
        out
    }
    pub fn from_u128(mut n: u128) -> Self {
        let mut out = Self::zero();
        while n != 0 {
            out.limbs[out.len] = (n % u128::from(BASE)) as u32;
            out.len += 1;
            n /= u128::from(BASE);
        }
        out
    }
    pub fn is_zero(&self) -> bool {
        self.len == 0
    }
    pub fn is_one(&self) -> bool {
        self.len == 1 && self.limbs[0] == 1
    }
    pub fn normalize(&mut self) {
        while self.len > 0 && self.limbs[self.len - 1] == 0 {
            self.len -= 1;
        }
    }
    pub fn compare(&self, other: &Self) -> Ordering {
        self.len.cmp(&other.len).then_with(|| {
            self.limbs[..self.len]
                .iter()
                .rev()
                .cmp(other.limbs[..other.len].iter().rev())
        })
    }
    pub fn add(&self, other: &Self) -> Result<Self, NumericRefusal> {
        let mut out = Self::zero();
        let mut carry = 0u64;
        out.len = self.len.max(other.len);
        for i in 0..out.len {
            let sum = u64::from(self.limbs[i]) + u64::from(other.limbs[i]) + carry;
            out.limbs[i] = (sum % BASE) as u32;
            carry = sum / BASE;
        }
        if carry != 0 {
            if out.len == MAX_WORK_LIMBS {
                return Err(NumericRefusal::IntegerBudget);
            }
            out.limbs[out.len] = carry as u32;
            out.len += 1;
        }
        Ok(out)
    }
    pub fn sub(&self, other: &Self) -> Self {
        debug_assert!(self.compare(other) != Ordering::Less);
        let mut out = self.clone();
        let mut borrow = 0i64;
        for i in 0..self.len {
            let n = i64::from(self.limbs[i]) - i64::from(other.limbs[i]) - borrow;
            if n < 0 {
                out.limbs[i] = (n + BASE as i64) as u32;
                borrow = 1;
            } else {
                out.limbs[i] = n as u32;
                borrow = 0;
            }
        }
        out.normalize();
        out
    }
    pub fn mul_small(&self, n: u32) -> Result<Self, NumericRefusal> {
        let mut out = Self::zero();
        let mut carry = 0u64;
        out.len = self.len;
        for i in 0..self.len {
            let v = u64::from(self.limbs[i]) * u64::from(n) + carry;
            out.limbs[i] = (v % BASE) as u32;
            carry = v / BASE;
        }
        if carry != 0 {
            if out.len == MAX_WORK_LIMBS {
                return Err(NumericRefusal::IntegerBudget);
            }
            out.limbs[out.len] = carry as u32;
            out.len += 1;
        }
        out.normalize();
        Ok(out)
    }
    pub fn mul(&self, other: &Self) -> Result<Self, NumericRefusal> {
        if self.is_zero() || other.is_zero() {
            return Ok(Self::zero());
        }
        if self.len + other.len > MAX_WORK_LIMBS {
            return Err(NumericRefusal::WorkBudget);
        }
        let mut out = Self::zero();
        out.len = self.len + other.len;
        for i in 0..self.len {
            let mut carry = 0u64;
            for j in 0..other.len {
                let at = i + j;
                let n = u64::from(out.limbs[at])
                    + u64::from(self.limbs[i]) * u64::from(other.limbs[j])
                    + carry;
                out.limbs[at] = (n % BASE) as u32;
                carry = n / BASE;
            }
            out.limbs[i + other.len] = carry as u32;
        }
        out.normalize();
        Ok(out)
    }
    pub fn div_rem(&self, den: &Self) -> Result<(Self, Self), NumericRefusal> {
        if den.is_zero() {
            return Err(NumericRefusal::DivisionByZero);
        }
        let mut quotient = Self::zero();
        quotient.len = self.len;
        let mut rem = Self::zero();
        for i in (0..self.len).rev() {
            if rem.len == MAX_WORK_LIMBS {
                return Err(NumericRefusal::WorkBudget);
            }
            for j in (0..rem.len).rev() {
                rem.limbs[j + 1] = rem.limbs[j];
            }
            rem.limbs[0] = self.limbs[i];
            rem.len += 1;
            rem.normalize();
            let (mut low, mut high) = (0u32, BASE as u32 - 1);
            while low < high {
                let mid = low + (high - low).div_ceil(2);
                match den.mul_small(mid) {
                    Ok(product) if product.compare(&rem) != Ordering::Greater => low = mid,
                    Ok(_) | Err(NumericRefusal::IntegerBudget) => high = mid - 1,
                    Err(reason) => return Err(reason),
                }
            }
            quotient.limbs[i] = low;
            rem = rem.sub(&den.mul_small(low)?);
        }
        quotient.normalize();
        Ok((quotient, rem))
    }
    pub fn gcd(mut a: Self, mut b: Self) -> Result<Self, NumericRefusal> {
        for _ in 0..super::MAX_NUMERIC_WORK_STEPS {
            if b.is_zero() {
                return Ok(a);
            }
            let (_, r) = a.div_rem(&b)?;
            a = b;
            b = r;
        }
        Err(NumericRefusal::WorkBudget)
    }
    pub fn exact_div(&self, other: &Self) -> Result<Self, NumericRefusal> {
        let (q, r) = self.div_rem(other)?;
        if !r.is_zero() {
            return Err(NumericRefusal::NonCanonical);
        }
        Ok(q)
    }
    pub fn parse(digits: &str) -> Result<Self, NumericRefusal> {
        if digits.is_empty() || digits.len() > MAX_INTEGER_LIMBS * 9 {
            return Err(NumericRefusal::IntegerBudget);
        }
        let mut out = Self::zero();
        for c in digits.bytes() {
            if !c.is_ascii_digit() {
                return Err(NumericRefusal::InvalidNumber);
            }
            out = out
                .mul_small(10)?
                .add(&Self::from_u64(u64::from(c - b'0')))?;
        }
        out.admit()?;
        Ok(out)
    }
    pub fn admit(&self) -> Result<(), NumericRefusal> {
        if self.len > MAX_INTEGER_LIMBS {
            Err(NumericRefusal::IntegerBudget)
        } else {
            Ok(())
        }
    }
    pub fn to_u64(&self) -> Option<u64> {
        let mut n = 0u64;
        for &d in self.limbs[..self.len].iter().rev() {
            n = n.checked_mul(BASE)?.checked_add(u64::from(d))?;
        }
        Some(n)
    }
    pub fn power10(exp: usize) -> Result<Self, NumericRefusal> {
        if exp >= MAX_INTEGER_LIMBS * 9 {
            return Err(NumericRefusal::IntegerBudget);
        }
        let mut out = Self::zero();
        out.len = exp / 9 + 1;
        out.limbs[exp / 9] = 10u32.pow((exp % 9) as u32);
        Ok(out)
    }
    pub fn encode_into(&self, out: &mut alloc::vec::Vec<u8>) {
        out.extend_from_slice(&(self.len as u16).to_le_bytes());
        for limb in &self.limbs[..self.len] {
            out.extend_from_slice(&limb.to_le_bytes());
        }
    }
    pub fn decode(cursor: &mut super::codec::Cursor<'_>) -> Result<Self, NumericRefusal> {
        let len = usize::from(cursor.u16()?);
        if len > MAX_INTEGER_LIMBS {
            return Err(NumericRefusal::IntegerBudget);
        }
        let mut out = Self::zero();
        out.len = len;
        for i in 0..len {
            out.limbs[i] = cursor.u32()?;
            if out.limbs[i] >= BASE as u32 {
                return Err(NumericRefusal::NonCanonical);
            }
        }
        if len > 0 && out.limbs[len - 1] == 0 {
            return Err(NumericRefusal::NonCanonical);
        }
        Ok(out)
    }
}
