//! Software conformance for the Mini PiTFT; physical acceptance stays separate.
use crate::process::Step;

macro_rules! test {
    ($id:literal, $description:literal, $filter:literal) => {
        Step::new(
            $id,
            $description,
            "cargo",
            &[
                "test",
                "--locked",
                "-p",
                "conduitos",
                "--lib",
                "--features",
                "native-face-raster",
                $filter,
            ],
        )
    };
}

pub const STEPS: &[Step] = &[
    test!(
        "minipitft.kernel",
        "Prove the checked display loop and CPU frame through scripted hardware",
        "minipitft"
    ),
    test!(
        "minipitft.protocol",
        "Prove exact checked ST7789 commands, timing and fault handling",
        "st7789"
    ),
    test!(
        "minipitft.buttons",
        "Prove checked two-button debounce and polling contracts",
        "two_button"
    ),
    test!(
        "minipitft.led",
        "Prove diagnostic LED possession and loss",
        "armv6_boot_indicator"
    ),
    test!(
        "minipitft.chunks",
        "Prove indexed finite frame encoding",
        "frame_chunks"
    ),
    test!(
        "minipitft.pixels",
        "Prove RGB565 byte order and orientation",
        "rgb565"
    ),
    test!(
        "minipitft.spi",
        "Prove bounded SPI operations and native possession",
        "spi_base"
    ),
    test!(
        "minipitft.digital",
        "Prove bounded digital operations and native possession",
        "digital_base"
    ),
];
