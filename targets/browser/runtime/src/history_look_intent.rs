//! Finite user intent becomes canonical Look bytes independently of Face/Plan.
use conduit_face::*;
use conduit_plot::rust_binding::NativeRustBinding;
use serde::Deserialize;
use std::cell::RefCell;

const INTENT_CAPACITY: usize = 256;
thread_local! {
    static INPUT: RefCell<[u8; INTENT_CAPACITY]> = const { RefCell::new([0; INTENT_CAPACITY]) };
    static PRIOR: RefCell<Box<[u8]>> = RefCell::new(vec![0; MAX_LOOK_BYTES].into_boxed_slice());
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    unit: Unit,
    density: Density,
    palette: Palette,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Unit {
    Celsius,
    Kelvin,
    Fahrenheit,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Density {
    Compact,
    Spacious,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Palette {
    Editorial,
    Phosphor,
}

fn construct(intent: &[u8], prior: &[u8]) -> Result<Vec<u8>, ()> {
    if intent.is_empty() || intent.len() > INTENT_CAPACITY || prior.len() > MAX_LOOK_BYTES {
        return Err(());
    }
    let intent: Intent = serde_json::from_slice(intent).map_err(|_| ())?;
    let base = if prior.is_empty() {
        editorial_look().map_err(|_| ())?
    } else {
        LookInfo::decode(prior).map_err(|_| ())?
    };
    let palette = match intent.palette {
        Palette::Editorial => VisualPalettePreference::Editorial,
        Palette::Phosphor => VisualPalettePreference::Phosphor,
    };
    let canonical = match palette {
        VisualPalettePreference::Editorial => editorial_look(),
        VisualPalettePreference::Phosphor => phosphor_look(),
    }
    .map_err(|_| ())?;
    let LookVisualFacetOption::Some(canonical_visual) = canonical.visual() else {
        return Err(());
    };
    let prior_visual = match base.visual() {
        LookVisualFacetOption::Some(value) => value,
        LookVisualFacetOption::None => canonical_visual,
    };
    let changed = prior_visual.expression().palette() != &palette;
    let identity = if changed || matches!(base.visual(), LookVisualFacetOption::None) {
        canonical.identity()
    } else {
        base.identity()
    };
    let expression = prior_visual.expression();
    let visual = LookVisualFacetOption::some(
        VisualExpressionInfo::new(
            match intent.density {
                Density::Compact => VisualDensityPreference::Compact,
                Density::Spacious => VisualDensityPreference::Spacious,
            },
            identity.into(),
            *expression.motion(),
            palette,
            *expression.text_scale_percent(),
            *expression.typography(),
            *expression.version(),
        )
        .map_err(|_| ())?,
        if changed {
            canonical_visual.tokens().clone()
        } else {
            prior_visual.tokens().clone()
        },
    )
    .map_err(|_| ())?;
    let quantity = LookQuantityFacetOption::some(match intent.unit {
        Unit::Celsius => LookTemperatureUnit::Celsius,
        Unit::Kelvin => LookTemperatureUnit::Kelvin,
        Unit::Fahrenheit => LookTemperatureUnit::Fahrenheit,
    })
    .map_err(|_| ())?;
    let bytes = LookInfo::new(
        base.generative().clone(),
        identity.into(),
        quantity,
        base.spoken().clone(),
        3,
        visual,
    )
    .map_err(|_| ())?
    .encode()
    .map_err(|_| ())?;
    if bytes.is_empty() || bytes.len() > MAX_LOOK_BYTES {
        return Err(());
    }
    Ok(bytes)
}
#[no_mangle]
pub extern "C" fn conduit_browser_history_look_intent_input_ptr() -> usize {
    INPUT.with(|v| v.borrow_mut().as_mut_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_browser_history_look_intent_input_capacity() -> usize {
    INTENT_CAPACITY
}
#[no_mangle]
pub extern "C" fn conduit_browser_history_look_intent_prior_ptr() -> usize {
    PRIOR.with(|v| v.borrow_mut().as_mut_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_browser_history_look_intent_prior_capacity() -> usize {
    MAX_LOOK_BYTES
}
#[no_mangle]
pub extern "C" fn conduit_browser_history_look_intent_output_ptr() -> usize {
    OUTPUT.with(|v| v.borrow().as_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_browser_history_look_intent_output_len() -> usize {
    OUTPUT.with(|v| v.borrow().len())
}
#[no_mangle]
pub extern "C" fn conduit_browser_history_look_intent_output_capacity() -> usize {
    MAX_LOOK_BYTES
}
#[no_mangle]
pub extern "C" fn conduit_browser_history_look_intent(intent_len: usize, prior_len: usize) -> i32 {
    OUTPUT.with(|v| v.borrow_mut().clear());
    let result = if intent_len == 0 || intent_len > INTENT_CAPACITY || prior_len > MAX_LOOK_BYTES {
        Err(())
    } else {
        INPUT.with(|input| {
            PRIOR.with(|prior| {
                construct(&input.borrow()[..intent_len], &prior.borrow()[..prior_len])
            })
        })
    };
    INPUT.with(|v| v.borrow_mut().fill(0));
    PRIOR.with(|v| v.borrow_mut().fill(0));
    match result {
        Ok(bytes) => {
            OUTPUT.with(|v| *v.borrow_mut() = bytes);
            0
        }
        Err(()) => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_units_preserve_independent_facets() {
        let mut prior = editorial_look().unwrap();
        for (unit, expected) in [
            ("celsius", LookTemperatureUnit::Celsius),
            ("kelvin", LookTemperatureUnit::Kelvin),
            ("fahrenheit", LookTemperatureUnit::Fahrenheit),
        ] {
            for palette in ["editorial", "phosphor"] {
                for density in ["compact", "spacious"] {
                    let intent = format!(
                        r#"{{"unit":"{unit}","density":"{density}","palette":"{palette}"}}"#
                    );
                    let look = LookInfo::decode(
                        &construct(intent.as_bytes(), &prior.clone().encode().unwrap()).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(look.spoken(), prior.spoken());
                    assert_eq!(look.generative(), prior.generative());
                    assert!(
                        matches!(look.quantity(), LookQuantityFacetOption::Some(q) if *q.temperature_unit() == expected)
                    );
                    let LookVisualFacetOption::Some(v) = look.visual() else {
                        panic!("visual");
                    };
                    assert_eq!(v.expression().identity(), look.identity());
                    assert_eq!(
                        *v.expression().density(),
                        if density == "compact" {
                            VisualDensityPreference::Compact
                        } else {
                            VisualDensityPreference::Spacious
                        }
                    );
                    prior = look;
                }
            }
        }
    }
    #[test]
    fn refuses_unknown_malformed_or_unbounded_intent_and_prior() {
        let valid = br#"{"unit":"kelvin","density":"compact","palette":"editorial"}"#;
        assert!(construct(valid, &[]).is_ok());
        for bad in [
            br#"{"unit":"rankine","density":"compact","palette":"editorial"}"#.as_slice(),
            br#"{"unit":"kelvin","density":"compact","palette":"editorial","extra":0}"#,
            b"{}",
            b"not json",
        ] {
            assert!(construct(bad, &[]).is_err());
        }
        assert!(construct(&[b' '; INTENT_CAPACITY + 1], &[]).is_err());
        assert!(construct(valid, b"not canonical Look bytes").is_err());
        assert!(construct(valid, &vec![0; MAX_LOOK_BYTES + 1]).is_err());
    }
    #[test]
    fn abi_refusal_clears_prior_output_and_inputs() {
        let valid = br#"{"unit":"kelvin","density":"compact","palette":"editorial"}"#;
        INPUT.with(|v| v.borrow_mut()[..valid.len()].copy_from_slice(valid));
        assert_eq!(conduit_browser_history_look_intent(valid.len(), 0), 0);
        assert!(conduit_browser_history_look_intent_output_len() > 0);
        assert_eq!(
            conduit_browser_history_look_intent(INTENT_CAPACITY + 1, 0),
            -1
        );
        assert_eq!(conduit_browser_history_look_intent_output_len(), 0);
        INPUT.with(|v| assert!(v.borrow().iter().all(|b| *b == 0)));
        PRIOR.with(|v| assert!(v.borrow().iter().all(|b| *b == 0)));
    }
}
