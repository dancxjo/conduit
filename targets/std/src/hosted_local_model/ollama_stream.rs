//! Bounded incremental Ollama generation below the portable text Flow.

use super::StreamingChunkDisposition;
use serde::Deserialize;
use serde_json::json;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdout, Command, Stdio};

const MAXIMUM_LINE_BYTES: usize = conduit_ai::MAXIMUM_GENERATED_TEXT_CHUNK_BYTES + 8 * 1024;

#[derive(Deserialize)]
struct GenerateChunk {
    response: String,
    #[serde(default)]
    done: bool,
}

pub(super) fn generate(
    endpoint: &str,
    timeout_seconds: &str,
    model_name: &str,
    input: &str,
    maximum_tokens: u64,
    maximum_output_bytes: u64,
    sink: &mut dyn FnMut(&conduit_ai::GeneratedTextChunk) -> StreamingChunkDisposition,
) -> conduit_ai::GeneratedTextFlowEvidence {
    let Ok(mut session) = Session::spawn(
        endpoint,
        timeout_seconds,
        model_name,
        input,
        maximum_tokens,
        maximum_output_bytes,
    ) else {
        return empty(conduit_ai::GeneratedTextFlowTerminal::ProviderLost);
    };
    loop {
        match session.next() {
            Step::Chunk(chunk) => match sink(&chunk) {
                StreamingChunkDisposition::Accepted => {}
                StreamingChunkDisposition::Backpressured => {
                    return session.finish(conduit_ai::GeneratedTextFlowTerminal::Backpressured);
                }
                StreamingChunkDisposition::Cancel => {
                    return session.finish(conduit_ai::GeneratedTextFlowTerminal::Cancelled);
                }
            },
            Step::Terminal(evidence) => return evidence,
        }
    }
}

pub(super) enum Step {
    Chunk(conduit_ai::GeneratedTextChunk),
    Terminal(conduit_ai::GeneratedTextFlowEvidence),
}

pub(super) struct Session {
    child: Child,
    reader: BufReader<ChildStdout>,
    flow: conduit_ai::BoundedGeneratedTextFlow,
    finished: bool,
    done_after_chunk: bool,
}

impl Session {
    pub(super) fn spawn(
        endpoint: &str,
        timeout_seconds: &str,
        model_name: &str,
        input: &str,
        maximum_tokens: u64,
        maximum_output_bytes: u64,
    ) -> Result<Self, conduit_ai::GeneratedTextFlowEvidence> {
        let request = json!({
            "model": model_name,
            "prompt": input,
            "stream": true,
            "keep_alive": "5m",
            "options": { "num_predict": maximum_tokens }
        });
        let Ok(body) = serde_json::to_vec(&request) else {
            return Err(empty(conduit_ai::GeneratedTextFlowTerminal::ProviderLost));
        };
        let url = format!("{endpoint}/api/generate");
        let mut command = Command::new("curl");
        command.args([
            "--silent",
            "--show-error",
            "--fail",
            "--no-buffer",
            "--max-time",
            timeout_seconds,
            "--header",
            "content-type: application/json",
            "--data-binary",
            "@-",
            &url,
        ]);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let Ok(mut child) = command.spawn() else {
            return Err(empty(conduit_ai::GeneratedTextFlowTerminal::ProviderLost));
        };
        if child
            .stdin
            .take()
            .and_then(|mut stdin| stdin.write_all(&body).ok())
            .is_none()
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(empty(conduit_ai::GeneratedTextFlowTerminal::ProviderLost));
        }
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(empty(conduit_ai::GeneratedTextFlowTerminal::ProviderLost));
        };
        let Some(flow) = conduit_ai::BoundedGeneratedTextFlow::new(maximum_output_bytes) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(empty(
                conduit_ai::GeneratedTextFlowTerminal::OutputBoundExhausted,
            ));
        };
        Ok(Self {
            child,
            reader: BufReader::new(stdout),
            flow,
            finished: false,
            done_after_chunk: false,
        })
    }

    pub(super) fn next(&mut self) -> Step {
        if self.finished {
            return Step::Terminal(
                self.flow
                    .finish(conduit_ai::GeneratedTextFlowTerminal::ProviderLost),
            );
        }
        if self.done_after_chunk {
            let status = self.child.wait();
            return Step::Terminal(self.finish(if status.is_ok_and(|status| status.success()) {
                conduit_ai::GeneratedTextFlowTerminal::Completed
            } else {
                conduit_ai::GeneratedTextFlowTerminal::ProviderLost
            }));
        }
        loop {
            let mut line = String::new();
            let read = self
                .reader
                .by_ref()
                .take((MAXIMUM_LINE_BYTES + 1) as u64)
                .read_line(&mut line);
            let Ok(read) = read else {
                return Step::Terminal(
                    self.finish(conduit_ai::GeneratedTextFlowTerminal::ProviderLost),
                );
            };
            if read == 0 || read > MAXIMUM_LINE_BYTES || !line.ends_with('\n') {
                return Step::Terminal(self.finish(if read > MAXIMUM_LINE_BYTES {
                    conduit_ai::GeneratedTextFlowTerminal::OutputBoundExhausted
                } else {
                    conduit_ai::GeneratedTextFlowTerminal::ProviderLost
                }));
            }
            let Ok(response) = serde_json::from_str::<GenerateChunk>(&line) else {
                return Step::Terminal(
                    self.finish(conduit_ai::GeneratedTextFlowTerminal::ProviderLost),
                );
            };
            if !response.response.is_empty() {
                let chunk = conduit_ai::GeneratedTextChunk {
                    sequence: self.flow.next_sequence(),
                    text: response.response,
                };
                if self.flow.admit(&chunk).is_err() {
                    return Step::Terminal(
                        self.finish(conduit_ai::GeneratedTextFlowTerminal::OutputBoundExhausted),
                    );
                }
                self.done_after_chunk = response.done;
                return Step::Chunk(chunk);
            }
            if response.done {
                let status = self.child.wait();
                return Step::Terminal(self.finish(
                    if status.is_ok_and(|status| status.success()) {
                        conduit_ai::GeneratedTextFlowTerminal::Completed
                    } else {
                        conduit_ai::GeneratedTextFlowTerminal::ProviderLost
                    },
                ));
            }
        }
    }

    pub(super) fn finish(
        &mut self,
        terminal: conduit_ai::GeneratedTextFlowTerminal,
    ) -> conduit_ai::GeneratedTextFlowEvidence {
        self.finished = true;
        if terminal != conduit_ai::GeneratedTextFlowTerminal::Completed {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.flow.finish(terminal)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.finished = true;
        }
    }
}

