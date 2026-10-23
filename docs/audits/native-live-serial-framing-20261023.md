# Complete live serial records (#6273)

Candidate 39910308318, tested merge preview
`41a5287a141d1182862b4591ec9b8292e38e2382`, failed its normal x86 journey
with `face-journey-io: stream did not contain valid UTF-8`. The retained failed
artifact 12483523202 has ZIP SHA256
`a5c8de4b0d84fe1a48f5da5d5d225aced620d532e433a6924f5fb46bcf7e904b`.
The complete 1,217,252-byte log is valid UTF-8; its complete-record prefix ends
at byte 1,214,999. An append-only live read had ended inside a multibyte scalar.
The original failure, raw serial, QMP records and captured frames remain intact.

The existing journey-input decoder already enforced newline-completed records.
That boundary now lives in one shared `live_serial` responsibility. Live file
reads truncate only the unfinished final record and decode the completed prefix
strictly, reusing the allocated read buffer. Journey, Face, graph, selection,
editor and stage-count observations use it. Invalid completed UTF-8 still returns
`InvalidData`; file errors and caller refusal labels remain intact. No lossy
replacement, input replay, retry, deadline increase or guest budget change was
introduced.

Two actual file-read regressions pass, covering every byte split of a record
containing Vietnamese, accented Latin, CJK and a four-byte scalar; they also
prove the former whole-file read fails at intermediate splits. Invalid completed
data remains an error even beside a partial tail, and missing files remain
`NotFound`. The seven existing journey-input controls pass, including strict
completed-record decoding, guest-record-correlated release and failure cleanup.
The owning xtask test executable and second command entrance compile, and
workspace formatting and whitespace checks pass.

These are focused CPU/file-I/O checks. Fresh current-source normal QEMU,
Candidate, merge and relevant native Integration remain required; no new local
QEMU success, globally green CI, physical or human acceptance is claimed here.
