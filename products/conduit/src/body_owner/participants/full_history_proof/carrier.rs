//! One real bounded RFC6455 loopback carrier; no synthetic Line evidence.
use super::super::{face_exchange, transport, BrowserAdmittedSnapshot, In, Out};
use conduit_std_host::browser_admission::MAX_BROWSER_ADMISSION_FRAME_BYTES;
use conduit_std_host::websocket::NativeWebSocketLine;
use std::{sync::mpsc, thread, time::Duration};

enum Command {
    Send(Vec<u8>),
    Receive,
    Stop,
}
pub(super) struct Carrier {
    pub(super) socket: transport::Socket,
    commands: mpsc::SyncSender<Command>,
    replies: mpsc::Receiver<Result<Vec<u8>, String>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Carrier {
    pub(super) fn open() -> Self {
        let listener = transport::Listener::bind().unwrap();
        let url = listener.url().unwrap();
        let address = url
            .strip_prefix("ws://")
            .unwrap()
            .split('/')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let (commands, incoming) = mpsc::sync_channel(1);
        let (outgoing, replies) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let mut line = NativeWebSocketLine::connect(
                address,
                &url,
                MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
            )
            .unwrap();
            while let Ok(command) = incoming.recv() {
                let result = match command {
                    Command::Send(bytes) => line
                        .send_binary(&bytes)
                        .map(|_| Vec::new())
                        .map_err(|e| format!("{e:?}")),
                    Command::Receive => line
                        .receive_binary_with(|bytes| bytes.to_vec())
                        .map_err(|e| format!("{e:?}")),
                    Command::Stop => break,
                };
                let failed = result.is_err();
                if outgoing.send(result).is_err() || failed {
                    break;
                }
            }
            let _ = line.close();
        });
        let socket = listener
            .accept(Duration::from_secs(5), "line/full64-abi-proof")
            .unwrap()
            .unwrap();
        Self {
            socket,
            commands,
            replies,
            worker: Some(worker),
        }
    }
    pub(super) fn to_owner(&mut self, frame: &In) -> (In, u32) {
        let bytes = serde_json::to_vec(frame).unwrap();
        assert!(bytes.len() <= MAX_BROWSER_ADMISSION_FRAME_BYTES);
        self.commands.send(Command::Send(bytes)).unwrap();
        let received = self.socket.receive(Duration::from_secs(5)).unwrap();
        assert!(self.reply().is_empty());
        received
    }
    pub(super) fn to_browser(&mut self, frame: &Out) -> Vec<u8> {
        self.commands.send(Command::Receive).unwrap();
        if matches!(frame, Out::FaceSnapshotResponse { .. }) {
            face_exchange::send_response(&mut self.socket, frame).unwrap();
        } else {
            self.socket.send(frame).unwrap();
        }
        self.reply()
    }
    pub(super) fn prepare_profile(&mut self, snapshot: &BrowserAdmittedSnapshot) {
        self.socket.prepare_owner_face_profile(snapshot).unwrap();
    }
    fn reply(&self) -> Vec<u8> {
        self.replies
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap()
    }
}
impl Drop for Carrier {
    fn drop(&mut self) {
        let _ = self.commands.try_send(Command::Stop);
        self.socket.close();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
