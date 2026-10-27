//! Allocation-free structural validation of trusted Make material.
use super::super::scalar_tape::{Operation as S, ScalarTape, ScratchProfile};
use super::super::structured::tape::Operation as T;
use super::super::*;
use super::wire::*;
impl Reader {
    pub(super) fn scan_evaluator(
        &mut self,
        depth: usize,
        budget: &mut usize,
    ) -> Result<usize, Refusal> {
        if depth > 32 {
            return Err(Refusal::InvalidProgram);
        }
        let bound = self.opt()?;
        if bound.is_some_and(|n| n > 65536) {
            return Err(Refusal::InvalidProgram);
        }
        let output = self.n()? as usize;
        Self::charge(budget, output)?;
        match self.b()? {
            0 => {
                kind(self.b()?)?;
                let b = self.opt()?;
                if b != bound {
                    return Err(Refusal::InvalidProgram);
                }
            }
            1 => {
                let schema = self.blob()?;
                if schema.is_empty() || schema.len() > 65536 {
                    return Err(Refusal::InvalidProgram);
                }
                if self.opt() != Ok(bound) {
                    return Err(Refusal::InvalidProgram);
                }
            }
            _ => return Err(Refusal::InvalidProgram),
        }
        match self.b()? {
            0 => {
                self.scalar()?;
            }
            1 => self.scan_structured(depth + 1, budget, output)?,
            2 => {
                let k = kind(self.b()?)?;
                let b = self.blob()?;
                conduit_core::validate_primitive_info(kind_name(k), b)
                    .map_err(|_| Refusal::InvalidProgram)?;
                if b.len() > output {
                    return Err(Refusal::InvalidProgram);
                }
            }
            _ => return Err(Refusal::InvalidProgram),
        }
        Ok(output)
    }
    pub(super) fn charge(budget: &mut usize, n: usize) -> Result<(), Refusal> {
        *budget = budget.checked_add(n).ok_or(Refusal::InvalidProgram)?;
        if n > 65536 || *budget > MAX {
            return Err(Refusal::InvalidProgram);
        }
        Ok(())
    }
    pub(super) fn scan_structured(
        &mut self,
        depth: usize,
        budget: &mut usize,
        output: usize,
    ) -> Result<(), Refusal> {
        if depth > 32 {
            return Err(Refusal::InvalidProgram);
        }
        let prefix = self.blob()?;
        if prefix.is_empty() || prefix.len() > 65536 {
            return Err(Refusal::InvalidProgram);
        }
        match self.b()? {
            0 => {
                let code = self.blob()?;
                let constants = self.blob()?;
                let n = self.n()? as usize;
                if code.len() % 12 != 0
                    || code.len() / 12 > 512
                    || n > 128
                    || constants.len() > 65536
                {
                    return Err(Refusal::InvalidProgram);
                }
                for _ in 0..n {
                    self.scan_child(depth + 1, budget)?
                }
                for pc in 0..code.len() / 12 {
                    match structured_instruction(code, pc)? {
                        T::Bytes { start, length } => {
                            constants
                                .get(
                                    start as usize
                                        ..(start as usize)
                                            .checked_add(length as usize)
                                            .ok_or(Refusal::InvalidProgram)?,
                                )
                                .ok_or(Refusal::InvalidProgram)?;
                        }
                        T::InputNode { start, length } => {
                            if start
                                .checked_add(length)
                                .is_none_or(|end| end > self.input_limit)
                            {
                                return Err(Refusal::InvalidProgram);
                            };
                        }
                        T::Value(v) => {
                            if v as usize >= n {
                                return Err(Refusal::InvalidProgram);
                            }
                        }
                        T::BranchFalse { value, target } => {
                            if value as usize >= n
                                || target as usize <= pc
                                || target as usize > code.len() / 12
                            {
                                return Err(Refusal::InvalidProgram);
                            }
                        }
                        T::Jump(target) => {
                            if target as usize <= pc || target as usize > code.len() / 12 {
                                return Err(Refusal::InvalidProgram);
                            }
                        }
                    }
                }
            }
            1 => (),
            2 => {
                if self.blob()?.len() > output {
                    return Err(Refusal::InvalidProgram);
                }
            }
            3 => self.scan_selection(depth + 1, budget)?,
            _ => return Err(Refusal::InvalidProgram),
        }
        Ok(())
    }
    pub(super) fn scan_child(&mut self, depth: usize, budget: &mut usize) -> Result<(), Refusal> {
        if depth > 32 || self.b()? > 1 {
            return Err(Refusal::InvalidProgram);
        }
        match self.b()? {
            0 => {
                self.blob()?;
            }
            1 => {
                self.scan_evaluator(depth + 1, budget)?;
            }
            2 => {
                let out = self.n()? as usize;
                Self::charge(budget, out)?;
                let k = self.b()?;
                if k != 255 {
                    kind(k)?;
                }
                self.scalar()?;
            }
            _ => return Err(Refusal::InvalidProgram),
        }
        Ok(())
    }
    pub(super) fn scan_selection(
        &mut self,
        depth: usize,
        budget: &mut usize,
    ) -> Result<(), Refusal> {
        if depth > 32 {
            return Err(Refusal::InvalidProgram);
        }
        if self.flag()? {
            self.scan_evaluator(depth + 1, budget)?;
        }
        let n = self.n()? as usize;
        if n > 128 {
            return Err(Refusal::InvalidProgram);
        }
        for _ in 0..n {
            match self.b()? {
                0 | 2 => {
                    let s =
                        core::str::from_utf8(self.blob()?).map_err(|_| Refusal::InvalidProgram)?;
                    if s.is_empty() || s.len() > 256 {
                        return Err(Refusal::InvalidProgram);
                    }
                }
                1 => {
                    u16::try_from(self.n()?).map_err(|_| Refusal::InvalidProgram)?;
                }
                _ => return Err(Refusal::InvalidProgram),
            }
        }
        self.blob()?;
        let cap = self.n()? as usize;
        Self::charge(budget, cap)?;
        self.flag()?;
        if self.flag()? {
            let a = self.n()?;
            let b = self.n()?;
            if a.checked_add(b).is_none_or(|end| end > self.input_limit) {
                return Err(Refusal::InvalidProgram);
            };
        }
        Ok(())
    }

