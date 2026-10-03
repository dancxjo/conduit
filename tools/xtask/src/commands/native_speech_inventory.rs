//! Linked function/prologue inventory. This deliberately proves no full stack bound.
use serde_json::{json, Value};
use std::collections::BTreeMap;

struct Function {
    address: u64,
    end: u64,
    symbol: String,
    instructions: Vec<(u64, String)>,
}

pub(super) fn report(assembly: &str) -> Result<Value, &'static str> {
    if !assembly.contains("file format elf32-littlearm") {
        return Err("function inventory supports only 32-bit ARM ELF");
    }
    if !assembly.contains("SYMBOL TABLE:") {
        return Err("function inventory requires the retained ELF symbol table");
    }
    let mut functions = BTreeMap::new();
    for line in assembly
        .lines()
        .take_while(|line| !line.starts_with("Disassembly of section"))
    {
        let fields: Vec<_> = line.split_whitespace().collect();
        let Some(flag) = fields.iter().position(|field| *field == "F") else {
            continue;
        };
        if fields.get(flag + 1) != Some(&".text") {
            continue;
        }
        let address = u64::from_str_radix(fields[0], 16).map_err(|_| "invalid function address")?;
        let size = u64::from_str_radix(fields.get(flag + 2).ok_or("missing function size")?, 16)
            .map_err(|_| "invalid function size")?;
        if size == 0 {
            return Err("function size is zero");
        }
        let symbol = fields.last().ok_or("missing function symbol")?.to_string();
        if functions
            .insert(
                address,
                Function {
                    address,
                    end: address.checked_add(size).ok_or("function range overflow")?,
                    symbol,
                    instructions: Vec::new(),
                },
            )
            .is_some()
        {
            return Err("aliased function entries require explicit inventory support");
        }
    }
    if functions.is_empty() {
        return Err("ELF has no supported text function symbols");
    }
    let mut previous_end = 0;
    for function in functions.values() {
        if function.address < previous_end {
            return Err("overlapping function ranges");
        }
        previous_end = function.end;
    }
    for line in assembly.lines() {
        let Some((address, body)) = line.trim().split_once(':') else {
            continue;
        };
        let Ok(address) = u64::from_str_radix(address, 16) else {
            continue;
        };
        let Some((_, function)) = functions.range_mut(..=address).next_back() else {
            continue;
        };
        if address >= function.end {
            continue;
        }
        // Literal pools have a .word/.short/etc directive after their raw bytes.
        // They are data inside a function's range, not instruction/control sites.
        if body.split_whitespace().any(|word| word.starts_with('.')) {
            continue;
        }
        let opcode = body
            .split_whitespace()
            .next()
            .ok_or("empty decoded instruction")?;
        // Even-width hexadecimal tokens are raw bytes/words, not mnemonics.
        // Alphabetic hex such as `ff` must not become an unknown prologue gap.
        let raw_hex =
            matches!(opcode.len(), 2 | 4 | 8) && opcode.chars().all(|c| c.is_ascii_hexdigit());
        if raw_hex || !opcode.chars().all(|c| c.is_ascii_alphabetic() || c == '.') {
            return Err("undecoded bytes inside a function require explicit inventory support");
        }
        function
            .instructions
            .push((address, body.trim().to_owned()));
    }
    let mut entries = Vec::new();
    let mut controls = Vec::new();
    let mut calls = Vec::new();
    for function in functions.values() {
        if function.instructions.first().map(|item| item.0) != Some(function.address) {
            return Err("function entry is not a decoded instruction");
        }
        let body = function
            .instructions
            .iter()
            .map(|(address, text)| format!("{address:x}: {text}\n"))
            .collect::<String>();
        let prologue = super::entry_prologue(&format!(
            "probe: file format elf32-littlearm\n{:x} <_start>:\n{body}",
            function.address
        ));
        entries.push(json!({
            "symbol": function.symbol, "address": format!("0x{:08x}",function.address),
            "entry_reservation_bytes": prologue.as_ref().ok(),
            "entry_inspection_gap": prologue.err(),
        }));
        for (address, text) in &function.instructions {
            let mut parts = text.splitn(2, char::is_whitespace);
            let opcode = parts.next().unwrap_or("");
            let operands = parts.next().unwrap_or("").trim();
            let first = operands.split(',').next().unwrap_or("").trim();
            if opcode == "bl" || opcode == "blx" {
                let target = operands.split_whitespace().next().unwrap_or("");
                if let Some(target) = target.strip_prefix("0x") {
                    let target = u64::from_str_radix(target, 16)
                        .map_err(|_| "invalid direct call address")?;
                    calls.push(
                        json!({"caller":function.symbol,"address":format!("0x{address:08x}"),
                        "target":format!("0x{target:08x}"),
                        "callee":functions.get(&target).map(|callee| &callee.symbol)}),
                    );
                } else {
                    controls.push(
                        json!({"function":function.symbol,"address":format!("0x{address:08x}"),
                        "instruction":text,"kind":"indirect_call"}),
                    );
                }
            } else if (opcode == "bx" && first != "lr") || first == "pc" {
                controls.push(
                    json!({"function":function.symbol,"address":format!("0x{address:08x}"),
                    "instruction":text,"kind":"computed_control"}),
                );
            }
        }
    }
    Ok(json!({
        "schema":"conduit.native-speech-stack-inventory.v1",
        "proof_class":"linked-assembly-inventory",
        "full_stack_status":"unproven",
        "gaps":["body stack adjustments and control-flow are not verified",
            "computed control targets are not resolved", "call-chain liveness is not established",
            "boot, interrupts and physical execution are excluded"],
        "functions":entries,"direct_calls":calls,"identified_computed_control_sites":controls,
    }))
}

