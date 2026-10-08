//! Generated reconstruction of checked record laws.

use crate::prelude::*;
use core::fmt::Write;

pub(super) fn emit(out: &mut String, invariants: &[crate::PortableExpressionProgram]) {
    writeln!(out, "    fn invariants() -> Result<Vec<conduit_plot::PortableExpressionProgram>, NativeBindingRefusal> {{")
        .expect("String writing is infallible");
    writeln!(out, "        Ok(vec![").expect("String writing is infallible");
    for invariant in invariants {
        let encoded = invariant
            .canonical_bytes()
            .expect("checked invariant has canonical bytes");
        writeln!(out, "            conduit_plot::PortableExpressionProgram::from_canonical_bytes(&{encoded:?}).map_err(NativeBindingRefusal::InvalidInvariantProgram)?,")
            .expect("String writing is infallible");
    }
    writeln!(out, "        ])\n    }}").expect("String writing is infallible");
}

pub(super) fn emit_table(
    out: &mut String,
    rust_name: &str,
    invariants: &[crate::PortableExpressionProgram],
) {
    writeln!(
        out,
        "#[allow(non_upper_case_globals)]\nstatic {rust_name}_PREPARED_NATIVE_LAWS: &[&[u8]] = &["
    )
    .unwrap();
    for invariant in invariants {
        writeln!(
            out,
            "    &{:?},",
            invariant
                .canonical_bytes()
                .expect("checked invariant has canonical bytes")
        )
        .unwrap();
    }
    writeln!(out, "];\n").unwrap();
}

pub(super) fn emit_from_table(out: &mut String, rust_name: &str) {
    writeln!(out, "    fn invariants() -> Result<Vec<conduit_plot::PortableExpressionProgram>, NativeBindingRefusal> {{\n        let mut laws = Vec::with_capacity({rust_name}_PREPARED_NATIVE_LAWS.len());\n        for encoded in {rust_name}_PREPARED_NATIVE_LAWS {{\n            laws.push(conduit_plot::PortableExpressionProgram::from_canonical_bytes(encoded).map_err(NativeBindingRefusal::InvalidInvariantProgram)?);\n        }}\n        Ok(laws)\n    }}").unwrap();
}