    pub(super) fn scalar(&mut self) -> Result<ScalarTape, Refusal> {
        let result = self.n()? as usize;
        let records = self.blob()?;
        let literals = self.blob()?;
        let count = records.len() / 24;
        if records.len() % 24 != 0 || count == 0 || count > 128 || result >= count {
            return Err(Refusal::InvalidProgram);
        }
        for pc in 0..count {
            let i = scalar_instruction(records, literals, pc)?;
            if i.destination > pc {
                return Err(Refusal::InvalidProgram);
            }
            let prior = |n: usize| {
                if n < pc {
                    Ok(())
                } else {
                    Err(Refusal::InvalidProgram)
                }
            };
            match i.operation {
                S::Input => (),
                S::Literal(b) => {
                    if b.len() > 16 {
                        return Err(Refusal::InvalidProgram);
                    }
                }
                S::ProjectedInput { start, length } => {
                    if start
                        .checked_add(length)
                        .is_none_or(|end| end > self.input_limit)
                    {
                        return Err(Refusal::InvalidProgram);
                    }
                }
                S::Unary(_, a) | S::Widen(a) | S::Move(a) => prior(a)?,
                S::Binary(_, false, a, b) => {
                    prior(a)?;
                    prior(b)?
                }
                S::Binary(_, true, ..) => return Err(Refusal::InvalidProgram),
                S::BranchFalse(a, target) => {
                    prior(a)?;
                    if target <= pc || target > count {
                        return Err(Refusal::InvalidProgram);
                    }
                }
                S::Jump(target) => {
                    if target <= pc || target > count {
                        return Err(Refusal::InvalidProgram);
                    }
                }
            }
        }
        Ok(ScalarTape {
            instructions: Vec::new(),
            linked: Some((records, literals)),
            result,
            scratch: ScratchProfile::prepare(count),
        })
    }
}
