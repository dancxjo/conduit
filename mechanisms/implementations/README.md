# Reusable implementation mechanisms

This directory owns reusable implementation mechanics that are neither
portable semantic meaning nor an exact host/board product. Current examples
are the fixed-storage reference synthesizer, the cross-target linear
framebuffer make contribution, and the licensed bounded Unifont and
Lucide corpora shared by finite graphical realizations.

Target packages may consume these mechanisms, but mechanisms do not own host,
boot, body, plan, play, application, or target-make orchestration truth.
