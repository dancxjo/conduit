//! Reviewed source reachability/authority observations. Not a call-graph proof.
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, process::Command};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Responsibility {
    name: String,
    path: String,
    sha256: String,
    anchors: Vec<String>,
    execution: String,
    authority: String,
}
const ROOT: &str =
    "root-source-reachable; linked presence checked separately; execution not inferred";
const GUEST: &str =
    "separately linked guest selected by x86_64 source; confinement/execution not inferred";
// Each row is a reviewed path/authority assertion backed by exact source bytes.
// Changing this table is an architecture-review event, not automatic isolation.
type Row = (
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static str,
    &'static str,
);
const ROWS: &[Row] = &[
    ("entry", "src/main.rs", &["graphical_startup::run(record)"], ROOT, "boot normalization and machine entry"),
    ("startup", "src/graphical_startup.rs", &["arch::initialize_xhci", "boot::framebuffer_display", "product_front_door::"], ROOT, "PCI/MMIO DMA-capable xHCI, firmware framebuffer, entropy and offers"),
    ("controller", "src/product_front_door.rs", &["fn run_controller", "runtime::RootRuntime"], ROOT, "product/lifecycle policy and checked effect dispatch remain privileged"),
    ("physical-platform", "src/product_front_door/platform.rs", &["controller: &mut XhciReady", "hid_session: Option<&mut HidKeyboardSession>"], ROOT, "USB/input/rescue/network device handles and write-only display sink"),
    ("runtime", "src/product_front_door/runtime.rs", &["struct RootRuntime", "run_tour"], ROOT, "ordinary/Tour execution paths remain Root; not all runtime moved"),
    ("mask-facade", "src/native_face_mask.rs", &["pub struct NativeFaceMask", "suspend"], ROOT, "Face/Show custody, checked input and scanout acknowledgement"),
    ("compositor-facade", "src/native_compositor/protected.rs", &["SharedDisplayOwner", "validate_active_session"], ROOT, "grant/session validation and transient raster transport; no retained guest pixel ownership"),
    ("scanout", "src/native_compositor/protected/scanout.rs", &["impl NativeCompositor"], ROOT, "validated returned spans copied to privileged display before acknowledgement"),
    ("mapping", "src/arch/x86_64/service_memory.rs", &["ServiceImage"], ROOT, "page-table construction and service memory admission; mappings need execution proof"),
    ("physical-stores", "src/arch/x86_64/service_memory/storage.rs", &["ServiceProfile", "AtomicBool"], ROOT, "nine independently claimed physical stores and page-table backing"),
    ("privilege-transition", "src/arch/x86_64/domain_transition.rs", &["conduitos_ordinary_enter", "domain_budget"], ROOT, "CPL3 entry, trap return and timer preemption; actual enforcement requires execution evidence"),
    ("effect-bases", "src/product_bases.rs", &["NativeProductBases"], ROOT, "admitted Root hardware/effect providers remain privilege-bearing"),
    ("profiles", "src/service_memory.rs", &["pub enum ServiceProfile", "RX/RO", "RW+NX"], ROOT, "closed ranges declare image/exchange/transfer/stack/private arenas; not installed mappings"),
    ("compositor", "compositor-service/main.rs", &["no_std"], GUEST, "retained composition/focus/damage; no framebuffer/device pointer grant"),
    ("draft-home", "creche-service/main.rs", &["creche_entry", "memory::EXCHANGE"], GUEST, "Draft and Home share immutable image but independent instances/arenas"),
    ("runtime-guest", "runtime-service/main.rs", &["no_std"], GUEST, "bounded canonical native-workset scheduler; no second semantic scheduler"),
    ("application", "application-service/main.rs", &["no_std"], GUEST, "native retained application state; remaining Root product paths separate"),
    ("network", "network-service/main.rs", &["no_std"], GUEST, "bounded protocol state; Root retains NIC/USB device authority"),
    ("tls", "tls-service/main.rs", &["no_std"], GUEST, "bounded TLS protocol state; Root retains transport authority"),
    ("mask-guest", "mask-service/main.rs", &["no_std"], GUEST, "checked Mask guest presentation state; Root retains input/scanout validation"),
    ("ordinary", "domain/main.rs", &["no_std"], GUEST, "ordinary protected text/runtime commands; not all Root runtime paths"),
];
const PROFILES: &[&str] = &[
    "Compositor",
    "Draft",
    "Home",
    "Runtime",
    "Application",
    "Network",
    "OrdinaryRuntime",
    "Mask",
    "Tls",
];

fn read(root: &Path, commit: Option<&str>, path: &str) -> Result<Vec<u8>, String> {
    if let Some(commit) = commit {
        let result = Command::new("git")
            .current_dir(root)
            .args(["show", &format!("{commit}:{path}")])
            .output()
            .map_err(|e| e.to_string())?;
        if !result.status.success() {
            return Err(format!("missing audited source {path} at {commit}"));
        }
        Ok(result.stdout)
    } else {
        fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))
    }
}
pub(super) fn observe(root: &Path, commit: Option<&str>) -> Result<Vec<Responsibility>, String> {
    let mut records = Vec::new();
    for (name, path, anchors, execution, authority) in ROWS {
        let path = format!("targets/conduitos/{path}");
        let bytes = read(root, commit, &path)?;
        let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        for anchor in *anchors {
            if !text.contains(anchor) {
                return Err(format!("reviewed {name} anchor missing: {anchor}"));
            }
        }
        if *name == "profiles" {
            let body = text
                .split_once("pub enum ServiceProfile {")
                .ok_or("missing profiles")?
                .1
                .split_once('}')
                .ok_or("unclosed profiles")?
                .0;
            let actual: Vec<_> = body
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.trim_end_matches(','))
                .collect();
            if actual != PROFILES {
                return Err("service profile inventory changed; review classifications".into());
            }
        }
        records.push(Responsibility {
            name: (*name).into(),
            path,
            sha256: super::digest(&bytes),
            anchors: anchors.iter().map(|s| (*s).into()).collect(),
            execution: (*execution).into(),
            authority: (*authority).into(),
        });
    }
    Ok(records)
}