#[cfg(test)]
mod tests {
    use super::report;
    const ELF: &str = "probe: file format elf32-littlearm\nSYMBOL TABLE:\n1000 g F .text 0010 _start\n1010 l F .text 0008 .hidden callee\n1018 l O .text 0004 DATA\nDisassembly of section .text:\n1000 <_start>:\n1000: push {r4, lr}\n1002: sub sp, #16\n1004: bl 0x1010 <callee>\n1008: mov pc, r0\n100a: 11 22 33 44 .word 0x44332211\n1010 <callee>:\n1010: push {r7, lr}\n1012: movs r0, #0\n1014: pop {r7, pc}\n1018 <DATA>:\n1018: 00 00 00 00 ....\n";
    #[test]
    fn functions_calls_and_computed_dispatch_are_separate_evidence() {
        let value = report(ELF).unwrap();
        assert_eq!(value["full_stack_status"], "unproven");
        assert_eq!(value["functions"].as_array().unwrap().len(), 2);
        assert_eq!(value["functions"][0]["entry_reservation_bytes"], 24);
        assert_eq!(value["functions"][1]["entry_reservation_bytes"], 8);
        assert_eq!(value["direct_calls"][0]["callee"], "callee");
        assert_eq!(
            value["identified_computed_control_sites"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            value["identified_computed_control_sites"][0]["address"],
            "0x00001008"
        );
    }
    #[test]
    fn unsupported_inputs_refuse_instead_of_inventing_bounds() {
        for assembly in [
            ELF.replace("elf32-littlearm", "elf64-x86-64"),
            ELF.replace("SYMBOL TABLE:", "missing"),
            ELF.replace("0008 .hidden callee", "0020 .hidden callee")
                .replace("1018 l O", "1020 l F"),
            ELF.replace("1010: push {r7, lr}", "1010: ff ff ????"),
            ELF.replace("1010: push {r7, lr}", "1010: abcd ????"),
            ELF.replace("1010: push {r7, lr}", "1010: deadbeef ????"),
        ] {
            assert!(
                report(&assembly).is_err(),
                "accepted unsupported input: {assembly}"
            );
        }
        let unknown = report(&ELF.replace("1010: push {r7, lr}", "1010: mov sp, r0")).unwrap();
        assert!(unknown["functions"][1]["entry_reservation_bytes"].is_null());
        assert!(unknown["functions"][1]["entry_inspection_gap"].is_string());
    }
}
