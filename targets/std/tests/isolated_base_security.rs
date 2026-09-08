#![cfg(all(target_os = "linux", feature = "isolated-base-proof"))]

use conduit_core::{
    ActivePlayId, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BaseCapabilityAuthority,
    BaseCapabilityScope, BaseInstanceId, BaseOperationClaim, BootId, CapabilityEnvelopeId,
    CapabilityId, CapabilityIssueRequest, HostId, HostOperationContractId, ImplementationId,
    KindId, PlanId, ResourceGenerationId, ResourcePoolId,
};
use conduit_std_host::isolated_base::{
    read_frame, spawn_provider, write_frame, ProviderFrame, SupervisorFrame, PROTOCOL_VERSION,
};
use std::fs;
use std::io::{BufReader, Write};
use std::path::PathBuf;

fn issue() -> CapabilityIssueRequest {
    let operation = HostOperationContractId::from("conduit.host/file-read@1");
    let scope = BaseCapabilityScope {
        host_id: HostId::from("host/thin"),
        boot_id: BootId::from("boot/current"),
        base_instance_id: BaseInstanceId::from("base/files/provider"),
        base_provider_generation: 1,
        plan_id: PlanId::from("plan/file-read"),
        active_play_id: ActivePlayId::from("play/file-read"),
        authority_grant_id: AuthorityGrantId::from("grant/file-read"),
        authority_contract_id: AuthorityContractId::from("authority/file-read@1"),
        capability_id: CapabilityId::from("file/read"),
        implementation_id: ImplementationId::from("linux/isolated-file-base@1"),
        operation_contract_id: operation.clone(),
        subject_kind: KindId::from("file/allowed-input"),
        resource_pool_id: ResourcePoolId::from("proof/allowed"),
        resource_generation_id: ResourceGenerationId("allowed/generation/1".into()),
        envelope_id: CapabilityEnvelopeId::from("file/read-only/allowed/max-64"),
        maximum_parameter_bytes: 64,
        maximum_result_bytes: 64,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 2,
    };
    CapabilityIssueRequest {
        authority: BaseCapabilityAuthority {
            grant: AuthorityGrant {
                grant_id: scope.authority_grant_id.clone(),
                contract_id: scope.authority_contract_id.clone(),
                host_operation_contract_id: operation.clone(),
                subject_kind: scope.subject_kind.clone(),
                host_id: scope.host_id.clone(),
                boot_id: scope.boot_id.clone(),
                capability_id: scope.capability_id.clone(),
            },
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: operation,
            envelope_id: scope.envelope_id.clone(),
            maximum_parameter_bytes: 64,
            maximum_result_bytes: 64,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 2,
        },
        scope,
    }
}

fn claim(issue: &CapabilityIssueRequest) -> BaseOperationClaim {
    let scope = &issue.scope;
    BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: 64,
        work_units: 1,
    }
}

fn proof_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "conduit-isolated-base-proof-{}",
        std::process::id()
    ))
}

