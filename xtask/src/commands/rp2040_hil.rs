use std::{fs, io::Read, path::Path};

use serde_json::json;

use crate::commands::embedded_gate::current_firmware_identity;

pub struct Rp2040HilOptions {
    pub port: Option<String>,
    pub expected_plan_hash: Option<String>,
    pub expected_firmware_identity: Option<String>,
    pub maximum_decisions: u32,
    pub timeout_seconds: f64,
    pub probe: bool,
    pub require_hardware: bool,
}

fn discover_port() -> Option<String> {
    let dev_dir = Path::new("/dev/serial/by-id");
    if !dev_dir.is_dir() {
        return None;
    }
    if let Ok(entries) = fs::read_dir(dev_dir) {
        let matches: Vec<_> = entries
            .flatten()
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.contains("Conduit_RP2040_HIL")
                    || name.to_lowercase().contains("conduit-rp2040-hil")
            })
            .collect();
        if matches.len() == 1 {
            return Some(matches[0].path().to_string_lossy().to_string());
        }
    }
    None
}

fn hash_bytes(val: &str) -> Result<Vec<u8>, String> {
    let normalized = val.strip_prefix("sha256:").unwrap_or(val);
    let decoded = hex::decode(normalized).map_err(|e| e.to_string())?;
    if decoded.len() != 32 {
        return Err("expected plan hash must be 32 bytes".to_string());
    }
    Ok(decoded)
}

#[cfg(unix)]
fn configure_raw_termios(fd: std::os::unix::io::RawFd) -> Result<(), String> {
    unsafe {
        let mut term: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(fd, &mut term) != 0 {
            return Err("tcgetattr failed".to_string());
        }
        term.c_iflag = 0;
        term.c_oflag = 0;
        term.c_cflag = libc::CS8 | libc::CREAD | libc::CLOCAL;
        term.c_lflag = 0;
        libc::cfsetispeed(&mut term, libc::B115200);
        libc::cfsetospeed(&mut term, libc::B115200);
        term.c_cc[libc::VMIN] = 0;
        term.c_cc[libc::VTIME] = 0;
        if libc::tcsetattr(fd, libc::TCSANOW, &term) != 0 {
            return Err("tcsetattr failed".to_string());
        }
        libc::tcflush(fd, libc::TCIOFLUSH);
    }
    Ok(())
}

