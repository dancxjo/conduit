//! Built-in options are complete checked Look resources, independent of Face.
use conduit_face::*;
use conduit_plot::rust_binding::NativeRustBinding;

fn luminance(color: &ThemeColor) -> f64 {
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.red()) + 0.7152 * channel(color.green()) + 0.0722 * channel(color.blue())
}
fn contrast(left: &ThemeColor, right: &ThemeColor) -> f64 {
    let (left, right) = (luminance(left), luminance(right));
    (left.max(right) + 0.05) / (left.min(right) + 0.05)
}

#[test]
fn built_in_options_round_trip_and_keep_readable_text_and_focus() {
    for (look, identity) in [
        (phosphor_look().unwrap(), PHOSPHOR_LOOK_ID),
        (editorial_look().unwrap(), EDITORIAL_LOOK_ID),
        (engineering_look().unwrap(), ENGINEERING_LOOK_ID),
        (
            field_instrument_day_look().unwrap(),
            FIELD_INSTRUMENT_DAY_LOOK_ID,
        ),
        (
            field_instrument_night_look().unwrap(),
            FIELD_INSTRUMENT_NIGHT_LOOK_ID,
        ),
        (
            navy_look(VisualDensity::Compact, VisualTypography::Editorial).unwrap(),
            navy_look_id(VisualDensity::Compact, VisualTypography::Editorial),
        ),
        (
            navy_look(VisualDensity::Spacious, VisualTypography::Engineering).unwrap(),
            navy_look_id(VisualDensity::Spacious, VisualTypography::Engineering),
        ),
    ] {
        assert_eq!(look.identity(), identity);
        let bytes = look.clone().encode().unwrap();
        assert!(bytes.len() <= MAX_LOOK_BYTES);
        assert_eq!(LookInfo::decode(&bytes).unwrap(), look);
        let LookVisualFacetOption::Some(visual) = look.visual() else {
            panic!("visual facet")
        };
        assert_eq!(visual.expression().identity(), identity);
        let tokens = visual.tokens();
        for surface in [
            tokens.background(),
            tokens.surface(),
            tokens.reading_paper(),
        ] {
            for text in [
                tokens.text_primary(),
                tokens.text_secondary(),
                tokens.emphasis(),
            ] {
                assert!(
                    contrast(text, surface) >= 4.5,
                    "{identity}: text contrast {}",
                    contrast(text, surface)
                );
            }
            assert!(
                contrast(tokens.focus(), surface) >= 3.0,
                "{identity}: focus contrast"
            );
        }
        let admitted = admit_visual_look(
            &look,
            VisualUserConstraints {
                forced_colors: true,
                reduced_motion: true,
                minimum_text_scale_percent: 200,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap();
        assert_eq!(admitted.selected.identity(), identity);
    }
}
