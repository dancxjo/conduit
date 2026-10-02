use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("time semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            serde_variant_exclusions: ["ReplayCommand".into()].into(),
            serde_record_types: [
                "AvailabilityBasis".into(),
                "AvailabilityInterval".into(),
                "CalendarEvent".into(),
                "CandidateConflict".into(),
                "CivilTrigger".into(),
                "ElapsedTrigger".into(),
                "LocalDate".into(),
                "LocalDateTime".into(),
                "LocalTime".into(),
                "MeetingCandidate".into(),
                "MeetingProposal".into(),
                "MeetingProposalRequest".into(),
                "MonotonicClockIdentity".into(),
                "MonotonicDuration".into(),
                "MonotonicInstant".into(),
                "NamedPatternTemplate".into(),
                "NamedPatternTemplateSlot".into(),
                "NamedTimeZone".into(),
                "NormalizedDurationSequence".into(),
                "Participant".into(),
                "ParticipantAvailability".into(),
                "ProposedMeetingSlot".into(),
                "RecurrenceDefinition".into(),
                "RecurrenceExpansion".into(),
                "RecurrenceOccurrence".into(),
                "RejectedMeetingSlot".into(),
                "ReminderOccurrence".into(),
                "ReminderSpecification".into(),
                "TemporalInstant".into(),
                "TemporalWindow".into(),
                "TimedCalendarSpan".into(),
            ]
            .into(),
            copy_record_types: [
                "CivilResolutionPolicy".into(),
                "LocalDate".into(),
                "LocalTime".into(),
                "MonotonicDuration".into(),
                "PulseObservation".into(),
                "RhythmState".into(),
            ]
            .into(),
            copy_nominal_types: ["WeekdaySet".into()].into(),
            serde_nominal_types: ["WeekdaySet".into()].into(),
            copy_record_value_getters: [
                "LocalDate".into(),
                "LocalTime".into(),
                "MonotonicDuration".into(),
            ]
            .into(),
            record_constructor_orders: [
                (
                    "LocalDate".into(),
                    vec!["year".into(), "month".into(), "day".into()],
                ),
                (
                    "LocalTime".into(),
                    vec![
                        "hour".into(),
                        "minute".into(),
                        "second".into(),
                        "nanosecond".into(),
                    ],
                ),
                (
                    "MonotonicDuration".into(),
                    vec!["ticks".into(), "scale".into()],
                ),
                (
                    "TemporalWindow".into(),
                    vec![
                        "start".into(),
                        "start_boundary".into(),
                        "end".into(),
                        "end_boundary".into(),
                    ],
                ),
            ]
            .into(),
            public_record_fields: [
                "AvailabilityBasis".into(),
                "AvailabilityInterval".into(),
                "CalendarEvent".into(),
                "CivilTrigger".into(),
                "ElapsedTrigger".into(),
                "LocalDateTime".into(),
                "MeetingCandidate".into(),
                "MeetingProposal".into(),
                "MeetingProposalRequest".into(),
                "Participant".into(),
                "ParticipantAvailability".into(),
                "ProposedMeetingSlot".into(),
                "RecurrenceDefinition".into(),
                "RecurrenceExpansion".into(),
                "RecurrenceOccurrence".into(),
                "RejectedMeetingSlot".into(),
                "ReminderOccurrence".into(),
                "ReminderSpecification".into(),
                "RhythmState".into(),
                "TemporalWindow".into(),
                "TemporalInstant".into(),
                "TimedCalendarSpan".into(),
            ]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("time semantic Types and codes must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated time bindings");
}
