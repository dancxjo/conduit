# Explicit syllable and boundary structure

Recovery for #5769 from archived draft #5273, exact preserved head
`6806f08fbb07723237441b97c93cebf5d1e72138`. The old syllable declarations,
preparation traversal and four tests were recovered independently of the draft's
contextual mapping, gestures and timing work. The original archived ref remains
preserved until this focused successor is accepted.

The current Source owner generates the Native bindings. The recovered component
retains the original intent and revisioned phone sequence; new companion relations
retain exact word references, Language/Variety, original universal IPA spelling
and every original IPA occurrence. Explicit IPA indices correlate supplied phone
occurrences with supplied notation; they do not infer phoneme/phone realization.

Preparation admits a finite profile of at most 64 syllables and 256 boundary
events. Each group has one explicitly indexed nucleus and ordered onset/nucleus/
coda positions. Known lexical stress requires that nucleus. Unknown and other
specification states remain explicit. Groups cannot duplicate an identity or
share a phone occurrence in this profile. Other linguistic sharing policies need
separately authored relations.

Word references resolve against the supplied immutable LanguageText and exact
text revision, with Unicode-scalar ranges. IPA is reparsed through the existing
qualified phonetic entrance; supplied original occurrences must equal its result.
Members must name actual phone events in increasing original notation order.
Phone, syllable, word, phrase, breath-group and turn boundaries retain supplied
positions in the exact phone basis. Position and source order are nondecreasing;
coincident different boundary events remain distinct. This does not invent pause
lengths, clocks or acoustic realization.

The two-word fixture uses explicit `tat tat` text, two `ˈtat` transcriptions,
primary lexical stress, six original phone occurrences, and all required boundary
kinds. Components serialize/decode and then execute preparation again. Original
text revision, phone revision, Variety and IPA occurrences remain inspectable.
Negative fixtures cover foreign text revision, conflicting membership, absent
stress nucleus, contradictory role order, non-phone/out-of-range IPA membership,
and reversed boundary order.

This is allocating linguistic preparation, not a Flow playback path. The older
optional seconds span is retained as supplied metadata; it is not used as a
linguistic source range. No contextual realization mapping or acoustic timing is
implemented here. The focused seven-test suite passes, including the original four recovered
tests and three new structure/refusal tests. Strict Clippy with warnings denied also passes. Merge admission remains pending.
