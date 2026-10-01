//! Generated reconstruction of checked record laws.

use crate::prelude::*;
use core::fmt::Write;

pub(super) fn emit(out: &mut String, invariants: &[crate::PortableExpressionProgram]) {
    writeln!(out, "    fn invariants() -> Result<Vec<conduit_form::PortableExpressionProgram>, NativeBindingRefusal> {{")
        .expect("String writing is infallible");
    writeln!(out, "        Ok(vec![").expect("String writing is infallible");
    for invariant in invariants {
        let encoded = invariant
            .canonical_bytes()
            .expect("checked invariant has canonical bytes");
        writeln!(out, "            conduit_form::PortableExpressionProgram::from_canonical_bytes(&{encoded:?}).map_err(NativeBindingRefusal::InvalidInvariantProgram)?,")
            .expect("String writing is infallible");
    }
    writeln!(out, "        ])\n    }}").expect("String writing is infallible");
}
