#[path = "../suites/todo.rs"]
mod todo;

use crate::{
    cli::GlobalOpts,
    process::{run_suite, Step, StepError},
    suites::check::{
        BROWSER_CHECK_STEPS, INPUT_SEMANTICS_STEPS, KERNEL_TAKEOVER_STEPS,
        OBSERVATORY_READINESS_STEPS, PLANNING_S2_STEPS, PLOT_S3_STEPS,
        SEMANTIC_CATALOG_READINESS_STEPS, SIM_READINESS_STEPS, WORKSPACE_STEPS,
    },
    suites::network_capability::NETWORK_CAPABILITY_STEPS,
    suites::pico_compositions::PICO_COMPOSITION_STEPS,
    suites::workspace_shards::WorkspaceShard,
    workspace::workspace_root,
};

mod cli;
mod workspace;
use workspace::run_workspace_shard;
#[path = "check/owner_presentation.rs"]
mod owner_presentation;
pub use cli::{CheckArgs, CheckScope, CheckSuite};

pub fn run(args: CheckArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root().map_err(|error| StepError::prereq("workspace-root", error))?;

    match args.suite.unwrap_or(CheckSuite::Workspace) {
        CheckSuite::Workspace => {
            run_suite(WORKSPACE_STEPS, &root, opts)?;
            run_suite(NETWORK_CAPABILITY_STEPS, &root, opts)?;
            run_suite(PICO_COMPOSITION_STEPS, &root, opts)
        }
        CheckSuite::WorkspaceLint => run_workspace_shard(WorkspaceShard::Lint, &root, opts),
        CheckSuite::WorkspaceTestFoundation => {
            run_workspace_shard(WorkspaceShard::TestFoundation, &root, opts)
        }
        CheckSuite::WorkspaceTestHosts => {
            run_workspace_shard(WorkspaceShard::TestHosts, &root, opts)
        }
        CheckSuite::WorkspaceTestHostsStd => {
            run_workspace_shard(WorkspaceShard::TestHostsStd, &root, opts)
        }
        CheckSuite::WorkspaceTestHostsBrowser => {
            run_workspace_shard(WorkspaceShard::TestHostsBrowser, &root, opts)
        }
        CheckSuite::WorkspaceTestHostsConduitos => {
            run_workspace_shard(WorkspaceShard::TestHostsConduitos, &root, opts)
        }
        CheckSuite::WorkspaceTestHostsWorkbench => {
            run_workspace_shard(WorkspaceShard::TestHostsWorkbench, &root, opts)
        }
        CheckSuite::WorkspaceTestProducts => {
            run_workspace_shard(WorkspaceShard::TestProducts, &root, opts)
        }
        CheckSuite::WorkspacePortable => run_workspace_shard(WorkspaceShard::Portable, &root, opts),
        CheckSuite::WorkspacePico => run_workspace_shard(WorkspaceShard::Pico, &root, opts),
        CheckSuite::Browser => run_suite(BROWSER_CHECK_STEPS, &root, opts),
        CheckSuite::Sim => run_suite(SIM_READINESS_STEPS, &root, opts),
        CheckSuite::KernelTakeover => run_suite(KERNEL_TAKEOVER_STEPS, &root, opts),
        CheckSuite::PlanningS2 => run_suite(PLANNING_S2_STEPS, &root, opts),
        CheckSuite::PlotS3 => run_suite(PLOT_S3_STEPS, &root, opts),
        CheckSuite::Observatory => run_suite(OBSERVATORY_READINESS_STEPS, &root, opts),
        CheckSuite::SemanticCatalog => run_suite(SEMANTIC_CATALOG_READINESS_STEPS, &root, opts),
        CheckSuite::QuantityMapping => run_suite(QUANTITY_MAPPING_STEPS, &root, opts),
        CheckSuite::TodoDurability => run_suite(todo::TODO_DURABILITY_STEPS, &root, opts),
        CheckSuite::TodoState => run_suite(todo::TODO_STATE_STEPS, &root, opts),
        CheckSuite::OwnerPresentation => run_suite(owner_presentation::STEPS, &root, opts),
        CheckSuite::InputSemantics => run_suite(INPUT_SEMANTICS_STEPS, &root, opts),
        CheckSuite::HostedBaseIsolation => run_suite(HOSTED_BASE_ISOLATION_STEPS, &root, opts),
        CheckSuite::IsolatedHttpBase => run_suite(ISOLATED_HTTP_BASE_STEPS, &root, opts),
        CheckSuite::InteropMembrane => run_suite(INTEROP_MEMBRANE_STEPS, &root, opts),
        CheckSuite::FederationSecurity => run_suite(FEDERATION_SECURITY_STEPS, &root, opts),
        CheckSuite::ConfinedGear => run_suite(CONFINED_GEAR_STEPS, &root, opts),
        CheckSuite::ConsequentialEffect => run_suite(CONSEQUENTIAL_EFFECT_STEPS, &root, opts),
        CheckSuite::Ros2Base => run_suite(ROS2_BASE_STEPS, &root, opts),
        CheckSuite::SecurityAcceptance => run_suite(SECURITY_ACCEPTANCE_STEPS, &root, opts),
        CheckSuite::PhysicalEffectHil => run_suite(PHYSICAL_EFFECT_HIL_STEPS, &root, opts),
        CheckSuite::All => {
            run_suite(WORKSPACE_STEPS, &root, opts)?;
            run_suite(NETWORK_CAPABILITY_STEPS, &root, opts)?;
            run_suite(PICO_COMPOSITION_STEPS, &root, opts)?;
            run_suite(BROWSER_CHECK_STEPS, &root, opts)
        }
    }
}

