use conduit_host_fabrication::{derive_esp32_feature_closure, BaseSelection};

#[derive(Debug, Clone)]
pub struct FeatureProjection {
    pub minimal_bases: Vec<BaseSelection>,
    pub full_bases: Vec<BaseSelection>,
    pub minimal_features: Vec<String>,
    pub full_features: Vec<String>,
}

pub fn checked_feature_projection() -> Result<FeatureProjection, Box<dyn std::error::Error>> {
    let kernel = BaseSelection {
        id: "base/kernel".into(),
        kind: "kernel/signal".into(),
        driver: "esp32/kernel-signal@1".into(),
    };
    let bluetooth = BaseSelection {
        id: "base/bluetooth-line".into(),
        kind: "line/bluetooth-le-gatt".into(),
        driver: "esp32/bluetooth-le-gatt@1".into(),
    };
    let minimal_bases = vec![kernel.clone()];
    let full_bases = vec![kernel, bluetooth];
    let minimal_features = derive(&minimal_bases)?;
    let full_features = derive(&full_bases)?;
    Ok(FeatureProjection {
        minimal_bases,
        full_bases,
        minimal_features,
        full_features,
    })
}

fn derive(bases: &[BaseSelection]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    derive_esp32_feature_closure(bases).map_err(|diagnostic| {
        format!("ESP32 Base-to-feature projection refused: {diagnostic:?}").into()
    })
}

pub fn base_labels(values: &[BaseSelection]) -> Vec<String> {
    values
        .iter()
        .map(|selection| match selection.kind.as_str() {
            "kernel/signal" => Ok("kernel-signal".to_owned()),
            "line/bluetooth-le-gatt" => Ok("bluetooth-le-gatt".to_owned()),
            _ => Err(format!(
                "unsupported checked ESP32 Base `{}`",
                selection.kind
            )),
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("checked Base constructors use the finite known inventory")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_bases_derive_exact_architecture_features() {
        let projection = checked_feature_projection().unwrap();
        assert_eq!(projection.minimal_features, ["kernel-signal"]);
        assert_eq!(projection.full_features, ["bluetooth", "kernel-signal"]);
        assert_eq!(base_labels(&projection.minimal_bases), ["kernel-signal"]);
        assert_eq!(
            base_labels(&projection.full_bases),
            ["kernel-signal", "bluetooth-le-gatt"]
        );
    }
}
