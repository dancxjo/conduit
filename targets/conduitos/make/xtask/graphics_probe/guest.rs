//! One bounded diagnostic guest. Drop stops it and destroys only private entropy/socket state.
use super::{fail, io, ConduitosArch, ConduitosError, ProbeArgs, Provider};
use std::{
    fs,
    io::Write,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub(super) struct Guest {
    pub child: Child,
    pub started: Instant,
    pub command: Vec<String>,
    pub version: String,
    pub socket: PathBuf,
    pub serial: PathBuf,
    pub debugger: Option<String>,
    _socket_directory: SocketDirectory,
    _entropy: Option<super::super::loongarch64_entropy_input::Input>,
}
impl Drop for Guest {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
struct SocketDirectory(PathBuf);
impl Drop for SocketDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("qmp.sock"));
        let _ = fs::remove_dir(&self.0);
    }
}

pub(super) fn executable(args: &ProbeArgs) -> Result<PathBuf, ConduitosError> {
    let name = args.emulator.clone().unwrap_or_else(|| {
        PathBuf::from(format!(
            "qemu-system-{}",
            if args.arch == ConduitosArch::Ia32 {
                "i386"
            } else {
                args.arch.as_str()
            }
        ))
    });
    if name.components().count() > 1 || name.is_absolute() {
        return name.canonicalize().map_err(io);
    }
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join(&name))
        .find(|p| p.is_file())
        .ok_or_else(|| {
            fail(
                "graphics-probe-emulator-unavailable",
                name.display().to_string(),
            )
        })?
        .canonicalize()
        .map_err(io)
}
fn argument(path: &Path) -> Result<String, ConduitosError> {
    Ok(path
        .canonicalize()
        .map_err(io)?
        .to_str()
        .ok_or_else(|| fail("graphics-probe-path-encoding", "non-UTF8 path"))?
        .replace(',', ",,"))
}
impl Guest {
    pub(super) fn start(
        args: &ProbeArgs,
        executable: &Path,
        image: &Path,
        output: &Path,
    ) -> Result<Self, ConduitosError> {
        let version = Command::new(executable)
            .arg("--version")
            .output()
            .map_err(io)?;
        if !version.status.success() {
            return Err(fail(
                "graphics-probe-emulator-version",
                "QEMU version query failed",
            ));
        }
        let version = String::from_utf8_lossy(&version.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned();
        let major = version
            .strip_prefix("QEMU emulator version ")
            .and_then(|s| s.split('.').next())
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or_else(|| {
                fail(
                    "graphics-probe-emulator-version",
                    "unrecognized QEMU version",
                )
            })?;
        if args.arch == ConduitosArch::Loongarch64 && major < 10 {
            return Err(fail(
                "unsupported-loongarch64-emulator",
                "QEMU 10 or newer is required for correct large-page translation",
            ));
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| fail("graphics-probe-clock", e.to_string()))?
            .as_nanos();
        let socket_directory =
            std::env::temp_dir().join(format!("conduit-fb-{}-{nonce}", std::process::id()));
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&socket_directory)
            .map_err(io)?;
        let directory = SocketDirectory(socket_directory);
        let socket = directory.0.join("qmp.sock");
        let serial = output.join("serial.log");
        fs::write(&serial, []).map_err(io)?;
        let mut command = vec![
            "-machine".into(),
            if matches!(args.arch, ConduitosArch::Ia32 | ConduitosArch::X86_64) {
                "q35"
            } else {
                "virt"
            }
            .into(),
            "-cpu".into(),
            match args.arch {
                ConduitosArch::Aarch64 => "neoverse-n2",
                ConduitosArch::Riscv64 => "rv64,zkr=true,sv57=off,sv48=off",
                ConduitosArch::Loongarch64 => "la464",
                _ => "max",
            }
            .into(),
            "-m".into(),
            match args.arch {
                ConduitosArch::Aarch64 => "512M",
                ConduitosArch::Riscv64 => "256M",
                ConduitosArch::Loongarch64 => "2G",
                ConduitosArch::X86_64 => conduitos::make::PC_PRODUCT_QEMU_MEMORY,
                _ => "512M",
            }
            .into(),
            "-smp".into(),
            "1".into(),
            "-display".into(),
            "none".into(),
            "-monitor".into(),
            "none".into(),
            "-net".into(),
            "none".into(),
            "-no-reboot".into(),
            "-qmp".into(),
            format!("unix:{},server=on,wait=off", socket.display()),
        ];
        if args.arch == ConduitosArch::Ia32 {
            command.extend([
                "-serial".into(),
                "none".into(),
                "-debugcon".into(),
                format!("file:{}", serial.display()),
                "-global".into(),
                "isa-debugcon.iobase=0xe9".into(),
            ]);
        } else {
            command.extend(["-serial".into(), format!("file:{}", serial.display())]);
        }
        if matches!(args.arch, ConduitosArch::Ia32 | ConduitosArch::X86_64) {
            command.extend([
                "-vga".into(),
                if matches!(args.provider, Provider::Vga) {
                    "std"
                } else {
                    "none"
                }
                .into(),
            ]);
        }
        match args.provider {
            Provider::Ramfb => command.extend(["-device".into(), "ramfb".into()]),
            Provider::Vga if !matches!(args.arch, ConduitosArch::Ia32 | ConduitosArch::X86_64) => {
                command.extend(["-device".into(), "VGA".into()])
            }
            _ => (),
        }
        if args.arch == ConduitosArch::Ia32 {
            let vars = output.join("run-vars.fd");
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
            let mut file = options.open(&vars).map_err(io)?;
            let mut source = fs::File::open(args.vars_template.as_ref().unwrap()).map_err(io)?;
            std::io::copy(&mut source, &mut file).map_err(io)?;
            file.flush().map_err(io)?;
            command.extend([
                "-drive".into(),
                format!(
                    "if=pflash,format=raw,readonly=on,file={}",
                    argument(&args.firmware)?
                ),
                "-drive".into(),
                format!("if=pflash,format=raw,file={}", argument(&vars)?),
            ]);
        } else {
            command.extend([
                "-bios".into(),
                args.firmware
                    .canonicalize()
                    .map_err(io)?
                    .to_string_lossy()
                    .into_owned(),
            ]);
        }
        if args.arch == ConduitosArch::Riscv64 {
            command.extend([
                "-accel".into(),
                "tcg,thread=single".into(),
                "-kernel".into(),
                args.boot_kernel
                    .as_ref()
                    .unwrap()
                    .canonicalize()
                    .map_err(io)?
                    .to_string_lossy()
                    .into_owned(),
                "-drive".into(),
                format!("if=virtio,format=raw,readonly=on,file={}", argument(image)?),
            ]);
        } else {
            command.extend([
                "-cdrom".into(),
                image.to_string_lossy().into_owned(),
                "-boot".into(),
                "d".into(),
            ]);
        }
        if args.arch == ConduitosArch::X86_64 {
            command.extend([
                "-device".into(),
                "qemu-xhci,id=conduitos-xhci,p2=2,p3=0".into(),
                "-device".into(),
                "usb-kbd,bus=conduitos-xhci.0,port=1".into(),
                "-device".into(),
                "usb-mouse,bus=conduitos-xhci.0,port=2".into(),
            ]);
        }
        let entropy = if args.arch == ConduitosArch::Loongarch64 {
            let input = super::super::loongarch64_entropy_input::Input::acquire(output)?;
            command.extend(["-fw_cfg".into(), input.argument()?]);
            Some(input)
        } else {
            None
        };
        let debugger = if args.arch == ConduitosArch::Ia32 {
            let listener = TcpListener::bind("127.0.0.1:0").map_err(io)?;
            let address = listener.local_addr().map_err(io)?.to_string();
            drop(listener);
            command.extend(["-S".into(), "-gdb".into(), format!("tcp:{address}")]);
            Some(address)
        } else {
            None
        };
        let started = Instant::now();
        let child = Command::new(executable)
            .args(&command)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                fs::File::create(output.join("qemu-stderr.log")).map_err(io)?,
            ))
            .spawn()
            .map_err(io)?;
        Ok(Self {
            child,
            started,
            command,
            version,
            socket,
            serial,
            debugger,
            _socket_directory: directory,
            _entropy: entropy,
        })
    }
}