#[test]
fn linux_provider_enforces_capability_and_os_blast_radius() {
    let root = proof_root();
    let allowed = root.join("allowed");
    let protected = root.join("protected-sibling");
    fs::create_dir_all(&allowed).unwrap();
    fs::create_dir_all(&protected).unwrap();
    fs::write(allowed.join("input.txt"), b"authorized-value").unwrap();
    fs::write(protected.join("sentinel.txt"), b"protected-sentinel").unwrap();

    let executable = PathBuf::from(env!("CARGO_BIN_EXE_conduit-isolated-file-base"));
    let mut child = spawn_provider(&executable).unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let issue = issue();
    write_frame(
        &mut input,
        &SupervisorFrame::Bootstrap {
            protocol_version: PROTOCOL_VERSION,
            issue: Box::new(issue.clone()),
            allowed_directory: allowed.display().to_string(),
            allowed_file: allowed.join("input.txt").display().to_string(),
            protected_sibling: protected.join("sentinel.txt").display().to_string(),
        },
    )
    .unwrap();
    assert_eq!(
        read_frame::<_, ProviderFrame>(&mut output).unwrap(),
        ProviderFrame::Ready {
            protocol_version: PROTOCOL_VERSION,
            enforcement: "process-isolated/landlock+seccomp".into(),
            environment_entries: 0,
            descriptor_ceiling: 8,
        }
    );

    write_frame(
        &mut input,
        &SupervisorFrame::Read {
            claim: Box::new(claim(&issue)),
        },
    )
    .unwrap();
    assert_eq!(
        read_frame::<_, ProviderFrame>(&mut output).unwrap(),
        ProviderFrame::ReadCompleted {
            bytes: b"authorized-value".to_vec()
        }
    );

    let mut forged = claim(&issue);
    forged.subject_kind = KindId::from("file/protected-sibling");
    write_frame(
        &mut input,
        &SupervisorFrame::Read {
            claim: Box::new(forged),
        },
    )
    .unwrap();
    assert_eq!(
        read_frame::<_, ProviderFrame>(&mut output).unwrap(),
        ProviderFrame::Refused {
            reason: "capability:WrongScope".into()
        }
    );

    for (request, probe, expected_error) in [
        (
            SupervisorFrame::ProbeRawSibling,
            "raw-sibling",
            libc::EACCES,
        ),
        (
            SupervisorFrame::ProbeProcessSpawn,
            "process-spawn",
            libc::EPERM,
        ),
        (
            SupervisorFrame::ProbeNetworkSocket,
            "network-socket",
            libc::EPERM,
        ),
    ] {
        write_frame(&mut input, &request).unwrap();
        let ProviderFrame::ProbeDenied {
            probe: actual,
            os_error,
        } = read_frame::<_, ProviderFrame>(&mut output).unwrap()
        else {
            panic!("provider did not report a mechanical denial");
        };
        assert_eq!(actual, probe);
        assert_eq!(os_error, expected_error);
    }

    write_frame(&mut input, &SupervisorFrame::Revoke).unwrap();
    assert_eq!(
        read_frame::<_, ProviderFrame>(&mut output).unwrap(),
        ProviderFrame::Revoked
    );
    write_frame(
        &mut input,
        &SupervisorFrame::Read {
            claim: Box::new(claim(&issue)),
        },
    )
    .unwrap();
    assert_eq!(
        read_frame::<_, ProviderFrame>(&mut output).unwrap(),
        ProviderFrame::Refused {
            reason: "capability:Revoked".into()
        }
    );
    write_frame(&mut input, &SupervisorFrame::Shutdown).unwrap();
    assert_eq!(
        read_frame::<_, ProviderFrame>(&mut output).unwrap(),
        ProviderFrame::Stopped
    );
    drop(input);
    assert!(child.wait().unwrap().success());

    // A replacement provider owns a fresh private table and generation. The
    // old Play claim remains visible data but cannot operate on the new channel.
    let mut replacement_issue = issue.clone();
    replacement_issue.scope.base_provider_generation = 2;
    replacement_issue.authority.base_provider_generation = 2;
    let mut replacement = spawn_provider(&executable).unwrap();
    let mut replacement_input = replacement.stdin.take().unwrap();
    let mut replacement_output = BufReader::new(replacement.stdout.take().unwrap());
    write_frame(
        &mut replacement_input,
        &SupervisorFrame::Bootstrap {
            protocol_version: PROTOCOL_VERSION,
            issue: Box::new(replacement_issue),
            allowed_directory: allowed.display().to_string(),
            allowed_file: allowed.join("input.txt").display().to_string(),
            protected_sibling: protected.join("sentinel.txt").display().to_string(),
        },
    )
    .unwrap();
    assert!(matches!(
        read_frame::<_, ProviderFrame>(&mut replacement_output).unwrap(),
        ProviderFrame::Ready { .. }
    ));
    write_frame(
        &mut replacement_input,
        &SupervisorFrame::Read {
            claim: Box::new(claim(&issue)),
        },
    )
    .unwrap();
    assert_eq!(
        read_frame::<_, ProviderFrame>(&mut replacement_output).unwrap(),
        ProviderFrame::Refused {
            reason: "capability:WrongScope".into()
        }
    );
    write_frame(&mut replacement_input, &SupervisorFrame::Shutdown).unwrap();
    assert_eq!(
        read_frame::<_, ProviderFrame>(&mut replacement_output).unwrap(),
        ProviderFrame::Stopped
    );
    drop(replacement_input);
    assert!(replacement.wait().unwrap().success());

    assert_eq!(
        fs::read(protected.join("sentinel.txt")).unwrap(),
        b"protected-sentinel"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn ipc_rejects_oversized_and_malformed_frames_before_provider_decode() {
    let mut oversized = Vec::new();
    oversized.extend_from_slice(&((4097_u32).to_le_bytes()));
    assert_eq!(
        read_frame::<_, SupervisorFrame>(&mut oversized.as_slice())
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidData
    );

    let mut malformed = Vec::new();
    malformed.extend_from_slice(&(1_u32.to_le_bytes()));
    malformed.write_all(b"{").unwrap();
    assert_eq!(
        read_frame::<_, SupervisorFrame>(&mut malformed.as_slice())
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::Other
    );
}
