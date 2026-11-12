//! Build-host semantic interfaces for selected autonomous device protocols.
//! These declarations create no available controller, timer, reservation or grant.
use conduit_plot::{ProfileCatalog, StartupCatalog};

pub fn install(startup: &mut StartupCatalog, profiles: &mut ProfileCatalog) -> Result<(), String> {
    let mut staged = startup.clone();
    let mut definitions = profiles.clone();
    conduit_i2c_transaction::contract::I2cContract::prepare()
        .map_err(|e| format!("I2C contract: {e:?}"))?
        .install_catalogs(&mut staged, &mut definitions)?;
    conduit_monotonic_wait::clock_at::contract::MonotonicClockContract::prepare()
        .map_err(|e| format!("deadline clock contract: {e:?}"))?
        .install_catalogs(&mut staged, &mut definitions)?;
    *startup = staged;
    *profiles = definitions;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    #[test]
    fn selected_i2c_and_deadline_clock_remain_native_owners() {
        let source = br#"
with machine/i2c/transact/request as I2cTransaction
with machine/i2c/transact/result as I2cResult
with machine/clock/at/request as MonotonicClockRequest
with machine/clock/at/result as MonotonicClockResult
plot bus-client (
    >> request: I2cTransaction...|
    result: I2cResult...| >>
) {
    bus: machine/i2c/transact
    request >> bus.request
    bus.result >> result
}
plot clock-client (
    >> request: MonotonicClockRequest...|
    result: MonotonicClockResult...| >>
) {
    clock: machine/clock/at
    request >> clock.request
    clock.result >> result
}
"#;
        let digest = format!("{:x}", Sha256::digest(source));
        let set = crate::workset::compile(
            source,
            &digest,
            &["bus-client".into(), "clock-client".into()],
            None,
        )
        .unwrap();
        for (entry, kind) in set.entries.iter().zip([
            conduit_i2c_transaction::contract::I2C_KIND,
            conduit_monotonic_wait::clock_at::contract::CLOCK_KIND,
        ]) {
            let material = crate::software::compile(&entry.plot.expanded, 24 * 1024).unwrap();
            assert!(
                material.nodes.is_empty(),
                "native IO must not acquire software offers"
            );
            assert!(
                entry.rust.contains(kind),
                "retain native selection for exact calibration"
            );
        }
    }
}
