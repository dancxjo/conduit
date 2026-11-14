//! Canonical checked native Mask data, not membership or execution authority.
use super::*;
#[path = "data/human.rs"]
mod human;
#[path = "data/render.rs"]
mod render;
#[path = "data/spread.rs"]
mod spread;
#[path = "data/surface.rs"]
mod surface;
pub(super) const BACKING_BYTES: usize = 6560
    + core::mem::size_of::<AuthorityBinding>()
    + 4 * core::mem::size_of::<ConfigurationEntry>()
    + 14 * core::mem::size_of::<ExpectedSign>()
    + 9 * core::mem::size_of::<ExpectedTerminal>()
    + core::mem::size_of::<FragmentCommitment>()
    + 2 * core::mem::size_of::<HostCallRequirement>()
    + 4 * core::mem::size_of::<KindConfigurationField>()
    + 2 * core::mem::size_of::<KindSemanticLaw>()
    + 4 * core::mem::size_of::<PlacementId>()
    + core::mem::size_of::<PlanFragment>()
    + 4 * core::mem::size_of::<PlannedConnection>()
    + 3 * core::mem::size_of::<PlannedForePort>()
    + 4 * core::mem::size_of::<PlannedGear>()
    + 9 * core::mem::size_of::<PortDescriptor>()
    + 2 * core::mem::size_of::<ResourceBinding>()
    + 2 * core::mem::size_of::<ResourcePortContract>()
    + 4 * core::mem::size_of::<StartupDependency>();
