//! Finite read-only hardware status. This is not a Body admission endpoint.
use embassy_net::{tcp::TcpSocket, Stack};
use embassy_time::{with_timeout, Duration, Timer};
#[path = "environment_http_request.rs"]
mod request;
pub(super) const SESSIONS: usize = 2;
const RX_BYTES: usize = 512;
const TX_BYTES: usize = 1536;
const HEADER_BYTES: usize = 192;
// Maximum status String capacity among the checked sensor profiles.
const STATUS_BYTES: usize = 1536;
// Input ends before response construction, so count their peak, not both phases.
const BUFFER_BYTES: usize = SESSIONS * (RX_BYTES + TX_BYTES + STATUS_BYTES + HEADER_BYTES);
const _: () = assert!(BUFFER_BYTES as u64 <= crate::config::DIAGNOSTIC_HTTP_BUFFER_LIMIT);
fn bounded_status<const N: usize>(status: heapless::String<N>) -> heapless::String<N> {
    const {
        assert!(N <= STATUS_BYTES);
    }
    status
}
async fn write(socket: &mut TcpSocket<'_>, mut bytes: &[u8]) -> Result<(), ()> {
    while !bytes.is_empty() {
        let length = socket.write(bytes).await.map_err(|_| ())?;
        if length == 0 {
            return Err(());
        }
        bytes = &bytes[length..];
    }
    Ok(())
}
async fn serve(socket: &mut TcpSocket<'_>) -> Result<(), ()> {
    use core::fmt::Write;
    let code = {
        let mut input = [0; 512];
        let mut length = 0;
        loop {
            if let Some(code) = request::status(&input[..length], input.len()) {
                break code;
            }
            let n = socket.read(&mut input[length..]).await.map_err(|_| ())?;
            if n == 0 {
                return Err(());
            }
            length += n;
        }
    };
    let status = bounded_status(crate::environment::status().map_err(|_| ())?);
    let body = if code == 200 {
        status.as_bytes()
    } else {
        b"{}"
    };
    let reason = match code {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        431 => "Request Header Fields Too Large",
        _ => "Bad Request",
    };
    let mut header = heapless::String::<HEADER_BYTES>::new();
    write!(&mut header,"HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",body.len()).map_err(|_|())?;
    write(socket, header.as_bytes()).await?;
    write(socket, body).await?;
    socket.flush().await.map_err(|_| ())
}
// A browser may open an idle speculative connection before its navigation.
// Keep a second finite session available instead of removing the only listener.
#[embassy_executor::task(pool_size = SESSIONS)]
pub async fn run(stack: Stack<'static>) -> ! {
    let mut rx = [0; RX_BYTES];
    let mut tx = [0; TX_BYTES];
    loop {
        stack.wait_config_up().await;
        let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
        socket.set_timeout(Some(Duration::from_secs(3)));
        let accepted = socket.accept(8080).await.is_ok();
        if accepted {
            let _ = with_timeout(Duration::from_secs(3), serve(&mut socket)).await;
            socket.close();
            let _ = with_timeout(Duration::from_millis(500), socket.flush()).await;
        }
        socket.abort();
        drop(socket);
        if !accepted {
            Timer::after_millis(10).await;
        }
    }
}
