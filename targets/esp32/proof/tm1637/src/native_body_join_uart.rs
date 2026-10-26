//! Narrow physical USB custody path. Private replies are written only to the
//! owning UART, never to the normal tracing/logging path or LAN discovery.
use crate::{
    native_body_join::owner::{Gate, Request},
    native_root::NativeRoot,
};
use alloc::vec::Vec;
use embassy_time::{with_timeout, Duration, Instant};
use esp_hal::uart::Uart;
const MAX_COMMAND: usize = 1024;
struct Reply(Vec<u8>);
impl Drop for Reply {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
pub(crate) struct Bootstrap {
    gate: Gate,
    command: Vec<u8>,
    began_ms: u64,
}
impl Bootstrap {
    pub(crate) fn configured() -> Result<Self, &'static str> {
        mod owner_key {
            include!(concat!(env!("OUT_DIR"), "/owner-control-key.rs"));
        }
        Ok(Self {
            gate: Gate::new(owner_key::PUBLIC_KEY)?,
            command: Vec::new(),
            began_ms: 0,
        })
    }
    /// Root calls once per ordinary service tick. Reception has a fixed five
    /// second deadline and at most32bytes per poll; local workers keep running.
    pub(crate) async fn poll(
        &mut self,
        root: &mut NativeRoot,
        uart: &mut Uart<'_, esp_hal::Async>,
    ) -> Result<(), &'static str> {
        let now = Instant::now().as_millis();
        if !self.command.is_empty() && now.saturating_sub(self.began_ms) > 5_000 {
            self.clear();
        }
        let mut input = [0; 32];
        let count = match with_timeout(Duration::from_millis(1), uart.read_async(&mut input)).await
        {
            Err(_) => return Ok(()),
            Ok(Err(_)) => return Err("body-join-uart-read"),
            Ok(Ok(n)) => n,
        };
        for byte in &input[..count] {
            if self.command.is_empty() {
                if *byte != b'J' {
                    continue;
                }
                self.command
                    .try_reserve_exact(MAX_COMMAND)
                    .map_err(|_| "body-join-uart-memory")?;
                self.began_ms = now;
            }
            if self.command.len() == MAX_COMMAND {
                self.clear();
                continue;
            }
            if *byte == b'\n' {
                let result = self.handle(root, uart).await;
                self.clear();
                if result.is_err() {
                    write(uart, b"\n{\"event\":\"body-join-refused\"}\n").await?;
                }
            } else if *byte != b'\r' {
                self.command.push(*byte);
            }
        }
        Ok(())
    }
    fn clear(&mut self) {
        self.command.fill(0);
        self.command = Vec::new();
    }
    async fn handle(
        &mut self,
        root: &mut NativeRoot,
        uart: &mut Uart<'_, esp_hal::Async>,
    ) -> Result<(), &'static str> {
        let now = Instant::now().as_millis();
        let (host, boot, body, key) = root.body_join_scope()?;
        if self.command == b"JOIN?" {
            let mut nonce = [0; 32];
            entropy(&mut nonce)?;
            self.gate.challenge(nonce, now)?;
            #[derive(serde::Serialize)]
            struct Challenge<'a> {
                schema: &'static str,
                host: &'a str,
                boot: &'a str,
                body: &'a str,
                root_key: [u8; 32],
                nonce: [u8; 32],
                valid_for_ms: u64,
                next_sequence: u64,
            }
            let challenge = Challenge {
                schema: "conduit.environment/private-invitation-challenge@1",
                host: host.as_str(),
                boot: boot.as_str(),
                body: body.as_str(),
                root_key: key,
                nonce,
                valid_for_ms: 30_000,
                next_sequence: self
                    .gate
                    .next_sequence()
                    .ok_or("body-join-owner-exhausted")?,
            };
            return write_json(uart, &challenge).await;
        }
        let bytes = self
            .command
            .strip_prefix(b"JOIN ")
            .ok_or("body-join-uart-command")?;
        let request: Request =
            serde_json::from_slice(bytes).map_err(|_| "body-join-uart-request")?;
        let authorized = self.gate.authorize(
            &request,
            host.as_str(),
            boot.as_str(),
            body.as_str(),
            &key,
            now,
        )?;
        let expires = now
            .checked_add(authorized.ttl_ms())
            .ok_or("body-join-uart-clock")?;
        let (mut secret, mut nonce) = ([0; 32], [0; 32]);
        entropy(&mut secret)?;
        entropy(&mut nonce)?;
        let result = root.issue_body_invitation(secret, nonce, now, expires);
        secret.fill(0);
        nonce.fill(0);
        let mut bootstrap = result?;
        let result = write_json(uart, &bootstrap).await;
        bootstrap.invitation.secret.fill(0);
        result
    }
}
fn entropy(bytes: &mut [u8]) -> Result<(), &'static str> {
    esp_hal::rng::Trng::try_new()
        .map_err(|_| "body-join-entropy")?
        .read(bytes);
    Ok(())
}
async fn write_json(
    uart: &mut Uart<'_, esp_hal::Async>,
    value: &impl serde::Serialize,
) -> Result<(), &'static str> {
    let mut reply = Reply(Vec::new());
    reply
        .0
        .try_reserve_exact(2048)
        .map_err(|_| "body-join-uart-memory")?;
    reply.0.resize(2048, 0);
    let length =
        serde_json_core::to_slice(value, &mut reply.0).map_err(|_| "body-join-uart-encoding")?;
    write(uart, b"\n").await?;
    write(uart, &reply.0[..length]).await?;
    write(uart, b"\n").await
}
async fn write(uart: &mut Uart<'_, esp_hal::Async>, mut bytes: &[u8]) -> Result<(), &'static str> {
    with_timeout(Duration::from_secs(2), async {
        while !bytes.is_empty() {
            let n = uart
                .write_async(bytes)
                .await
                .map_err(|_| "body-join-uart-write")?;
            if n == 0 {
                return Err("body-join-uart-closed");
            }
            bytes = &bytes[n..];
        }
        Ok(())
    })
    .await
    .map_err(|_| "body-join-uart-timeout")?
}
