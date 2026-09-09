# ROS 2 topic Base

Status: native ROS 2 Jazzy topic vertical and bounded Base contract for #3077.

ROS is realization, never Conduit meaning. `RosTopicBase` is configured with a
finite list of directional mappings. Each mapping retains the exact ROS topic,
interface type, QoS reliability/durability/history depth, payload/queue bounds,
semantic Kind, adapter/Base identity, and authority requirement. Topic names,
message packages, DDS details, and origin parameters do not enter Forms.

The first supported mapping is bounded `std_msgs/msg/String`. Its codec rejects
wrong types, invalid lengths, invalid UTF-8, and payload overflow. Subscription
and publication use separate #3072 capability tables and operation contracts;
a read capability cannot publish and a write capability cannot select another
topic. Provider lifecycle loss removes current availability without changing
Body identity.

Discovery is diagnostic observation only. Only an exact configured import can
be admitted, ROS nodes never become Conduit Hosts, and an outward manifestation
carries the generic interop origin identity from #3099. Rediscovery therefore
does not create an independent import or authority by default.

Run `cargo xtask check ros2-base`. Its deterministic tier attacks direction,
type, bounds, sibling selection, revocation, and lifecycle. Its native tier
runs the same production Base boundary against the pinned
`ros:jazzy-ros-core` rclpy implementation with selected input/output topics and
an independently observed sibling-output sentinel. The selected inbound value
then becomes the literal input of an ordinary checked, ROS-independent text
Form, runs through the shared planner and std kernel, and returns through the
selected native ROS publisher. Docker is an internal proof mechanism;
`cargo xtask` remains the repository entrance.

The native fixture proves ROS topic transport, not physical actuation or DDS
hostile-process confinement. It does not disable ROS security, expose arbitrary
messages as bytes, import the graph, provide services/actions/tf2/ros2_control,
or grant graph-wide management. Those families require their own semantic and
lifecycle-compatible mappings rather than widening this topic Base.