pub(super) fn empty(
    terminal: conduit_ai::GeneratedTextFlowTerminal,
) -> conduit_ai::GeneratedTextFlowEvidence {
    conduit_ai::GeneratedTextFlowEvidence {
        chunks: 0,
        generated_bytes: 0,
        terminal,
        retained_private_text: false,
    }
}

#[cfg(test)]
fn decode(
    reader: impl BufRead,
    maximum_output_bytes: u64,
    sink: &mut dyn FnMut(&conduit_ai::GeneratedTextChunk) -> StreamingChunkDisposition,
) -> conduit_ai::GeneratedTextFlowEvidence {
    let Some(mut flow) = conduit_ai::BoundedGeneratedTextFlow::new(maximum_output_bytes) else {
        return empty(conduit_ai::GeneratedTextFlowTerminal::OutputBoundExhausted);
    };
    let mut saw_done = false;
    for line in reader.lines() {
        let Ok(line) = line else {
            return flow.finish(conduit_ai::GeneratedTextFlowTerminal::ProviderLost);
        };
        if line.len() > conduit_ai::MAXIMUM_GENERATED_TEXT_CHUNK_BYTES + 8 * 1024 {
            return flow.finish(conduit_ai::GeneratedTextFlowTerminal::OutputBoundExhausted);
        }
        let Ok(response) = serde_json::from_str::<GenerateChunk>(&line) else {
            return flow.finish(conduit_ai::GeneratedTextFlowTerminal::ProviderLost);
        };
        if !response.response.is_empty() {
            let chunk = conduit_ai::GeneratedTextChunk {
                sequence: flow.next_sequence(),
                text: response.response,
            };
            if flow.admit(&chunk).is_err() {
                return flow.finish(conduit_ai::GeneratedTextFlowTerminal::OutputBoundExhausted);
            }
            match sink(&chunk) {
                StreamingChunkDisposition::Accepted => {}
                StreamingChunkDisposition::Backpressured => {
                    return flow.finish(conduit_ai::GeneratedTextFlowTerminal::Backpressured);
                }
                StreamingChunkDisposition::Cancel => {
                    return flow.finish(conduit_ai::GeneratedTextFlowTerminal::Cancelled);
                }
            }
        }
        if response.done {
            saw_done = true;
            break;
        }
    }
    flow.finish(if saw_done {
        conduit_ai::GeneratedTextFlowTerminal::Completed
    } else {
        conduit_ai::GeneratedTextFlowTerminal::ProviderLost
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn incremental_lines_cross_the_boundary_before_completion() {
        let input = concat!(
            "{\"response\":\"First sentence. \",\"done\":false}\n",
            "{\"response\":\"Later text.\",\"done\":false}\n",
            "{\"response\":\"\",\"done\":true}\n",
        );
        let mut chunks = Vec::new();
        let evidence = decode(Cursor::new(input), 64, &mut |chunk| {
            chunks.push(chunk.clone());
            StreamingChunkDisposition::Accepted
        });
        assert_eq!(chunks.len(), 2);
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.text.as_str())
                .collect::<String>(),
            "First sentence. Later text."
        );
        assert_eq!(
            evidence.terminal,
            conduit_ai::GeneratedTextFlowTerminal::Completed
        );
        assert_eq!(evidence.generated_bytes, 27);
        assert!(!evidence.retained_private_text);
    }

    #[test]
    fn pressure_cancellation_bounds_and_missing_close_remain_distinct() {
        let input = concat!(
            "{\"response\":\"one\",\"done\":false}\n",
            "{\"response\":\"two\",\"done\":true}\n",
        );
        let pressure = decode(Cursor::new(input), 64, &mut |_| {
            StreamingChunkDisposition::Backpressured
        });
        assert_eq!(
            pressure.terminal,
            conduit_ai::GeneratedTextFlowTerminal::Backpressured
        );
        let cancelled = decode(Cursor::new(input), 64, &mut |_| {
            StreamingChunkDisposition::Cancel
        });
        assert_eq!(
            cancelled.terminal,
            conduit_ai::GeneratedTextFlowTerminal::Cancelled
        );
        let overflow = decode(
            Cursor::new("{\"response\":\"long\",\"done\":false}\n"),
            3,
            &mut |_| StreamingChunkDisposition::Accepted,
        );
        assert_eq!(
            overflow.terminal,
            conduit_ai::GeneratedTextFlowTerminal::OutputBoundExhausted
        );
        let lost = decode(
            Cursor::new("{\"response\":\"ok\",\"done\":false}\n"),
            8,
            &mut |_| StreamingChunkDisposition::Accepted,
        );
        assert_eq!(
            lost.terminal,
            conduit_ai::GeneratedTextFlowTerminal::ProviderLost
        );
    }
}
