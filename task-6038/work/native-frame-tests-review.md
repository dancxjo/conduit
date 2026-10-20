# Independent review of native frame fixture repair

Source03656e600, reviewed by native_focus_proof. No blocker. Current binary maximum32754B; document JSON adds overhead, so its body of32754B necessarily exceeds the32710B first document payload while fitting64KiB assembly. Explicit next<document.len assertion keeps the missing-next-frame case meaningful. Second-frame byte44 is payload (44-byte wire header), so tampering retains metadata and fails digest validation as Decode.

Return payload is32714B per frame;32715B forces exactly two frames with a one-byte final payload. Existing total/offset/digest checks remain, exact reconstructed bytes are added, and pressure64KiB+1 must leave sent frame count unchanged. Edits are tests only; no production limits or execution paths change.

Original fullworkspace partial result8894pass/2fail/42ignore, stopping at native1180pass/2fail/2ignore, is retained separately and remains red. Focused validation running; this review is not execution proof.
