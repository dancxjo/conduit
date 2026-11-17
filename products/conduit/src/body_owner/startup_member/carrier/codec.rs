//! Fixed joined-control framing; initial Host offers are compared without owning a copy.
use super::*;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
#[path = "codec/initial_host.rs"]
mod initial_host;

#[derive(Serialize)]
struct Frame<'a, T> {
    kind: &'static str,
    protocol: u16,
    command: &'a T,
}
#[derive(Serialize)]
struct Hello<'a> {
    kind: &'static str,
    protocol: u16,
    session_secret: &'a [u8; 32],
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    kind: String,
    protocol: u16,
    reply: Reply,
}

impl Carrier {
    pub(super) fn hello(
        &mut self,
        secret: &mut [u8; 32],
        expected: &HostAdvertisement,
    ) -> Result<[u8; 32], String> {
        let result = self.send_value(&Hello {
            kind: "hello",
            protocol: 1,
            session_secret: secret,
        });
        secret.fill(0);
        result?;
        let workspace = Arc::get_mut(&mut self.workspace)
            .ok_or("startup workspace is still borrowed")?
            .get_mut()
            .map_err(|_| "startup workspace unavailable")?;
        let count = self
            .line
            .receive_binary(workspace.read_buffer())
            .map_err(|_| "startup member initial observation failed")?;
        let observed = initial_host::observe(&workspace.read_buffer()[..count], expected)?;
        let expected_length = {
            let mut cursor = Cursor::new(workspace.write_buffer());
            serde_json::to_writer(&mut cursor, expected)
                .map_err(|_| "startup selected offer exceeds joined frame")?;
            cursor.position() as usize
        };
        let expected_digest: [u8; 32] =
            sha2::Sha256::digest(&workspace.write_buffer()[..expected_length]).into();
        if observed
            .is_some_and(|(digest, length)| digest != expected_digest || length != expected_length)
        {
            return Err("startup actual rich Host differs from canonical selected offer".into());
        }
        Ok(expected_digest)
    }
    pub(super) fn send_command<T: Serialize>(&mut self, command: &T) -> Result<(), String> {
        self.send_value(&Frame {
            kind: "startup-member",
            protocol: 1,
            command,
        })
    }
    fn send_value<T: Serialize>(&mut self, value: &T) -> Result<(), String> {
        let workspace = Arc::get_mut(&mut self.workspace)
            .ok_or("startup workspace is still borrowed")?
            .get_mut()
            .map_err(|_| "startup workspace unavailable")?;
        let length = {
            let mut cursor = Cursor::new(workspace.write_buffer());
            serde_json::to_writer(&mut cursor, value)
                .map_err(|_| "startup control message exceeds its joined frame")?;
            cursor.position() as usize
        };
        let result = self
            .line
            .send_binary(&workspace.write_buffer()[..length])
            .map_err(|_| "startup control send outcome unknown".into());
        workspace.write_buffer()[..length].fill(0);
        result
    }
    pub(super) fn send_encoded(&mut self, length: usize) -> Result<(), String> {
        let workspace = Arc::get_mut(&mut self.workspace)
            .ok_or("startup workspace is still borrowed")?
            .get_mut()
            .map_err(|_| "startup workspace unavailable")?;
        let bytes = workspace
            .write_buffer()
            .get_mut(..length)
            .ok_or("startup encoded frame bound")?;
        let result = self
            .line
            .send_binary(bytes)
            .map_err(|_| "startup commit outcome unknown".into());
        bytes.fill(0);
        result
    }
    pub(super) fn receive_reply(&mut self) -> Result<Reply, ReceiveError> {
        let workspace = Arc::get_mut(&mut self.workspace)
            .ok_or(ReceiveError::Unknown)?
            .get_mut()
            .map_err(|_| ReceiveError::Unknown)?;
        let count = match self.line.receive_binary(workspace.read_buffer()) {
            Ok(count) => count,
            Err(conduit_std_host::secure_websocket::SecureWebSocketError::Transport(
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut,
            )) => return Err(ReceiveError::Waiting),
            Err(_) => return Err(ReceiveError::Unknown),
        };
        let bytes = &workspace.read_buffer()[..count];
        crate::durable_host_control::startup_member::preflight::reply(bytes)
            .map_err(|_| ReceiveError::Malformed)?;
        let response: Response =
            serde_json::from_slice(bytes).map_err(|_| ReceiveError::Malformed)?;
        if response.kind != "startup-member" || response.protocol != 1 {
            return Err(ReceiveError::Malformed);
        }
        Ok(response.reply)
    }
}

pub(super) fn encode_commit(
    workspace: &Arc<Mutex<StartupMemberControlWorkspace>>,
    session_id: [u8; 32],
    receipt: &DelegatedBodyPreparationReceipt,
    biography: &BodyBiographyEvidence,
) -> Result<usize, String> {
    let mut workspace = workspace
        .lock()
        .map_err(|_| "startup workspace unavailable")?;
    let mut cursor = Cursor::new(workspace.write_buffer());
    serde_json::to_writer(
        &mut cursor,
        &Frame {
            kind: "startup-member",
            protocol: 1,
            command: &BorrowedCommand::Commit {
                session_id,
                preparation_id: &receipt.preparation_id,
                biography,
            },
        },
    )
    .map_err(|_| "retained startup Commit exceeds its finite frame")?;
    Ok(cursor.position() as usize)
}
#[cfg(test)]
#[path = "codec/wire_tests.rs"]
mod wire_tests;
