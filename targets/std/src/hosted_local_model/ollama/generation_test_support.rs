//! Fenced loopback HTTP responses; actual curl processes, no model or device.
#![allow(dead_code)] // Shared by focused transport, worker, and installed-Play tests.
use std::{
    collections::VecDeque,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub enum Response {
    Json(&'static str),
    Bytes(Vec<u8>),
    Hold(&'static str),
    Lost,
}

pub struct Server {
    pub endpoint: String,
    pub requests: Receiver<String>,
    pub closed: Receiver<usize>,
    late: Sender<()>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

struct Held {
    socket: TcpStream,
    remainder: Vec<u8>,
    sequence: usize,
}

impl Server {
    pub fn start(responses: impl IntoIterator<Item = Response>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (observed, requests) = channel();
        let (retired, closed) = channel();
        let (late, release) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let mut responses: VecDeque<_> = responses.into_iter().collect();
        let thread = thread::spawn(move || {
            let mut held: Vec<Held> = Vec::new();
            let mut sequence = 0;
            while !stopping.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        socket
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        socket
                            .set_write_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let request = read_request(&mut socket);
                        match responses.pop_front().expect("unexpected model request") {
                            Response::Lost => {}
                            Response::Json(body) => {
                                write_header(&mut socket, body.len());
                                socket.write_all(body.as_bytes()).unwrap();
                            }
                            Response::Bytes(body) => {
                                socket
                                    .set_write_timeout(Some(Duration::from_secs(30)))
                                    .unwrap();
                                write_header(&mut socket, body.len());
                                socket.write_all(&body).unwrap();
                            }
                            Response::Hold(body) => {
                                write_header(&mut socket, body.len());
                                let split = body.len().min(12);
                                socket.write_all(&body.as_bytes()[..split]).unwrap();
                                socket.set_nonblocking(true).unwrap();
                                held.push(Held {
                                    socket,
                                    remainder: body.as_bytes()[split..].to_vec(),
                                    sequence,
                                });
                            }
                        }
                        observed.send(request).unwrap();
                        sequence += 1;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(error) => panic!("fixture accept: {error}"),
                }
                if release.try_recv().is_ok() {
                    for mut pending in held.drain(..) {
                        let _ = pending.socket.write_all(&pending.remainder);
                        let _ = retired.send(pending.sequence);
                    }
                }
                let mut index = 0;
                while index < held.len() {
                    let mut byte = [0];
                    match held[index].socket.read(&mut byte) {
                        Ok(0) => {
                            let pending = held.swap_remove(index);
                            let _ = retired.send(pending.sequence);
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::ConnectionReset
                                    | std::io::ErrorKind::BrokenPipe
                            ) =>
                        {
                            let pending = held.swap_remove(index);
                            let _ = retired.send(pending.sequence);
                        }
                        _ => index += 1,
                    }
                }
                thread::sleep(Duration::from_millis(1));
            }
        });
        Self {
            endpoint,
            requests,
            closed,
            late,
            stop,
            thread: Some(thread),
        }
    }

    pub fn request(&self) -> String {
        self.requests
            .recv_timeout(Duration::from_secs(5))
            .expect("missing fixture request")
    }

    pub fn send_late(&self) {
        self.late.send(()).unwrap();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

fn write_header(socket: &mut TcpStream, bytes: usize) {
    write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {bytes}\r\nConnection: close\r\n\r\n").unwrap();
}

fn read_request(socket: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 4096];
        let size = socket.read(&mut chunk).unwrap();
        assert_ne!(size, 0);
        bytes.extend_from_slice(&chunk[..size]);
        assert!(bytes.len() <= 1024 * 1024);
        let Some(end) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
            continue;
        };
        let header = std::str::from_utf8(&bytes[..end]).unwrap();
        let length = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        if bytes.len() >= end + 4 + length {
            return String::from_utf8(bytes).unwrap();
        }
    }
}

pub const READY: &str = r#"{"response":"ready","done":true,"prompt_eval_count":1,"eval_count":1}"#;
pub const LATE: &str = r#"{"response":"LATE_A","done":true,"prompt_eval_count":1,"eval_count":1}"#;
pub const FRESH: &str =
    r#"{"response":"FRESH_B","done":true,"prompt_eval_count":1,"eval_count":1}"#;
