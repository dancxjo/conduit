#!/usr/bin/env python3
"""Native rclpy fixture for the narrow Conduit ROS topic Base proof."""

import json
import os
import select
import sys
import time

import rclpy
from rclpy.node import Node
from rclpy.qos import DurabilityPolicy, HistoryPolicy, QoSProfile, ReliabilityPolicy
from std_msgs.msg import String


class Fixture(Node):
    def __init__(self):
        super().__init__("conduit_ros2_topic_fixture")
        qos = QoSProfile(
            history=HistoryPolicy.KEEP_LAST,
            depth=4,
            reliability=ReliabilityPolicy.RELIABLE,
            durability=DurabilityPolicy.VOLATILE,
        )
        self.selected = None
        self.output = None
        self.sibling_output = None
        self.create_subscription(String, "/fixture/input", self._selected, qos)
        self.create_subscription(String, "/fixture/output", self._output, qos)
        self.create_subscription(String, "/fixture/sibling_output", self._sibling_output, qos)
        self.input_publisher = self.create_publisher(String, "/fixture/input", qos)
        self.sibling_publisher = self.create_publisher(String, "/fixture/sibling", qos)
        self.output_publisher = self.create_publisher(String, "/fixture/output", qos)

    def _selected(self, message):
        self.selected = message.data

    def _output(self, message):
        self.output = message.data

    def _sibling_output(self, message):
        self.sibling_output = message.data


def spin_until(node, predicate, seconds=8.0):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline and not predicate():
        rclpy.spin_once(node, timeout_sec=0.05)
    return predicate()


def main():
    rclpy.init()
    node = Fixture()
    time.sleep(0.5)
    selected = String()
    selected.data = "hello from ros"
    sibling = String()
    sibling.data = "protected sibling"
    for _ in range(4):
        node.input_publisher.publish(selected)
        node.sibling_publisher.publish(sibling)
        rclpy.spin_once(node, timeout_sec=0.1)
    if not spin_until(node, lambda: node.selected is not None):
        raise RuntimeError("selected ROS subscription did not receive its positive control")
    print(json.dumps({"selected": node.selected, "sibling_sent": sibling.data}), flush=True)

    ready, _, _ = select.select([sys.stdin], [], [], 10.0)
    if not ready:
        raise RuntimeError("Conduit ROS Base did not return its selected output")
    command = json.loads(sys.stdin.readline())
    if command.get("topic") != "/fixture/output":
        raise RuntimeError("provider received an unauthorized output topic")
    output = String()
    output.data = command["text"]
    node.output_publisher.publish(output)
    if not spin_until(node, lambda: node.output is not None):
        raise RuntimeError("native ROS output subscriber did not observe publication")
    print(
        json.dumps(
            {
                "output": node.output,
                "sibling_output": node.sibling_output,
                "origin": command["origin"],
            }
        ),
        flush=True,
    )
    node.destroy_node()
    rclpy.shutdown()
    os._exit(0)


if __name__ == "__main__":
    main()
