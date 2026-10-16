//! Exact inverse Angle Source, codec, bounded projection and installed kernel proof.
use crate::process::Step;
pub const STEPS:&[Step]=&[Step::new(
    "angle.inverse",
    "Prove exact inverse Angle domains, canonical transport, certified error and ordinary installed Plan/Play",
    "cargo",
    &[
        "test","--locked","-p","conduit-core","-p","conduit-plot","-p","conduit-std-host",
        "--test","inverse_trigonometry","--test","inverse_circular",
        "--test","mathematical_inverse_trigonometry","--test","mathematical_angle",
        "--test","quantity_atan2","--test","inverse_angle_writer",
        "--test","mathematical_enclosure","--test","mathematical_trigonometry",
        "--test","inverse_angle","--test","mathematical_angle_transport",
        "--test","angle_trigonometry","--test","mathematical_value_transport",
    ],
).with_environment(&[
    ("CARGO_PROFILE_TEST_OPT_LEVEL","1"),("CARGO_PROFILE_TEST_DEBUG","0"),
    ("CARGO_PROFILE_TEST_DEBUG_ASSERTIONS","true"),("CARGO_PROFILE_TEST_OVERFLOW_CHECKS","true"),
])];
