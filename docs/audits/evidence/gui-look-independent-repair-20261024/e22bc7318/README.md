# Independent GUI Look repair proof

Exact original PR head: 3a416f499ec46a2af7beb92b5b2c2b3990099095.
Exact isolated repair: e22bc731803c074d7b235c6518fc747fb4b4ec68.

Original strengthened typography regression failed once; both original forced-color cases failed. Corrected complete owner Face Chromium fixture suite passed all 104 tests with one worker and zero retries. Raw logs and original source/hash review are retained.

PNG captures show the actual repaired renderer with explicitly identified fixture data, Phosphor palette and engineering typography, in normal and forced-color modes. They are not live Body, native QEMU, physical, CI or publication acceptance.

The owner subsequently advanced independently to 849c54e and removed the same typography palette overwrite. The forced-color specificity defect remains in that exact source; tests were not rerun there. Do not overwrite the active owner branch with this older recovery branch. Original issues 6349, 6174 and 5366 remain open.
