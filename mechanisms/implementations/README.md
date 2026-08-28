# Reusable implementation mechanisms

This directory owns reusable implementation mechanics that are neither
portable semantic meaning nor an exact Host/board product. Current examples
are the fixed-storage reference synthesizer and the cross-target linear
framebuffer fabrication contribution.

Target packages may consume these mechanisms, but mechanisms do not own Host,
Boot, Body, Plan, Play, application, or target-fabrication orchestration truth.
