# Declared contextual IPA profile G1@1

The finite conformance specimen in `semantics/speech/tests/contextual_ipa_profile.rs`
uses the manually supplied text `tata tata tata`, phonemic /tɑtɑ/, and an explicit
versioned IPA profile `g1/contextual-ipa@1`. Its declared Variety is
`g1/declared-careful-tata`, with inventory revision `g1/inventory/1`.
The spelling `tata` names manually declared /tɑtɑ/; [ɑ] matches the existing
compact voice binding. This is a small declared profile, not a universal English pronunciation rule.

The inventory selects [tʰ] for word-initial /t/ and default [t] for medial /t/.
The specimen retains primary versus unstressed syllables, source occurrence
identity, the original phoneme, and the original Known-phone constraints.
Selection validates those supplied phone constraints; it does not generate or
rewrite the original phonetic tape. The existing contextual-intent tests separately
exercise deriving these choices when the original phone is unspecified.

The full carrier joins admitted Unicode IPA, phoneme/phone correspondence,
syllables, lexical words, and an executed existing Language parser commitment.
The test compares the original carrier identity and canonical bytes, commitment
request/result bytes, stress, and borrowed source occurrences after selection.
It refuses an eligible unsupported context and a foreign inventory at the same
selection entrance. Disabling aspiration cannot silently substitute plain [t]
for an original Known [tʰ] constraint.

The commitment is a real execution of the existing finite parser fixture with
three legal transitions. The lexical and IPA material is manually declared;
this does not establish automatic text-to-pronunciation inference, playback
authority, PhonoGesture gesture generation, or a correction to heteronym stress
such as noun versus verb “record”.

Run the focused target with:

```sh
cargo +stable test --locked -p conduit-speech --features semantic-bindings,kernel --test contextual_ipa_profile
```

The target is also registered in the supported `cargo +stable xtask check speech-ipa`
entrance. Registration alone does not claim that the full suite has passed.
