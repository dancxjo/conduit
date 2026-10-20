The stage05 panel PR #6055 remains draft while its exact Candidate finishes. All five native target checks passed; browser and admission were still running at the latest scheduled observation.

A separate full workspace run found integration fallout from the accepted #6054 frame-bound repair: two native chunk tests still assumed that a 20,000-byte payload requires multiple frames. With the 32 KiB frame, that fixture fits in one frame. Repair #6140 derives the fixtures from the actual admitted capacity, retaining truncation/digest refusal and adding exact reassembly and zero-send pressure assertions. It changes tests only. Focused owner tests passed 14/14 before the current-dev refresh; exact refreshed-commit checks are underway. The original full workspace result remains red, not waived or relabeled.

I am holding the stage05 merge until this integration repair is accepted. The ordered native queue is unchanged; no stage06 native implementation has started.
