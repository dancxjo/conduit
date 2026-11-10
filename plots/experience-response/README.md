# Experience response

The Plot accepts explicit, typed `ExperienceEvidenceRequest` revisions over a
Flow. `experience/prepare-model-request` validates the selected evidence and
writes one bounded generation request for `llm/generate-flow`. It includes
classifications, sources, exact instants, and relationships. The output Fore
carries the model result envelope with its execution provenance.

The formatter uses a prepared 4,096-byte buffer. Oversize requests refuse
before a model call. A bounded Cord carries one request at a time in the
hosted proof. The model Back admits one call in flight.

Run `cargo xtask check pete-cognition` to exercise two revisions through the
installed hosted kernel and a fenced fake model. `cargo xtask check pete-cognition-live` also checks a selected historical fact through the local
Ollama Back with an installed `gemma3:1b`. This Plot accepts prepared evidence. Pete's runtime Situation, autobiography feedback, and provider-loss
recovery still require wiring in the continuing Body.
