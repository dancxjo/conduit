use conduit_core::*;

#[test]
fn source_declared_prefix_groups_and_opt_in_are_exact() {
    for &(group, symbol, exponent, _) in BUILTIN_PREFIX_DEFINITIONS {
        let base = if group == "si" {
            Unit::Meter
        } else {
            Unit::Byte
        };
        let spelling = format!("{symbol}{}", base.symbol());
        let unit = Unit::resolve(&spelling).unwrap();
        assert_eq!(unit.symbol(), spelling);
        if group == "si" {
            assert_eq!(unit.decimal_exponent(), i16::from(exponent));
        } else {
            assert_eq!(unit.binary_exponent(), exponent as u16);
        }
        assert_eq!(Unit::decode(&unit.encode()), Ok(unit));
    }
    for spelling in ["kmin", "kkHz", "kMiB", "kpx", "µ°C"] {
        assert!(Unit::resolve(spelling).is_err(), "{spelling}");
    }
    assert_eq!(Unit::resolve("cm²").unwrap().decimal_exponent(), -4);
    assert_eq!(Unit::resolve("Qm³").unwrap().decimal_exponent(), 90);
    assert!(Unit::resolve("um")
        .unwrap()
        .same_physical_definition(Unit::resolve("µm").unwrap()));
    assert_ne!(
        Unit::resolve("um").unwrap().encode(),
        Unit::resolve("µm").unwrap().encode()
    );
}
