//! Bounded signed owner carrier; replies do not establish a Body start.
use super::control;
use ed25519_dalek::{Signer, SigningKey};
use std::{
    error::Error,
    fs,
    io::{self, BufRead, Read, Write},
    path::PathBuf,
};
pub fn document(path: PathBuf) -> Result<serde_json::Value, Box<dyn Error>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 128 * 1024 {
        return Err("owner document bound".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn exchange(
    key: &SigningKey,
    request: control::Request<'_>,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let expected_operation = serde_json::to_value(request.operation)?;
    let expected_sequence = request.sequence;
    let signature = key
        .sign(
            &request
                .transcript()
                .map_err(|e| format!("transcript: {e:?}"))?,
        )
        .to_bytes();
    println!(
        "{}",
        serde_json::json!({"event":"owner-command", "request": request, "signature":signature.as_slice()})
    );
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().take(24577).read_line(&mut line)?;
    if line.len() > 24576 || !line.ends_with('\n') {
        return Err("owner reply bound".into());
    }
    let reply: serde_json::Value = serde_json::from_str(&line)?;
    if reply["event"] != "native-owner-control"
        || reply["authenticated"] != true
        || reply["operation"] != expected_operation
        || reply["sequence"] != expected_sequence
    {
        return Err("Root did not authenticate owner command".into());
    }
    Ok(reply)
}
