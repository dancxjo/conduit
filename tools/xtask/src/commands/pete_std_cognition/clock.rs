//! Prepared Linux CLOCK_MONOTONIC domain, not a cross-clock correlation.
//! boot_id is kernel-provided random identity, not authenticated machine identity.
//! The finite operator lifecycle does not mutate time namespaces. Procfs guards
//! reject observed changes; separate reads cannot exclude hostile A->B->A changes
//! or forged procfs. Holding a namespace FD prevents inode reuse, not mutation.
use conduit_time::{NativeTemporalInstant, NativeTemporalScale};
use std::{fs::File, io::Read, os::unix::fs::MetadataExt};

const BOOT_ID: &str = "/proc/sys/kernel/random/boot_id";
const ACTIVE: &str = "/proc/thread-self/ns/time";
const CHILDREN: &str = "/proc/thread-self/ns/time_for_children";
// Linux registers timens_offsets only in the thread-group proc directory.
const OFFSETS: &str = "/proc/self/timens_offsets";
const LEADER_ACTIVE: &str = "/proc/self/ns/time";
const OFFSET_SOURCE: &str = "/proc/self/ns/time_for_children";
const BOOT_BOUND: usize = 37;
const OFFSETS_BOUND: usize = 256;

pub(super) trait ClockSource {
    fn basis(&self) -> &str;
    fn millis(&mut self) -> Result<u64, String>;
    fn now(&mut self) -> Result<NativeTemporalInstant, String> {
        let ticks = self.millis()?;
        NativeTemporalInstant::new(
            self.basis().into(),
            1,
            NativeTemporalScale::Milliseconds,
            ticks,
            0,
        )
        .map_err(|error| format!("Pete clock instant: {error:?}"))
    }
}

