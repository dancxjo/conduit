# Dependency commit custody at the speech boundary

`LanguageParserStableDependencyAdmission` correlates an exact portable arc with
the complete parser stable fact, including its analysis, lexical choices,
dependent, head, relation base and subtype. Stability alone does not establish
that the contiguous parser commit operation advanced.

`LanguageParserCommittedDependencyAdmission` additionally retains the exact
commit query and runtime output. Source contracts require the fact to match and
every candidate's lexical choices, parser identity, score and graph state to be
preserved, with the committed frontier advancing once for each active candidate.
An otherwise valid uncommitted runtime is refused. The runtime epoch remains
bound by its existing Source law to the exact lexical source sequence.
The retained stable-fact laws also require all active candidates to agree on the
dependent's lexical choice and its governor's choice, and require the token
spans to fit the stable source prefix. Reading candidate zero's choice therefore
projects admitted consensus; it does not choose a pronunciation in Rust.

The `parser_committed_dependency` test uses the retained actual Partial-source
session trace and checks acceptance and uncommitted-snapshot refusal. Its
dynamic contract check passed in 108.94 seconds and passed again after binding
generation in 110.60 seconds. The committed speech continuation passed in
57.67 seconds, retaining the complete 142,093-byte committed Native admission.

The production `committed_token_role` entry point retains a borrow of the whole
committed admission with its prepared Source token role. It derives the original
dependent and lexical choice mechanically and requires the complete lexical
tape to match. Its separate receipt replay compares the exact Source role
request/result bytes and checks refusal of a different lexical revision.
Module wiring and this replay are pending validation.

The ignored `stable_vocative_continuation` test reconstructs the exact lexical
tape from revision history and requires the committed admission before preparing
the dependent's pronunciation, linguistic pitch, realization and playback tape.
Receipts retain the complete committed admission bytes. These are the early
Travis token at ordinal zero; they do not substitute for the separate
`Hello, Travis.` vocative at ordinal two.

Playback acknowledgment in this fixture is supplied by the effect-owner test
fixture. Actual PCM rendering, refusal under pressure/cancellation, and this
acknowledgment are distinct from physical playback, later protected parser
revision, native neural utterance synthesis, or human listening proof. None of
those are established by this test alone. Earlier stable-only receipts retain
their original false commit-custody flag.