const HOSTED_BASE_ISOLATION_STEPS: &[Step] = &[
    Step::new(
        "hosted-base-isolation.linux",
        "Prove capability-scoped file access and mechanical sibling/process/network denial",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--features",
            "isolated-base-proof",
            "--test",
            "isolated_base_security",
            "--locked",
        ],
    ),
    Step::new(
        "hosted-base-isolation.file-copy",
        "Prove production file/copy semantics and hostile provider confinement",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--features",
            "isolated-base-proof",
            "--test",
            "isolated_file_copy",
            "--locked",
        ],
    ),
    Step::new(
        "hosted-base-isolation.http",
        "Prove exact-endpoint HTTP descriptor authority and mechanical socket denial",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--features",
            "isolated-http-base",
            "--test",
            "isolated_http_base",
            "--locked",
        ],
    ),
];

const ISOLATED_HTTP_BASE_STEPS: &[Step] = &[Step::new(
    "isolated-http-base.exact-endpoint",
    "Prove checked HTTP meaning, exact endpoint authority, and OS-confined provider denial",
    "cargo",
    &[
        "test",
        "-p",
        "conduit-std-host",
        "--features",
        "isolated-http-base",
        "--test",
        "isolated_http_base",
        "--locked",
    ],
)];

const INTEROP_MEMBRANE_STEPS: &[Step] = &[Step::new(
    "interop-membrane.core",
    "Prove directional mapping, sibling exclusion, and origin reflection fencing",
    "cargo",
    &["test", "-p", "conduit-core", "interop::tests", "--locked"],
)];

const FEDERATION_SECURITY_STEPS: &[Step] = &[Step::new(
    "federation-security.loopback",
    "Prove mutual peer attribution, receiver authority, replay fencing, and A-B-C containment",
    "cargo",
    &[
        "test",
        "-p",
        "conduit-body",
        "federation::tests",
        "--locked",
    ],
)];

const CONFINED_GEAR_STEPS: &[Step] = &[Step::new(
    "confined-gear.wasmi",
    "Prove no-WASI imports, finite execution, and exact Base capability mediation",
    "cargo",
    &[
        "test",
        "-p",
        "conduit-std-host",
        "--features",
        "confined-gear",
        "--test",
        "confined_gear_security",
        "--locked",
    ],
)];

const CONSEQUENTIAL_EFFECT_STEPS: &[Step] = &[Step::new(
    "consequential-effect.contract",
    "Prove attended possession, last-mile bounds, no retry, and safe loss disposition",
    "cargo",
    &[
        "test",
        "-p",
        "conduit-core",
        "--test",
        "consequential_effect",
        "--locked",
    ],
)];

