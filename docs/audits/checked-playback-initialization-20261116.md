# Checked playback initialization: source handoff

This source checkpoint extracts the existing checked `audio/speaker` selection into a crate-visible normal-IMAGE initializer. It reuses the current Make schema and ALSA provider. It checks the selected executable before and after metadata discovery, requires one exact card/device/base identity, and retains the actual Boot and offer generation. It publishes no playback authority and opens no PCM stream.

The new initializer and six focused tests are **not compiled or executed yet**. Rustfmt and diff hygiene passed. Normal IMAGE integration is intentionally pending the existing funded Std construction lane: include playback in InitialBases preparation, bind its non-authorizing checked provider entry using the same original IMAGE/counter, and retain the selected provider on the initialized Host. Keep original budgets and independent explicit playback grants. Do not copy an old whole image_initialization cohort into current dev.

The existing product selected_host_configuration/playback.rs should become a thin user of this same preparation logic rather than maintain a second selector. Its current before/after executable checks must survive. The private InitialPlayback helper is not an installed provider until that normal pipeline is connected and qualified.

Required qualification in the owning #6886 lane: actual checked Host description through normal IMAGE construction; missing/changed/ambiguous endpoint and executable rejection; actual supported planner grant/admission; protected Started-to-joined-member chime binding; bounded PCM and SDL/speaker coexistence in the same admitted IMAGE. Synthetic selector tests are not hardware, audibility or deployment proof.
