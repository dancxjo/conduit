//! Short-lived environment-backed Wi-Fi secret values.

use super::PicoResult;

pub(super) struct SecretEnvValue(Vec<u8>);

impl SecretEnvValue {
    pub(super) fn read(variable: &str) -> PicoResult<Self> {
        let value = std::env::var(variable).map_err(|_| {
            format!("required secret environment variable `{variable}` is absent or not UTF-8")
        })?;
        Ok(Self(value.into_bytes()))
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for SecretEnvValue {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
