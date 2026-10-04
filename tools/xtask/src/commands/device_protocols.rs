//! Standing deterministic validation of reusable device protocol composition.
use crate::{
    cli::GlobalOpts,
    process::{run_suite, Step},
    workspace::workspace_root,
};

pub fn run(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    run_suite(
        &[
            Step::new(
                "device-protocols.borrowed-values",
                "Check exact borrowed projections and prepared typed composition",
                "cargo",
                &[
                    "test",
                    "--locked",
                    "-p",
                    "conduit-core",
                    "--test",
                    "borrowed_projection",
                    "--test",
                    "typed_pair",
                ],
            ),
            Step::new(
                "device-protocols.finite-joins",
                "Check exact typed pairing, pressure and finite closure",
                "cargo",
                &[
                    "test",
                    "--locked",
                    "-p",
                    "conduit-composite",
                    "-p",
                    "conduit-semantic-catalog",
                    "-p",
                    "conduitos",
                    "flow_zip",
                ],
            ),
            Step::new(
                "device-protocols.binary",
                "Check and execute reviewed portable binary plots",
                "cargo",
                &[
                    "test",
                    "--locked",
                    "-p",
                    "conduit-plot",
                    "--test",
                    "device_binary",
                    "--test",
                    "device_frames",
                    "--test",
                    "typed_pair_expression",
                    "--test",
                    "typed_pair_refinements",
                ],
            ),
            Step::new(
                "device-protocols.compensation",
                "Check exact record construction and authored BME280 arithmetic",
                "cargo",
                &[
                    "test",
                    "--locked",
                    "-p",
                    "conduit-plot",
                    "--test",
                    "named_record_expression",
                    "--test",
                    "native_expression_construction",
                    "--test",
                    "bme280_compensation",
                    "--test",
                    "bme280_lifecycle",
                    "--test",
                    "bme280_observation",
                ],
            ),
            Step::new(
                "device-protocols.native-expressions",
                "Check exact native expression admission and bounded kernel execution",
                "cargo",
                &[
                    "test",
                    "--locked",
                    "-p",
                    "conduitos",
                    "--lib",
                    "expression_host_call",
                ],
            ),
            Step::new(
                "device-protocols.i2c",
                "Check exact I2C schemas, finite geometry and typed outcomes",
                "cargo",
                &["test", "--locked", "-p", "conduitos", "--lib", "i2c_base"],
            ),
        ],
        &root,
        opts,
    )?;
    Ok(())
}
