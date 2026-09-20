//! Native responder for the pinned-Chromium protected-Line interoperability proof.

use conduit_protected_line::{
    EndpointBinding, ProtectedHandshake, Role, SessionBinding, SessionLimits,
};
use std::io::{self, BufRead, Write};

const BROWSER_PAYLOAD: &[u8] = b"browser-to-native secret";
const NATIVE_PAYLOAD: &[u8] = b"native-to-browser secret";

fn main() {
    if let Err(error) = run() {
        eprintln!("protected-line-browser-peer refused: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let stdin = io::stdin();
    let mut input = stdin.lock().lines();
    let mut handshake =
        ProtectedHandshake::new(Role::Responder, &binding(), limits(), [8; 32], [5; 32])
            .map_err(debug("initialize native responder"))?;
    let mut inbound = decode_line(input.next(), "first handshake")?;
    handshake
        .read_message(&inbound)
        .map_err(debug("read browser handshake"))?;
    inbound.fill(0);
    let mut outbound = vec![
        0;
        handshake
            .next_message_bytes()
            .map_err(debug("size response"))?
    ];
    handshake
        .write_message(&mut outbound)
        .map_err(debug("write native handshake"))?;
    write_hex(&outbound)?;
    outbound.fill(0);

    let mut session = handshake
        .finish()
        .map_err(debug("finish native handshake"))?;
    let mut protected = decode_line(input.next(), "browser protected frame")?;
    let mut plaintext = [0_u8; 256];
    let opened = session
        .open(&protected, &mut plaintext)
        .map_err(debug("open browser frame"))?;
    protected.fill(0);
    if &plaintext[..opened] != BROWSER_PAYLOAD {
        plaintext.fill(0);
        return Err("browser plaintext differed after native authentication".into());
    }
    plaintext.fill(0);
    let mut response = [0_u8; 290];
    let sealed = session
        .seal(NATIVE_PAYLOAD, &mut response)
        .map_err(debug("seal native response"))?;
    write_hex(&response[..sealed])?;
    response.fill(0);
    session.close();
    Ok(())
}

fn binding() -> SessionBinding {
    SessionBinding {
        initiator: EndpointBinding {
            host_id: "host/browser/one".into(),
            boot_id: "boot/browser/one".into(),
        },
        responder: EndpointBinding {
            host_id: "host/native/two".into(),
            boot_id: "boot/native/two".into(),
        },
        negotiation_id: "negotiation/browser-native".into(),
        line_session_id: "line/browser-native".into(),
        candidate_binding: "route/browser-native".into(),
        transport_binding: "relay/inspecting-proof@1".into(),
    }
}

fn limits() -> SessionLimits {
    SessionLimits {
        maximum_payload_bytes: 256,
        maximum_frames_per_direction: 8,
        maximum_bytes_per_direction: 2_048,
    }
}

fn decode_line(line: Option<Result<String, io::Error>>, context: &str) -> Result<Vec<u8>, String> {
    let line = line
        .ok_or_else(|| format!("{context} was absent"))?
        .map_err(|error| format!("read {context}: {error}"))?;
    if line.is_empty() || line.len() > 131_200 || line.len() % 2 != 0 {
        return Err(format!("{context} violated its encoded bound"));
    }
    line.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let text = core::str::from_utf8(pair).map_err(|_| format!("{context} was not hex"))?;
            u8::from_str_radix(text, 16).map_err(|_| format!("{context} was not hex"))
        })
        .collect()
}

fn write_hex(bytes: &[u8]) -> Result<(), String> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    for byte in bytes {
        write!(output, "{byte:02x}").map_err(|error| format!("write proof frame: {error}"))?;
    }
    writeln!(output).map_err(|error| format!("finish proof frame: {error}"))?;
    output
        .flush()
        .map_err(|error| format!("flush proof frame: {error}"))
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}