pub(super) fn construct() -> PlannedMaskPlot {
    PlannedMaskPlot {
        mask: MaskPlot {
            plot_identity: PlotIdentity {
                source_document_id:
                    "a33fa003c240c875026b8b81f95e703c4a78b252583f2e5383a4da5465653bd3".into(),
                checked_plot_id: "28eb72135c48fed21c4d8fcca7bbfe3f3791e733cf431667ada29ed43994f0f7"
                    .into(),
                expanded_plot_id:
                    "4c4d6f8adafdc395537e70943f0a89d2c6304c5ff408a72d0dccdb76bb5c6d29".into(),
            },
            plot_name: "native-graphical".into(),
            face_input: MaskPlotBoundary {
                front_port_id: "face".into(),
                gear_id: "native-graphical/spread".into(),
                gear_port_id: "source".into(),
            },
            interaction_output: MaskPlotBoundary {
                front_port_id: "interaction".into(),
                gear_id: "native-graphical/human".into(),
                gear_port_id: "interaction".into(),
            },
            show_output: MaskPlotBoundary {
                front_port_id: "show".into(),
                gear_id: "native-graphical/render".into(),
                gear_port_id: "show".into(),
            },
        },
        plan: Plan {
            plan_id: "14fa97e5f2cad09b14cb91b3ae8adc61fc86d3318051ad275b41421f54ef0a66".into(),
            source_document_id: "a33fa003c240c875026b8b81f95e703c4a78b252583f2e5383a4da5465653bd3"
                .into(),
            checked_plot_id: "28eb72135c48fed21c4d8fcca7bbfe3f3791e733cf431667ada29ed43994f0f7"
                .into(),
            expanded_plot_id: "4c4d6f8adafdc395537e70943f0a89d2c6304c5ff408a72d0dccdb76bb5c6d29"
                .into(),
            completion_policy: PlanCompletionPolicy::Live,
            realization_backs: Vec::new(),
            fragments: Vec::from([PlanFragment {
                plan_id: "14fa97e5f2cad09b14cb91b3ae8adc61fc86d3318051ad275b41421f54ef0a66".into(),
                fragment_id: "19f3ae6da8eb344aa8b33fed1c7382a0012010e0e49082d7d3660e57b736e776"
                    .into(),
                source_document_id:
                    "a33fa003c240c875026b8b81f95e703c4a78b252583f2e5383a4da5465653bd3".into(),
                checked_plot_id: "28eb72135c48fed21c4d8fcca7bbfe3f3791e733cf431667ada29ed43994f0f7"
                    .into(),
                expanded_plot_id:
                    "4c4d6f8adafdc395537e70943f0a89d2c6304c5ff408a72d0dccdb76bb5c6d29".into(),
                completion_policy: PlanCompletionPolicy::Live,
                realization_backs: Vec::new(),
                host_id: "canonical/native-template".into(),
                boot_id: "canonical/native-template/boot".into(),
                offer_generation: OfferGeneration(1),
                placements: Vec::from([
                    human::construct(),
                    render::construct(),
                    spread::construct(),
                    surface::construct(),
                ]),
                execution_regions: Vec::new(),
                execution_fusions: Vec::new(),
                connections: Vec::from([
                    PlannedConnection {
                        connection_id:
                            "629149e8d42d031ec3c50db509aff673c153962ed99fec0cc85a5b9486fc671c"
                                .into(),
                        source_placement_id:
                            "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b"
                                .into(),
                        source_port_id: "show".into(),
                        sink_placement_id:
                            "dd81120a27a414bbc5ec5c1c69656d03103a54966c504d8acae66c3084decd23"
                                .into(),
                        sink_port_id: "show".into(),
                        value_kind: "face/show@1".into(),
                        track: ConnectionTrack::Payload,
                        temporal: PortTemporal::Value,
                        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
                        selected_line: None,
                        admitted_lines: Vec::new(),
                        item_capacity: 1,
                        byte_capacity: 49152,
                        resource: None,
                        abnormal_kind: None,
                    },
                    PlannedConnection {
                        connection_id:
                            "935e77f4966cf9fa34d9bcc66e523c834b8cd1e12235481ce40056a59c16037e"
                                .into(),
                        source_placement_id:
                            "bb80d3911353a3d6fad132b7bc21ab4e98a8b7f8414c26278b751fd82e4d4445"
                                .into(),
                        source_port_id: "face".into(),
                        sink_placement_id:
                            "dd81120a27a414bbc5ec5c1c69656d03103a54966c504d8acae66c3084decd23"
                                .into(),
                        sink_port_id: "face".into(),
                        value_kind: "face/face@1".into(),
                        track: ConnectionTrack::Payload,
                        temporal: PortTemporal::Value,
                        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
                        selected_line: None,
                        admitted_lines: Vec::new(),
                        item_capacity: 1,
                        byte_capacity: 49152,
                        resource: None,
                        abnormal_kind: None,
                    },
                    PlannedConnection {
                        connection_id:
                            "9ffd1b5026a0492a92431d8aa4ae1d6316b6c6df9541efae3fe7e09701330084"
                                .into(),
                        source_placement_id:
                            "bb80d3911353a3d6fad132b7bc21ab4e98a8b7f8414c26278b751fd82e4d4445"
                                .into(),
                        source_port_id: "face".into(),
                        sink_placement_id:
                            "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b"
                                .into(),
                        sink_port_id: "face".into(),
                        value_kind: "face/face@1".into(),
                        track: ConnectionTrack::Payload,
                        temporal: PortTemporal::Value,
                        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
                        selected_line: None,
                        admitted_lines: Vec::new(),
                        item_capacity: 1,
                        byte_capacity: 49152,
                        resource: None,
                        abnormal_kind: None,
                    },
                    PlannedConnection {
                        connection_id:
                            "f258c85c39e2762b1647bb43cb22865b26c7465c984fd424d1f75e9c07375ff3"
                                .into(),
                        source_placement_id:
                            "71b907f7982840d85d6e9087a88cd7f18a950619a0aa4eae4b8aad9f8da39a2b"
                                .into(),
                        source_port_id: "show-resource".into(),
                        sink_placement_id:
                            "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b"
                                .into(),
                        sink_port_id: "show-resource".into(),
                        value_kind: "face/show-resource@1".into(),
                        resource: Some(PlannedResourceConnection {
                            contract: ResourcePortContract {
                                port_id: "show-resource".into(),
                                class_id: "face.resource/show@1".into(),
                                ownership: ResourcePortOwnership::Move,
                                lifecycle: ResourcePortLifecycle::Play,
                                mobility: ResourcePortMobility::HostLocal,
                            },
                            owner_placement_id:
                                "71b907f7982840d85d6e9087a88cd7f18a950619a0aa4eae4b8aad9f8da39a2b"
                                    .into(),
                            source_binding: ResourceBinding {
                                pool_id: "canonical/surface".into(),
                                class_id: "face.resource/show@1".into(),
                                units: 1,
                                protected: None,
                                compute: None,
                                content: None,
                            },
                        }),
                        track: ConnectionTrack::Payload,
                        temporal: PortTemporal::Value,
                        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
                        selected_line: None,
                        admitted_lines: Vec::new(),
                        item_capacity: 1,
                        byte_capacity: 49152,
                        abnormal_kind: None,
                    },
                ]),
                fore_ports: Vec::from([
                    PlannedForePort {
                        front_port_id: "face".into(),
                        direction: PortDirection::Input,
                        placement_id:
                            "bb80d3911353a3d6fad132b7bc21ab4e98a8b7f8414c26278b751fd82e4d4445"
                                .into(),
                        gear_port_id: "source".into(),
                        value_kind: "face/face@1".into(),
                        track: ConnectionTrack::Payload,
                        temporal: PortTemporal::Value,
                        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
                        item_capacity: 1,
                        byte_capacity: 49152,
                        value_contract: None,
                        abnormal_kind: None,
                        selected_line: None,
                    },
                    PlannedForePort {
                        front_port_id: "interaction".into(),
                        direction: PortDirection::Output,
                        placement_id:
                            "dd81120a27a414bbc5ec5c1c69656d03103a54966c504d8acae66c3084decd23"
                                .into(),
                        gear_port_id: "interaction".into(),
                        value_kind: "face/interaction@2".into(),
                        track: ConnectionTrack::Payload,
                        temporal: PortTemporal::Flow { closes: true },
                        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
                        item_capacity: 1,
                        byte_capacity: 8192,
                        value_contract: None,
                        abnormal_kind: None,
                        selected_line: None,
                    },
                    PlannedForePort {
                        front_port_id: "show".into(),
                        direction: PortDirection::Output,
                        placement_id:
                            "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b"
                                .into(),
                        gear_port_id: "show".into(),
                        value_kind: "face/show@1".into(),
                        track: ConnectionTrack::Payload,
                        temporal: PortTemporal::Value,
                        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
                        item_capacity: 1,
                        byte_capacity: 49152,
                        value_contract: None,
                        abnormal_kind: None,
                        selected_line: None,
                    },
                ]),
                shared_pools: Vec::new(),
                startup_dependencies: Vec::from([
                    StartupDependency {
                        prerequisite_placement_id:
                            "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b"
                                .into(),
                        dependent_placement_id:
                            "71b907f7982840d85d6e9087a88cd7f18a950619a0aa4eae4b8aad9f8da39a2b"
                                .into(),
                    },
                    StartupDependency {
                        prerequisite_placement_id:
                            "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b"
                                .into(),
                        dependent_placement_id:
                            "bb80d3911353a3d6fad132b7bc21ab4e98a8b7f8414c26278b751fd82e4d4445"
                                .into(),
                    },
                    StartupDependency {
                        prerequisite_placement_id:
                            "dd81120a27a414bbc5ec5c1c69656d03103a54966c504d8acae66c3084decd23"
                                .into(),
                        dependent_placement_id:
                            "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b"
                                .into(),
                    },
                    StartupDependency {
                        prerequisite_placement_id:
                            "dd81120a27a414bbc5ec5c1c69656d03103a54966c504d8acae66c3084decd23"
                                .into(),
                        dependent_placement_id:
                            "bb80d3911353a3d6fad132b7bc21ab4e98a8b7f8414c26278b751fd82e4d4445"
                                .into(),
                    },
                ]),
                startup_order: Vec::from([
                    "dd81120a27a414bbc5ec5c1c69656d03103a54966c504d8acae66c3084decd23".into(),
                    "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b".into(),
                    "71b907f7982840d85d6e9087a88cd7f18a950619a0aa4eae4b8aad9f8da39a2b".into(),
                    "bb80d3911353a3d6fad132b7bc21ab4e98a8b7f8414c26278b751fd82e4d4445".into(),
                ]),
                cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
                terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
                expected_terminals: Vec::from([
                    ExpectedTerminal::PlacementCompleted(
                        "dd81120a27a414bbc5ec5c1c69656d03103a54966c504d8acae66c3084decd23".into(),
                    ),
                    ExpectedTerminal::PlacementCompleted(
                        "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b".into(),
                    ),
                    ExpectedTerminal::PlacementCompleted(
                        "bb80d3911353a3d6fad132b7bc21ab4e98a8b7f8414c26278b751fd82e4d4445".into(),
                    ),
                    ExpectedTerminal::PlacementCompleted(
                        "71b907f7982840d85d6e9087a88cd7f18a950619a0aa4eae4b8aad9f8da39a2b".into(),
                    ),
                    ExpectedTerminal::ConnectionCompleted(
                        "629149e8d42d031ec3c50db509aff673c153962ed99fec0cc85a5b9486fc671c".into(),
                    ),
                    ExpectedTerminal::ConnectionCompleted(
                        "935e77f4966cf9fa34d9bcc66e523c834b8cd1e12235481ce40056a59c16037e".into(),
                    ),
                    ExpectedTerminal::ConnectionCompleted(
                        "9ffd1b5026a0492a92431d8aa4ae1d6316b6c6df9541efae3fe7e09701330084".into(),
                    ),
                    ExpectedTerminal::ConnectionCompleted(
                        "f258c85c39e2762b1647bb43cb22865b26c7465c984fd424d1f75e9c07375ff3".into(),
                    ),
                    ExpectedTerminal::PlanCompleted,
                ]),
                expected_sign: Vec::from([
                    ExpectedSign::PlanFragmentReceived,
                    ExpectedSign::PlacementPrepared(
                        "dd81120a27a414bbc5ec5c1c69656d03103a54966c504d8acae66c3084decd23".into(),
                    ),
                    ExpectedSign::PlacementPrepared(
                        "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b".into(),
                    ),
                    ExpectedSign::PlacementPrepared(
                        "bb80d3911353a3d6fad132b7bc21ab4e98a8b7f8414c26278b751fd82e4d4445".into(),
                    ),
                    ExpectedSign::PlacementPrepared(
                        "71b907f7982840d85d6e9087a88cd7f18a950619a0aa4eae4b8aad9f8da39a2b".into(),
                    ),
                    ExpectedSign::PlacementTerminal(
                        "dd81120a27a414bbc5ec5c1c69656d03103a54966c504d8acae66c3084decd23".into(),
                    ),
                    ExpectedSign::PlacementTerminal(
                        "5ffb809dfaeca106917d7288bb0f329a4820fa7b5d3ae60eaf668cf245a50f7b".into(),
                    ),
                    ExpectedSign::PlacementTerminal(
                        "bb80d3911353a3d6fad132b7bc21ab4e98a8b7f8414c26278b751fd82e4d4445".into(),
                    ),
                    ExpectedSign::PlacementTerminal(
                        "71b907f7982840d85d6e9087a88cd7f18a950619a0aa4eae4b8aad9f8da39a2b".into(),
                    ),
                    ExpectedSign::ConnectionTerminal(
                        "629149e8d42d031ec3c50db509aff673c153962ed99fec0cc85a5b9486fc671c".into(),
                    ),
                    ExpectedSign::ConnectionTerminal(
                        "935e77f4966cf9fa34d9bcc66e523c834b8cd1e12235481ce40056a59c16037e".into(),
                    ),
                    ExpectedSign::ConnectionTerminal(
                        "9ffd1b5026a0492a92431d8aa4ae1d6316b6c6df9541efae3fe7e09701330084".into(),
                    ),
                    ExpectedSign::ConnectionTerminal(
                        "f258c85c39e2762b1647bb43cb22865b26c7465c984fd424d1f75e9c07375ff3".into(),
                    ),
                    ExpectedSign::PlanTerminal,
                ]),
                sign_storage_budget: SignStorageBudget {
                    item_capacity: 14,
                    byte_capacity: 782,
                },
                plan_fragments: Vec::from([FragmentCommitment {
                    host_id: "canonical/native-template".into(),
                    fragment_id: "19f3ae6da8eb344aa8b33fed1c7382a0012010e0e49082d7d3660e57b736e776"
                        .into(),
                }]),
                states: Vec::new(),
            }]),
            activations: Vec::new(),
            activation_preparations: Vec::new(),
        },
    }
}