pub fn run(
    workspace_root: &Path,
    opts: Rp2040HilOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    let port = opts.port.or_else(discover_port);

    if opts.probe {
        let firmware_id =
            current_firmware_identity(workspace_root, "thumbv6m-none-eabi", "release")?;
        let report = json!({
            "schema": "conduit.rp2040-hil-probe/v1",
            "detected": port.is_some(),
            "port": port,
            "expected_firmware_identity": format!("sha256:{}", hex::encode(firmware_id)),
        });
        println!("{}", serde_json::to_string(&report)?);
        if port.is_none() && opts.require_hardware {
            std::process::exit(2);
        }
        return Ok(());
    }

    let port_str = match port {
        Some(p) => p,
        None => {
            let message = "no unique Conduit RP2040 HIL USB-CDC device detected";
            if opts.require_hardware {
                eprintln!("{message}");
                std::process::exit(2);
            } else {
                let report = json!({ "executed": false, "reason": message });
                println!("{}", serde_json::to_string(&report)?);
                return Ok(());
            }
        }
    };

    let plan_str = opts
        .expected_plan_hash
        .ok_or("--expected-plan-hash is required for an HIL run")?;
    let expected_plan = hash_bytes(&plan_str)?;

    let expected_firmware = match opts.expected_firmware_identity {
        Some(ref f) => hash_bytes(f)?,
        None => current_firmware_identity(workspace_root, "thumbv6m-none-eabi", "release")?,
    };

    let mut nonce = [0u8; 16];
    getrandom_bytes(&mut nonce)?;

    // REQUEST struct: magic(4s), version(H), nonce(16s), plan(32s), max_decisions(I)
    let mut request = Vec::with_capacity(58);
    request.extend_from_slice(b"CNH1");
    request.extend_from_slice(&1u16.to_be_bytes()); // PROTOCOL_VERSION = 1
    request.extend_from_slice(&nonce);
    request.extend_from_slice(&expected_plan);
    request.extend_from_slice(&opts.maximum_decisions.to_be_bytes());

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let path_c = std::ffi::CString::new(Path::new(&port_str).as_os_str().as_bytes())?;
        let fd = unsafe {
            libc::open(
                path_c.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(format!("failed to open serial port {port_str}").into());
        }

        struct FdGuard(libc::c_int);
        impl Drop for FdGuard {
            fn drop(&mut self) {
                unsafe { libc::close(self.0) };
            }
        }
        let _guard = FdGuard(fd);

        configure_raw_termios(fd)?;

        let written = unsafe { libc::write(fd, request.as_ptr() as *const _, request.len()) };
        if written < 0 || written as usize != request.len() {
            return Err("failed to write complete CNH1 request to serial port".into());
        }

        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs_f64(opts.timeout_seconds);

        fn read_exact_fd(
            fd: libc::c_int,
            len: usize,
            deadline: std::time::Instant,
        ) -> Result<Vec<u8>, String> {
            let mut buf = vec![0u8; len];
            let mut read_bytes = 0;
            while read_bytes < len {
                let now = std::time::Instant::now();
                if now >= deadline {
                    return Err(format!("timed out after {read_bytes} of {len} bytes"));
                }
                let timeout = deadline - now;
                let mut pfd = libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                };
                let ret = unsafe { libc::poll(&mut pfd, 1, timeout.as_millis() as libc::c_int) };
                if ret > 0 && (pfd.revents & libc::POLLIN) != 0 {
                    let n = unsafe {
                        libc::read(
                            fd,
                            buf[read_bytes..].as_mut_ptr() as *mut _,
                            len - read_bytes,
                        )
                    };
                    if n > 0 {
                        read_bytes += n as usize;
                    }
                }
            }
            Ok(buf)
        }

        // Header format (179 bytes):
        // magic(4s), version(u16), response_nonce(16s), plan(32s), firmware(32s), capability(32s), boot_id(16s), run_sequence(u64), status(u8), decisions(u32), count(u16)
        let header_buf = read_exact_fd(fd, 179, deadline)?;
        let magic = &header_buf[0..4];
        let version = u16::from_be_bytes(header_buf[4..6].try_into().unwrap());
        let response_nonce = &header_buf[6..22];
        let plan = &header_buf[22..54];
        let firmware_identity = &header_buf[54..86];
        let capability_report_hash = &header_buf[86..118];
        let boot_id = &header_buf[118..134];
        let run_sequence = u64::from_be_bytes(header_buf[134..142].try_into().unwrap());
        let status = header_buf[142];
        let decisions = u32::from_be_bytes(header_buf[143..147].try_into().unwrap());
        let count = u16::from_be_bytes(header_buf[147..149].try_into().unwrap());

        if magic != b"CNR1"
            || version != 1
            || response_nonce != nonce
            || plan != expected_plan
            || firmware_identity != expected_firmware
            || capability_report_hash == [0u8; 32]
            || status != 1
        {
            return Err("HIL response header failed identity or status validation".into());
        }

        let event_kinds = [
            (1, "allocation-prepared"),
            (2, "node-prepared"),
            (3, "run-started"),
            (4, "decision"),
            (5, "value-accepted"),
            (6, "value-consumed"),
            (7, "pressure-entered"),
            (8, "pressure-cleared"),
            (9, "node-completed"),
            (10, "cancellation-requested"),
            (11, "run-succeeded"),
            (12, "run-cancelled"),
            (13, "run-failed"),
        ];
        let event_kinds_map: std::collections::BTreeMap<u8, &str> =
            event_kinds.into_iter().collect();

        let mut events = Vec::new();
        for expected_sequence in 0..(count as u64) {
            // Event struct format (108 bytes):
            // magic(4s), version(u16), nonce(16s), plan(32s), boot(16s), run(u64), sequence(u32), tick(u32), subject_kind(u8), subject_index(u16), kind(u8), value_length(u16), value(16s)
            let ev_buf = read_exact_fd(fd, 108, deadline)?;
            let ev_magic = &ev_buf[0..4];
            let ev_version = u16::from_be_bytes(ev_buf[4..6].try_into().unwrap());
            let ev_nonce = &ev_buf[6..22];
            let ev_plan = &ev_buf[22..54];
            let ev_boot = &ev_buf[54..70];
            let ev_run = u64::from_be_bytes(ev_buf[70..78].try_into().unwrap());
            let sequence = u32::from_be_bytes(ev_buf[78..82].try_into().unwrap());
            let tick = u32::from_be_bytes(ev_buf[82..86].try_into().unwrap());
            let subject_kind = ev_buf[86];
            let subject_index = u16::from_be_bytes(ev_buf[87..89].try_into().unwrap());
            let kind = ev_buf[89];
            let value_length = u16::from_be_bytes(ev_buf[90..92].try_into().unwrap()) as usize;
            let val_bytes = &ev_buf[92..108];

            if ev_magic != b"CNE1"
                || ev_version != 1
                || ev_nonce != nonce
                || ev_plan != expected_plan
                || ev_boot != boot_id
                || ev_run != run_sequence
                || (sequence as u64) != expected_sequence
                || !event_kinds_map.contains_key(&kind)
                || value_length > val_bytes.len()
            {
                return Err("HIL event attribution or sequence validation failed".into());
            }

            events.push(json!({
                "sequence": sequence,
                "tick": tick,
                "subject_kind": subject_kind,
                "subject_index": subject_index,
                "kind": event_kinds_map[&kind],
                "value": hex::encode(&val_bytes[..value_length]),
            }));
        }

        let received_kinds: std::collections::BTreeSet<&str> = events
            .iter()
            .filter_map(|e| e.get("kind").and_then(serde_json::Value::as_str))
            .collect();

        let required = [
            "allocation-prepared",
            "node-prepared",
            "run-started",
            "value-accepted",
            "value-consumed",
            "pressure-entered",
            "pressure-cleared",
            "node-completed",
            "run-succeeded",
        ];

        for req in required {
            if !received_kinds.contains(req) {
                return Err(format!("HIL evidence omitted missing required event {req}").into());
            }
        }

        let accepted: Vec<Vec<u8>> = events
            .iter()
            .filter(|e| e.get("kind").and_then(serde_json::Value::as_str) == Some("value-accepted"))
            .filter_map(|e| e.get("value").and_then(serde_json::Value::as_str))
            .filter_map(|v| hex::decode(v).ok())
            .collect();

        let expected_accepted = vec![42u32.to_be_bytes().to_vec(), vec![0x01]];
        if accepted != expected_accepted {
            return Err("HIL semantic values differ from the representative oracle".into());
        }

        let report = json!({
            "schema": "conduit.rp2040-hil-report/v1",
            "executed": true,
            "port": port_str,
            "plan_hash": format!("sha256:{}", hex::encode(plan)),
            "firmware_identity": format!("sha256:{}", hex::encode(firmware_identity)),
            "capability_report_hash": format!("sha256:{}", hex::encode(capability_report_hash)),
            "boot_id": hex::encode(boot_id),
            "run_sequence": run_sequence,
            "decisions": decisions,
            "evidence_records": count,
            "normalized": {
                "values": accepted.iter().map(hex::encode).collect::<Vec<_>>(),
                "pressure_entered": received_kinds.contains("pressure-entered"),
                "pressure_cleared": received_kinds.contains("pressure-cleared"),
                "terminal": "run-succeeded",
            },
        });

        println!("{}", serde_json::to_string_pretty(&report)?);
    }

    Ok(())
}

fn getrandom_bytes(buf: &mut [u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::fs::File;
        let mut f = File::open("/dev/urandom").map_err(|e| e.to_string())?;
        f.read_exact(buf).map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        Err("platform random not available".to_string())
    }
}
