use std::env;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use conduit_core::{AuthorityTime, Id, InstancePath, PinnedDescriptor, SemanticHash};
use conduit_embedded_build::{
    EmbeddedHostOperationBinding, EmbeddedNodeBinding, EmbeddedProgramIdentity,
    generate_embedded_plan,
};
use sha2::{Digest, Sha256};

#[path = "src/reference_plan.rs"]
mod reference_plan;

const FIRMWARE_INPUTS: [&str; 14] = [
    "../../Cargo.lock",
    "../../Cargo.toml",
    "../../crates/conduit-core/Cargo.toml",
    "../../crates/conduit-core/src",
    "../../crates/conduit-embedded/Cargo.toml",
    "../../crates/conduit-embedded/src/lib.rs",
    "../../crates/conduit-embedded-build/Cargo.toml",
    "../../crates/conduit-embedded-build/src",
    "Cargo.toml",
    "build.rs",
    "memory.x",
    "src/lib.rs",
    "src/main.rs",
    "src/reference_plan.rs",
];

fn main() {
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    std::fs::copy("memory.x", output.join("memory.x")).expect("copy RP2040 memory layout");
    let conduit_revision = conduit_revision();
    let embedded_plan = generated_embedded_plan(&conduit_revision);
    fs::write(output.join("embedded_plan.rs"), &embedded_plan)
        .expect("write generated embedded plan");
    let mut digest = Sha256::new();
    for path in FIRMWARE_INPUTS {
        let input = Path::new(path);
        if input.is_dir() {
            let mut files = Vec::new();
            collect_files(input, input, &mut files);
            files.sort();
            for relative in files {
                let label = format!("{path}/{}", relative.display());
                hash_file(&mut digest, &label, &input.join(relative));
            }
        } else {
            hash_file(&mut digest, path, input);
        }
    }
    hash_bytes(
        &mut digest,
        "generated-embedded-plan",
        embedded_plan.as_bytes(),
    );
    let target = env::var("TARGET").expect("Cargo supplies TARGET");
    let profile = env::var("PROFILE").expect("Cargo supplies PROFILE");
    let rustc = env::var_os("RUSTC").expect("Cargo supplies RUSTC");
    let rustc_version = Command::new(rustc)
        .arg("-vV")
        .output()
        .expect("query rustc version");
    assert!(rustc_version.status.success(), "rustc -vV must succeed");
    hash_bytes(&mut digest, "cargo-target", target.as_bytes());
    hash_bytes(&mut digest, "cargo-profile", profile.as_bytes());
    hash_bytes(&mut digest, "rustc-version", &rustc_version.stdout);
    let digest = digest.finalize();
    let mut generated = String::from(
        "pub const FIRMWARE_IDENTITY: conduit_core::SemanticHash = \
         conduit_core::SemanticHash::from_bytes([",
    );
    for (index, byte) in digest.iter().enumerate() {
        if index > 0 {
            generated.push(',');
        }
        write!(generated, "{byte}").expect("write generated firmware identity");
    }
    generated.push_str("]);\n");
    fs::write(output.join("firmware_identity.rs"), generated)
        .expect("write generated firmware identity");
    println!("cargo:rustc-env=CONDUIT_FIRMWARE_TARGET={target}");
    println!("cargo:rustc-env=CONDUIT_FIRMWARE_PROFILE={profile}");
    println!("cargo:rustc-link-search={}", output.display());
    if target == "thumbv6m-none-eabi" {
        println!("cargo:rustc-link-arg=-Tlink.x");
    }
}

fn conduit_revision() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir("../..")
        .output()
        .expect("query exact Conduit revision");
    assert!(output.status.success(), "git rev-parse HEAD must succeed");
    let revision = String::from_utf8(output.stdout)
        .expect("Git revision is UTF-8")
        .trim()
        .to_owned();
    assert!(
        revision.len() == 40
            && revision
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "Git must return one full lowercase commit"
    );
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    let symbolic = Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .current_dir("../..")
        .output()
        .expect("query Conduit symbolic ref");
    if symbolic.status.success() {
        let reference = String::from_utf8(symbolic.stdout)
            .expect("Git symbolic ref is UTF-8")
            .trim()
            .to_owned();
        println!("cargo:rerun-if-changed=../../.git/{reference}");
    }
    revision
}

fn generated_embedded_plan(conduit_revision: &str) -> String {
    const NO_PORTS: &[Id<'static>] = &[];
    const INPUT_PORTS: &[Id<'static>] = &[Id("in")];
    const OUTPUT_PORTS: &[Id<'static>] = &[Id("out")];
    reference_plan::with_equivalence_plans(|_, rp2040_plan, _| {
        let sensor_host_operations = [EmbeddedHostOperationBinding {
            ordinal: 0,
            effect_hash: rp2040_plan.nodes[0].required_effects[0],
            resource_binding: rp2040_plan.nodes[0].required_resources[0],
        }];
        let indicator_host_operations = [EmbeddedHostOperationBinding {
            ordinal: 1,
            effect_hash: rp2040_plan.nodes[2].required_effects[0],
            resource_binding: rp2040_plan.nodes[2].required_resources[0],
        }];
        let bindings = [
            EmbeddedNodeBinding {
                instance: InstancePath::new("fixture/sensor").expect("reference instance"),
                driver: pin("fixture/rp2040-sensor-driver", 80),
                input_ports: NO_PORTS,
                output_ports: OUTPUT_PORTS,
                host_operations: &sensor_host_operations,
            },
            EmbeddedNodeBinding {
                instance: InstancePath::new("fixture/threshold").expect("reference instance"),
                driver: pin("fixture/rp2040-threshold-driver", 81),
                input_ports: INPUT_PORTS,
                output_ports: OUTPUT_PORTS,
                host_operations: &[],
            },
            EmbeddedNodeBinding {
                instance: InstancePath::new("fixture/indicator").expect("reference instance"),
                driver: pin("fixture/rp2040-indicator-driver", 82),
                input_ports: INPUT_PORTS,
                output_ports: NO_PORTS,
                host_operations: &indicator_host_operations,
            },
        ];
        generate_embedded_plan(
            &rp2040_plan,
            conduit_core::PlanValidationContext {
                supported_schema_version: rp2040_plan.schema_version,
                now: AuthorityTime {
                    basis: Id("clock/monotonic"),
                    tick: 1,
                },
            },
            reference_plan::embedded_profile(),
            EmbeddedProgramIdentity {
                conduit_revision,
                policy_package_hash: reference_plan::PROGRAM_FIXTURE_PACKAGE_HASH,
                policy_lock_hash: reference_plan::PROGRAM_FIXTURE_LOCK_HASH,
            },
            &bindings,
        )
        .expect("checked reference plan must lower exactly")
        .render_rust_module()
    })
}

const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([byte; 32]),
    }
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read firmware input directory") {
        let path = entry.expect("read firmware input entry").path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else {
            files.push(
                path.strip_prefix(root)
                    .expect("firmware input remains below root")
                    .to_owned(),
            );
        }
    }
}

fn hash_file(digest: &mut Sha256, label: &str, path: &Path) {
    let bytes = fs::read(path).expect("read firmware identity input");
    hash_bytes(digest, label, &bytes);
    println!("cargo:rerun-if-changed={}", path.display());
}

fn hash_bytes(digest: &mut Sha256, label: &str, bytes: &[u8]) {
    digest.update(label.as_bytes());
    digest.update([0]);
    digest.update(
        u64::try_from(bytes.len())
            .expect("firmware input length")
            .to_be_bytes(),
    );
    digest.update(bytes);
}
