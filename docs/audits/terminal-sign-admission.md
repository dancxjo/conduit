# Admit terminal lifecycle Signs before committing the Step

Development slice for #5231's pressure and exact Sign obligations. A normal or
abnormal terminal Step previously committed staged work and called the Back's
commit hook before its output publisher checked remote lifecycle Sign capacity.
The completed flag was also set before that check. Separately, the ordinary
terminal budget counted each output route once, although a remote normal-close
or abnormal-terminal track produces both a ValueRouted Sign and a remote closure
Sign. Near sequence exhaustion, publication could therefore fail after commit.

Terminal admission now counts routed terminal values and remote closure records
separately, checks the remote budget, and includes both in the ordinary Sign
reservation before Step commit. Normal completion visits every output port;
abnormal termination visits only its declared terminal output. An abnormal
output without an abnormal track still ends at its semantic boundary without
closing sibling tracks, matching the publisher. The existing completion or
semantic-abnormal Sign and recovery records are reserved by the Step caller.
No runtime state or allocation is added.

Two fixed-storage native scheduler regressions each cover normal and abnormal
outcomes over two remote terminal tracks. One supplies room for only one remote
closure; the other starts the ordinary sequence with four positions remaining,
where the old reservation admitted the Step but not its complete publication.
Both reproduced a committed Back before refusal. After repair, the Back commit
hook is untouched, the node remains incomplete and ready, queues and closure
flags remain unchanged, and no value is allocated. The one StepFuelGranted Sign
for the attempted invocation remains truthful evidence. Supplying a fresh test
Sign sink with capacity permits retry and emits exactly two routed values, two
remote closures, and the final terminal Sign, plus the retry's fuel grant.

This fixes terminal Sign admission. Other terminal queue/value pressure and
arbitrary failing Back/storage implementations are not established as whole-Step
transactions by these tests. No wall-clock improvement, physical evidence, stable
acceptance, or parent-issue completion is claimed.

Local validation: both focused fixed-only regressions passed; alloc and
linear-ready-reference kernel suites each passed 197 tests. Formatting, strict
alloc all-target kernel Clippy, and diff whitespace checks passed. No CI was
dispatched or awaited.
