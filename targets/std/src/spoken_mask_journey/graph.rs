//! Ordinary Mask composition; streaming changes admitted work, not queue size.
pub(crate) fn source(
    plot_name: &str,
    maximum_output_bytes: u32,
    streaming: bool,
    language: &conduit_language::LanguageRequest,
) -> String {
    let language_request = conduit_language::language_request_literal(language);
    let projection = if streaming {
        "presentation/generated-manifestation-speech-stream"
    } else {
        "presentation/generated-manifestation-speech"
    };
    let synthesis = if streaming {
        format!("commit: speech/commit-generated-text\n voice: speech/synthesize-stream(language-request = {language_request}, maximum-output-bytes = {maximum_output_bytes}, maximum-audio-millis = 30000, maximum-segments = 32)")
    } else {
        format!("voice: speech/synthesize(language-request = {language_request}, maximum-output-bytes = {maximum_output_bytes})")
    };
    let work = if streaming {
        ", maximum-blocks = 32768, maximum-audio-millis = 30000"
    } else {
        ""
    };
    let artifact_work = if streaming {
        "(maximum-blocks = 32768, maximum-audio-millis = 30000)"
    } else {
        ""
    };
    let speech_cord = if streaming {
        "speech.speech >> commit.generated\n commit.segments >> voice.text"
    } else {
        "speech.speech >> voice.text"
    };
    format!(
        r#"plot {plot_name} (
 >> face: Presentation
 interaction: FaceInteraction...| >>
 show: Show >>
) {{
 request: presentation/adapt-generative-request
 language: llm/present(16384, 1, 4096, 16384, 0)
 envelope: presentation/build-generated-validation-envelope
 validator: presentation/generated-semantic-validator
 accepted: presentation/retain-generated-validation
 speech: {projection}
 {synthesis}
 convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = "stereo-left-right"{work})
 artifact: presentation/spoken-artifact{artifact_work}
 shown: presentation/artifact-acknowledged-show
 no-input: presentation/no-interaction
 face >> request.presentation
 request.request >> language.request
 request.request >> envelope.request
 language.result >> envelope.candidate
 language.result >> accepted.candidate
 envelope.envelope >> validator.envelope
 validator.assessment >> accepted.assessment
 accepted.manifestation >> speech.manifestation
 accepted.manifestation >> shown.manifestation
 {speech_cord}
 voice.audio >> convert.audio
 convert.converted >> artifact.audio
 artifact.receipt >> shown.artifact
 shown.show >> show
 no-input.interaction >> interaction
}}
"#
    )
}