const ROS2_BASE_STEPS: &[Step] = &[
    Step::new(
        "ros2-base.contract",
        "Prove exact direction, type, QoS, discovery, lifecycle, origin, and capability checks",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--test",
            "ros2_base_security",
            "--locked",
        ],
    ),
    Step::new(
        "ros2-base.native-jazzy",
        "Run selected ROS input/output topics and a sibling sentinel on native rclpy",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--test",
            "ros2_native_topic_proof",
            "--locked",
        ],
    ),
];

const SECURITY_ACCEPTANCE_STEPS: &[Step] = &[
    Step::new(
        "security-acceptance.matrix",
        "Validate finite complete secret-free blast-radius receipts",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--test",
            "security_acceptance_matrix",
            "--locked",
        ],
    ),
    Step::new(
        "security-acceptance.authority",
        "Attack forged, broadened, stale, exhausted, and revoked Base capabilities",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-core",
            "--test",
            "base_capability",
            "--locked",
        ],
    ),
    Step::new(
        "security-acceptance.gear",
        "Attack the confined Gear engine and its sole capability import",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--features",
            "confined-gear",
            "--test",
            "confined_gear_security",
            "--locked",
        ],
    ),
    Step::new(
        "security-acceptance.base",
        "Attack the Linux process-isolated file Base and independently observe its sibling",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--features",
            "isolated-base-proof",
            "--test",
            "isolated_base_security",
            "--locked",
        ],
    ),
    Step::new(
        "security-acceptance.http-base",
        "Attack exact endpoint, redirect, stale scope, raw socket, and listener boundaries",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--features",
            "isolated-http-base",
            "--test",
            "isolated_http_base",
            "--locked",
        ],
    ),
    Step::new(
        "security-acceptance.federation",
        "Attack authenticated A-B-C federation authority and replay fences",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-body",
            "federation::tests",
            "--locked",
        ],
    ),
    Step::new(
        "security-acceptance.ros2",
        "Attack the ROS topic Base and run its native sibling sentinel",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--test",
            "ros2_native_topic_proof",
            "--locked",
        ],
    ),
    Step::new(
        "security-acceptance.consequential",
        "Attack attended last-mile physical-effect authority and loss behavior",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-core",
            "--test",
            "consequential_effect",
            "--locked",
        ],
    ),
    Step::new(
        "security-acceptance.conduitos",
        "Attack x86_64 kernel memory, sibling domains, MMIO, I/O, and handles",
        "cargo",
        &["xtask", "make", "conduitos", "isolation-proof"],
    ),
];

const PHYSICAL_EFFECT_HIL_STEPS: &[Step] = &[Step::new(
    "physical-effect-hil.wifi",
    "Require fresh attendance and independently observe one bounded physical transmission",
    "cargo",
    &[
        "run",
        "-p",
        "conduit-std-host",
        "--features",
        "physical-effect-proof",
        "--bin",
        "conduit-physical-effect-proof",
        "--locked",
    ],
)];

const QUANTITY_MAPPING_STEPS: &[Step] = &[
    Step::new(
        "quantity-mapping.browser-build",
        "Build the actual browser quantity runtime",
        "cargo",
        &[
            "build",
            "-p",
            "conduit-browser-runtime",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--features",
            "tour-surface",
            "--locked",
        ],
    ),
    Step::new(
        "quantity-mapping.contract",
        "Check exact mapping refusals and bounded structured Quantity encoding",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-semantic-catalog",
            "--all-features",
            "--locked",
            "quantity_",
        ],
    ),
    Step::new(
        "quantity-mapping.kernel",
        "Execute authored quantity Plots with output, correlated Signs and connected refusals",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-std-host",
            "--lib",
            "--locked",
            "quantity_",
        ],
    ),
    Step::new(
        "quantity-mapping.browser-runtime",
        "Execute authored quantity Plots through browser kernel and typed output effects",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-browser-runtime",
            "--lib",
            "--locked",
            "quantity",
        ],
    ),
    Step::new(
        "quantity-mapping.chromium",
        "Prove real pointer causality and deterministic alternate input in pinned Chromium",
        "node",
        &[
            "proof/browser/node_modules/@playwright/test/cli.js",
            "test",
            "--config",
            "proof/browser/playwright.config.mjs",
            "--project=chromium",
            "quantity-controller.spec.mjs",
        ],
    ),
];
