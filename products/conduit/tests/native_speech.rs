//! Public checker entrance for the native voice and exact PCM Fore alias.
use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[test]
fn public_checker_accepts_native_speech_and_pcm_flow() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "conduit-native-voice-{}-{nonce}.conduit",
        std::process::id()
    ));
    fs::write(&path, "plot voice (\n >> text: Text <= 512B\n audio: PcmFrames...| <= 285B >>\n) {\n speech: speech/english-utterance(clock=7)\n text >> speech.text\n speech.audio >> audio\n}.\n").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(&path).unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