pub(super) struct PreparedClock {
    basis: String,
    namespace: NamespaceId,
    // Keep the selected namespace alive so its inode cannot be recycled while
    // this source is guarded. The inode is deliberately absent from basis.
    _namespace_guard: File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NamespaceId {
    device: u64,
    inode: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Namespaces {
    active: NamespaceId,
    children: NamespaceId,
    leader_active: NamespaceId,
    offset_source: NamespaceId,
}

impl PreparedClock {
    pub(super) fn prepare() -> Result<Self, String> {
        let guard = File::open(ACTIVE).map_err(|error| format!("Pete clock namespace: {error}"))?;
        let namespace = namespace_id(
            guard
                .metadata()
                .map_err(|error| format!("Pete clock namespace metadata: {error}"))?,
        );
        let boot = read_file(BOOT_ID, BOOT_BOUND)?;
        let basis = prepare_domain(&boot, namespace, namespaces, || {
            read_file(OFFSETS, OFFSETS_BOUND)
        })?;
        Ok(Self {
            basis,
            namespace,
            _namespace_guard: guard,
        })
    }
}
impl ClockSource for PreparedClock {
    fn basis(&self) -> &str {
        &self.basis
    }
    fn millis(&mut self) -> Result<u64, String> {
        // CLOCK_MONOTONIC follows the calling thread's active namespace. Check
        // that domain around each sample, including after moving to a producer
        // thread. No namespace switch or timestamp rewrite is performed here.
        sample_in_domain(self.namespace, active_namespace, || {
            conduit_std_host::std_create_uart::monotonic_millis()
                .map_err(|error| format!("Pete clock sample: {error:?}"))
        })
    }
}

fn namespace_id(metadata: std::fs::Metadata) -> NamespaceId {
    NamespaceId {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}
fn observe_namespace(path: &str) -> Result<NamespaceId, String> {
    std::fs::metadata(path)
        .map(namespace_id)
        .map_err(|error| format!("Pete clock namespace {path}: {error}"))
}
fn active_namespace() -> Result<NamespaceId, String> {
    observe_namespace(ACTIVE)
}
fn namespaces() -> Result<Namespaces, String> {
    Ok(Namespaces {
        active: active_namespace()?,
        children: observe_namespace(CHILDREN)?,
        leader_active: observe_namespace(LEADER_ACTIVE)?,
        offset_source: observe_namespace(OFFSET_SOURCE)?,
    })
}

fn read_file(path: &str, maximum: usize) -> Result<String, String> {
    let file = File::open(path).map_err(|error| format!("Pete clock source {path}: {error}"))?;
    read_bounded(file, maximum).map_err(|error| format!("Pete clock source {path}: {error}"))
}
fn read_bounded(reader: impl Read, maximum: usize) -> Result<String, String> {
    let mut bytes = Vec::with_capacity(maximum + 1);
    reader
        .take((maximum + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read failed: {error}"))?;
    if bytes.len() > maximum {
        return Err("byte bound exceeded".into());
    }
    String::from_utf8(bytes).map_err(|_| "invalid UTF-8".into())
}

fn prepare_domain(
    boot: &str,
    selected: NamespaceId,
    mut observe: impl FnMut() -> Result<Namespaces, String>,
    offsets: impl FnOnce() -> Result<String, String>,
) -> Result<String, String> {
    let boot = boot_uuid(boot)?;
    let before = observe()?;
    // /proc/self/timens_offsets describes the process leader's children, not
    // the calling thread. Bind that source and both active/children pairs to
    // the pinned selected namespace before reading, and recheck all afterward.
    if [
        before.active,
        before.children,
        before.leader_active,
        before.offset_source,
    ]
    .iter()
    .any(|namespace| *namespace != selected)
    {
        return Err("Pete clock caller/offset-source namespace mismatch".into());
    }
    let offset = monotonic_offset(&offsets()?)?;
    if observe()? != before {
        return Err("Pete clock namespace changed during preparation".into());
    }
    Ok(format!(
        "clock/linux-monotonic@1/{boot}/{}/{}",
        offset.0, offset.1
    ))
}

fn sample_in_domain(
    selected: NamespaceId,
    mut observe: impl FnMut() -> Result<NamespaceId, String>,
    sample: impl FnOnce() -> Result<u64, String>,
) -> Result<u64, String> {
    if observe()? != selected {
        return Err("Pete clock active namespace changed".into());
    }
    let ticks = sample()?;
    if observe()? != selected {
        return Err("Pete clock active namespace changed".into());
    }
    Ok(ticks)
}

fn boot_uuid(input: &str) -> Result<&str, String> {
    let uuid = input.strip_suffix('\n').unwrap_or(input);
    if uuid.len() != 36
        || !uuid.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
        || !uuid
            .bytes()
            .any(|byte| byte.is_ascii_hexdigit() && byte != b'0')
    {
        return Err("Pete clock malformed kernel boot UUID".into());
    }
    Ok(uuid)
}

fn monotonic_offset(input: &str) -> Result<(i64, u32), String> {
    let mut monotonic = None;
    let mut boottime = None;
    for line in input.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 3 {
            return Err("Pete clock malformed namespace offsets".into());
        }
        let seconds = fields[1]
            .parse::<i64>()
            .map_err(|_| "Pete clock invalid offset seconds")?;
        let nanos = fields[2]
            .parse::<u32>()
            .map_err(|_| "Pete clock invalid offset nanoseconds")?;
        if nanos >= 1_000_000_000 {
            return Err("Pete clock invalid offset nanoseconds".into());
        }
        let slot = match fields[0] {
            "monotonic" => &mut monotonic,
            "boottime" => &mut boottime,
            _ => return Err("Pete clock unknown namespace offset".into()),
        };
        if slot.replace((seconds, nanos)).is_some() {
            return Err("Pete clock duplicate namespace offset".into());
        }
    }
    match (monotonic, boottime) {
        (Some(offset), Some(_)) => Ok(offset),
        _ => Err("Pete clock missing namespace offset".into()),
    }
}

#[cfg(test)]
#[path = "clock_domain_tests.rs"]
mod domain_tests;
#[cfg(test)]
#[path = "clock_process_tests.rs"]
mod process_tests;
