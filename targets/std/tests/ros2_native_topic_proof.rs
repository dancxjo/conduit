use conduit_core::*;
use conduit_std_host::ros2_base::*;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[path = "ros2_base_security.rs"]
mod security_support;

struct DockerRosProvider<'a> {
    input: &'a mut dyn Write,
}

impl NativeRosTopicProvider for DockerRosProvider<'_> {
    fn publish(
        &mut self,
        topic_name: &str,
        _interface_type: &str,
        _qos: RosQos,
        encoded: &[u8],
        origin: &str,
    ) -> Result<(), RosBaseRefusal> {
        let text = decode_ros_string(encoded)?;
        writeln!(
            self.input,
            "{}",
            serde_json::json!({"topic": topic_name, "text": text, "origin": origin})
        )
        .map_err(|_| RosBaseRefusal::Provider)?;
        self.input.flush().map_err(|_| RosBaseRefusal::Provider)
    }
}

#[test]
fn native_ros_topic_round_trip_has_no_sibling_or_reflection_effect() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../proof/ros2/topic_base_fixture.py")
        .canonicalize()
        .unwrap();
    let mount = format!("{}:/proof.py:ro", script.display());
    let container_id = format!("conduit-ros2-proof-{}", std::process::id());
    let created = Command::new("docker")
        .args([
            "create",
            "--name",
            &container_id,
            "-i",
            "--network",
            "host",
            "-v",
            &mount,
            "ros:jazzy-ros-core",
            "python3",
            "/proof.py",
        ])
        .stdout(Stdio::null())
        .status()
        .expect("Docker is required for this release/HIL tier");
    assert!(created.success(), "failed to create ROS proof container");
    let mut child = Command::new("docker")
        .args(["start", "-a", "-i", &container_id])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start pinned ROS Jazzy proof container");
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let inbound: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(inbound["sibling_sent"], "protected sibling");

    let mut base = RosTopicBase::prepare(vec![
        security_support::topic(
            "input",
            "/fixture/input",
            InteropDirection::ExternalToConduit,
        ),
        security_support::topic(
            "output",
            "/fixture/output",
            InteropDirection::ConduitToExternal,
        ),
    ])
    .unwrap();
    assert_eq!(
        base.consider_discovery("/fixture/sibling", 64, None),
        ImportDecision::Unconfigured
    );
    let encoded = encode_ros_string(inbound["selected"].as_str().unwrap(), 68).unwrap();
    let mut subscribe = security_support::authority("input", "conduit.host/ros2-subscribe@1");
    let semantic_text = base
        .import_string(
            &InteropMappingId::from("input"),
            ROS_STRING_TYPE,
            &encoded,
            &mut subscribe,
        )
        .unwrap();
    let transformed = semantic_text.to_uppercase();
    let mut publish = security_support::authority("output", "conduit.host/ros2-publish@1");
    let manifestation = base
        .publish_string(
            &InteropMappingId::from("output"),
            &transformed,
            &mut publish,
            &mut DockerRosProvider { input: &mut input },
        )
        .unwrap();

    line.clear();
    output.read_line(&mut line).unwrap();
    let outbound: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(outbound["output"], "HELLO FROM ROS");
    assert!(outbound["sibling_output"].is_null());
    assert_eq!(
        outbound["origin"],
        manifestation.origin.manifestation_id.as_str()
    );
    drop(input);
    assert!(child.wait().unwrap().success());
    assert!(Command::new("docker")
        .args(["rm", &container_id])
        .stdout(Stdio::null())
        .status()
        .unwrap()
        .success());
}
