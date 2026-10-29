//! Finite read-only hardware status. This is not a Body admission endpoint.
use embassy_net::{tcp::TcpSocket, Stack};
use embassy_time::{with_timeout, Duration, Timer};
#[path = "environment_http_request.rs"]
mod request;
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
    let mut input = [0; 512];
    let mut length = 0;
    let code = loop {
        if let Some(code) = request::status(&input[..length], input.len()) {
            break code;
        }
        let n = socket.read(&mut input[length..]).await.map_err(|_| ())?;
        if n == 0 {
            return Err(());
        }
        length += n;
    };
    let status = crate::environment::status().map_err(|_| ())?;
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
    let mut header = heapless::String::<192>::new();
    write!(&mut header,"HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",body.len()).map_err(|_|())?;
    write(socket, header.as_bytes()).await?;
    write(socket, body).await?;
    socket.flush().await.map_err(|_| ())
}
#[embassy_executor::task]
pub async fn run(stack: Stack<'static>) -> ! {
    let mut rx = [0; 512];
    let mut tx = [0; 1536];
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
