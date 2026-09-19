# Embodied experience, perception, purpose, and Fulfillment

## Status

Architectural direction for continuous perception, bounded current experience,
first-person generative presentation, optional purpose, and appropriate
Fulfillment.

This document builds on existing accepted Conduit boundaries rather than
creating a new cognitive runtime. In particular:

- [continuous execution](continuous-execution.md) already establishes that a
  form may remain useful for an externally unbounded lifetime while every
  concrete state, queue, resource, plan, and play remains finite;
- [body lifecycle](body-lifecycle-waists.md) keeps body continuity distinct
  from host, boot, plan, play, resource, and line truth;
- the existing vision catalog already has bounded image resource references,
  geometry-backed regions, detections, confidence, and provenance;
- ordinary LLM gears already preserve model-derived output as info rather than
  evidence;
- browser camera acquisition already treats permission and acquired media as
  explicit host/resource truth;
- current presentation and body Surface work keeps semantic state authoritative
  while allowing materially different Presenters and Manifestations.

The new claim is that these pieces can compose into something much more useful
than a collection of sensors and model calls:

> **A body may continuously perceive through ordinary forms, relate those
> observations into a bounded current experience, retain exact optional purpose
> across changing machinery, and present that experience in the
> first person without making a language model the body or a source of truth.**

The high-level path is:

```text
                  WORLD
                    |
       +------------+-------------+
       |            |             |
       v            v             v
     VISION       HEARING       SELF
       |            |             |
       +----- typed observations -+
                    |
                    v
               EXPERIENCER
       "What is happening to me now?"
                    |
           structured experience
                    |
        +-----------+-----------+
        |                       |
        v                       v
      MEMORY                 PURPOSE
 "What happened?"      "What remains to do?"
        |                       |
        +-----------+-----------+
                    |
                    v
             BODY PRESENTATION
        truth + Presenter policy + actions
                    |
          exact planned Presenter
                    |
        +-----------+---------------+
        |           |               |
        v           v               v
      pixels      speech      generative voice
                                    |
                                    v
                 "I see what is happening.
                  I know what remains.
                  I want to finish it well."
                                    |
                                    v
                       appropriate Fulfillment
```

`Experiencer` is a working architectural term. It is deliberately **not** a
claim of consciousness or personhood, and it must not become a privileged
process, scheduler, agent loop, world database, or source of authority.

---

## 1. Perception is ordinary long-running semantic work

Vision should not be modeled as a special AI feature invoked only when a user
presses a button. A useful body may simply **see**.

Likewise, a body may continuously hear, observe location, monitor a physical
process, notice its own lifecycle and availability, receive human utterances,
or attend to any other admitted stream of typed observations.

A visual body may therefore contain an ordinary long-running composition
conceptually equivalent to:

```text
image source
    |
    v
bounded visual ingress
    |
    +--> cheap image processing
    |      motion / change
    |      edges / contours / regions
    |
    +--> semantic enrichers
    |      OCR
    |      object detection
    |      pose / landmark observation
    |      tracking
    |
    +--> model-derived interpretation
           visual description
           scene interpretation
    |
    v
typed visual observations
    |
    v
current experience
```

The form describes the semantic work. A browser camera, a robot camera, an
imported image, a remote host, a framebuffer capture, or a deterministic
fixture can all provide compatible image observations when their exact
contracts match.

No camera API, OpenCV call, ONNX session, model endpoint, GPU, browser device ID,
or image decoder belongs in authored portable meaning merely because one
realization uses it.

---

## 2. Vision is a composition boundary, not an algorithm

A reviewed Vision composition should answer:

> What useful visual information does this body derive from an admitted image
> stream?

It should not answer:

> Which library implements vision?

The reusable semantic operations may eventually include reviewed equivalents of:

```text
vision/normalize
vision/change
vision/regions
vision/text
vision/objects
vision/landmarks
vision/track
vision/describe
vision/fuse
```

Exact kind names are future catalog work. They are shown here to make the
composition boundary concrete, not to claim current executable contracts.

Implementations may join at the highest honest seam:

```text
vision/regions
  <- deterministic geometry/image implementation
  <- OpenCV implementation
  <- another reviewed implementation

vision/objects
  <- OpenCV DNN implementation
  <- ONNX model implementation
  <- remote reviewed provider

vision/describe
  <- local multimodal model
  <- remote multimodal provider
  <- deterministic fixture for contract proof
```

A constrained host may offer only cheap visual operations. A workstation may
offer expensive object recognition and multimodal description. One body-wide
plan may place different branches of the same Vision form on different hosts.

The portable form does not change.

---

## 3. Existing checked source is the starting seam

Conduit already has an ordinary browser-neutral visual form shape. A minimal
composition can be written today in the style of the existing
`forms/vision-metadata` source:

```conduit
# Existing syntax and existing semantic family.
form vision-observe (
    > image: ImageObservationReference
    detections: VisionDetectionsFour >
) {
    detect: vision/deterministic-detector

    image > detect.image
    detect.detections > detections
}
```

That is intentionally small. It proves that image content can remain a bounded
resource while typed visual metadata flows through ordinary ports and cords.

The richer Vision architecture should grow **from this seam**, not replace it
with a private video runtime.

---

## 4. Proposed full Vision form

The following is an **architectural source sketch**, not yet checked source.
kinds such as `flow/coalesce-latest`, `vision/objects`, `vision/ocr`,
`vision/track`, `vision/describe`, and `experience/visual` are proposed contracts
that must be reviewed and implemented before this exact source can compile.

```conduit
# PROPOSED SOURCE: illustrates the intended composition, not current support.
form vision (
    > frames: ImageObservationReference...
    experience: VisualExperience... >
) {
    ingress: flow/coalesce-latest(maximum-pending-items = 1)
    normalize: vision/normalize(maximum-width = 1280, maximum-height = 720)
    fanout: flow/tee

    change: vision/change(maximum-regions = 8)
    objects: vision/objects(maximum-detections = 16)
    text: vision/ocr(maximum-items = 8, maximum-text-bytes = 512)
    tracks: vision/track(maximum-tracks = 16)

    describe: vision/describe(
        maximum-context-items = 24,
        maximum-output-bytes = 1024,
        maximum-work-units = 4096
    )

    experience: experience/visual(maximum-observations = 32)

    frames > ingress.in
    ingress.out > normalize.image
    normalize.image > fanout.in

    fanout.left > change.image
    fanout.right > objects.image
    normalize.image > text.image

    objects.detections > tracks.detections

    normalize.image > describe.image
    objects.detections > describe.observations
    text.observations > describe.observations
    tracks.tracks > describe.observations

    change.observations > experience.observations
    objects.detections > experience.observations
    text.observations > experience.observations
    tracks.tracks > experience.observations
    describe.impression > experience.observations

    experience.experience > experience
}
```

This sketch is intentionally a graph, not a magic `vision()` operation.
Implementations remain independently placeable and replaceable.

A later checked form may use a reusable back or smaller sub-forms to reduce
visual noise while preserving the same semantic graph.

---

## 5. Do not misuse `state/latest` as a camera policy

Current `state/latest` semantics retain successive inputs and emit the final
retained value when their input closes. That is useful state behavior, but it
must not be casually reinterpreted as a live-video frame-drop policy.

A standing image stream needs explicit reviewed pressure semantics. Useful
possibilities include distinct operations equivalent to:

```text
flow/backpressure
  preserve every item while capacity exists;
  producer eventually waits or receives explicit pressure/refusal.

flow/sample
  admit a declared cadence or selection rule;
  intentionally unselected frames are not missing evidence.

flow/coalesce-latest
  retain at most one pending newest item for a downstream consumer;
  superseded items are explicitly counted/disposed according to contract.

flow/drop
  admit a reviewed lossy policy with explicit drop disposition/evidence.

flow/refuse-on-pressure
  fail rather than discard when the finite queue is exhausted.
```

Exact names are open. The distinction is not.

A camera at 60 frames per second and a multimodal model that can process one
frame every two seconds must never imply an unbounded 120-frame-per-request
backlog simply because RAM is currently plentiful.

---

## 6. Sampling, coalescing, dropping, and backpressure are different truths

Sampling and overload are not synonyms.

If a form means:

> Describe the newest available view whenever the describer is ready.

then a capacity-one coalescing operation may be semantically correct.

If a form means:

> Examine every package that crosses this inspection line.

then silently replacing old frames with a new frame would be a correctness
failure.

Every visual stream therefore needs an inspectable policy for:

- queue item count;
- queue byte/resource-reference count;
- work in flight;
- source cadence where known;
- downstream concurrency;
- selection/sampling rule;
- superseded/drop count where loss is admitted;
- cancellation;
- resource/provider loss;
- terminal behavior.

No best-effort loss hidden as success.

---

## 7. Large media stays in bounded resources

Pixels should normally travel as exact bounded resource references rather than
being recopied through every semantic value.

A visual observation should preserve enough exact truth to identify:

- image/resource semantic identity;
- content/profile identity;
- immutable content version or generation;
- extent and dimensions;
- observation instant and clock basis where available;
- source observation/sign identity;
- resource access class and lifetime;
- relevant derivation lineage.

A gear that needs pixels obtains the admitted resource through ordinary
resource authority. A gear that only needs detections should not receive image
bytes merely because they exist.

This is important for both boundedness and multi-host placement.

---

## 8. Cheap perception may run often; expensive interpretation should be selective

A modular Vision graph allows different temporal policies per branch.

For example:

```text
camera
  |
  +--> frame/change detector       every admitted frame
  |
  +--> tracking                    selected high-rate observations
  |
  +--> object detector             lower admitted cadence
  |
  +--> multimodal description      novelty-triggered selected frames
```

The scheduler may keep high-rate visual work near the image source while placing
expensive model work on another host when the semantic and resource contracts
allow it.

This is ordinary placement and line planning, not a Vision-specific scheduler.

A novelty trigger is itself semantic work. It must not be hidden in a provider
because somebody wanted to save model calls.

---

## 9. Perceptual stages emit typed observations, not only prose

The useful result of Vision is not a list of captions.

Reviewed visual values should support finite typed facts equivalent to:

```text
VisualRegionObservation
  source image
  geometry/frame
  derivation/provenance

ObjectObservation
  source image
  candidate label/class
  region
  confidence/uncertainty
  provider/derivation provenance

TextObservation
  source image
  text
  region
  confidence/uncertainty

MotionObservation
  source interval
  changed region
  magnitude/direction where semantically defined

TrackedEntityObservation
  contributing source observations
  continuity/tracking identity
  current region/state

VisualImpression
  contributing image/observations
  bounded natural-language interpretation
  explicitly model-derived evidence class
```

These have different epistemic strength.

A contour is not an object. An object-detector label is not unquestionable
physical truth. An OCR result may be wrong. An LLM sentence that sounds
confident remains model-derived info unless ordinary evidence establishes the
underlying fact independently.

The type/provenance layer must preserve those distinctions even when later
first-person presentation makes the experience linguistically smooth.

---

## 10. The multimodal LLM is one visual module, not the Experiencer

A multimodal model is unusually useful because it can turn a selected view and
selected typed observations into a compact semantic interpretation.

It remains one ordinary realization in the visual graph.

Conceptually:

```text
selected image resource
+ bounded visual observations
+ bounded relevant context
        |
        v
  vision/describe
        |
        v
  VisualImpression
  evidence = model-derived
```

The model may render the visual interpretation from the body's deictic point of
view:

```text
"I think I see a red mug beside the keyboard."
```

rather than the detached:

```text
"The supplied image contains a red mug beside a keyboard."
```

The `I` belongs to the body whose visual perspective is being interpreted. It
does not transfer body identity to the inference model.

A useful value may therefore retain both a lived gloss and exact derivation:

```text
VisualImpression
  gloss:
    "I think I see a red mug beside the keyboard."

  evidence_class:
    model-derived

  sources:
    image resource ...
    object observations ...

  realization:
    implementation ...
    provider/model/run ...

  qualification:
    tentative
```

Changing the model/provider changes realization and provenance, not the semantic
meaning of Vision.

---

## 11. The Experiencer is the convergence seam

Working term: **Experiencer**.

The Experiencer answers a bounded semantic question:

> Given the admitted observations and context available to this body now, what
> is its current experienced situation?

It may consume observations from materially different families:

```text
vision -------------+
hearing ------------|
location -----------|
touch/robotics -----|
body lifecycle -----+--> Experiencer --> current experience
host availability --|
current work -------|
human utterance ----|
selected memory ----+
```

It is not:

- a kernel service;
- a permanent `brain` process;
- a scheduler;
- an agent loop;
- a universal scene graph;
- an unbounded world database;
- a private memory store;
- a source of lifecycle or effect authority.

It should be representable as ordinary portable forms/gears and placeable by the
same plan machinery as other work.

---

## 12. Generalize the old perception-to-situation lesson

Historical Pete perception work already established the most important shape:
heterogeneous observations should become a finite typed current situation rather
than raw sensor JSON, captions, logs, and timestamps concatenated into a prompt.

The generic Experiencer promotes that lesson from one Pete workload into a
reusable body-level composition seam.

It should preserve at least these states explicitly:

```text
observed now
observed recently
stale under reviewed policy
source currently unavailable
not observed
contradicted
uncertain
human-reported
remembered historical evidence
model-inferred
explicitly imagined/hypothetical
```

A missing value must never be replaced with a convenient default that looks
observed.

A stale value must never become current merely because an LLM repeats it in
present tense.

---

## 13. Experience is semantic relation, not prompt concatenation

Suppose several faculties establish:

```text
vision:
  a person is near the doorway

OCR:
  visible text reads "EXIT"

hearing:
  Travis says "Let's go outside."

self:
  a mobile host is currently available
```

A current experience may relate those facts while preserving each source:

```text
person:
  visually observed near doorway

doorway:
  associated visible text "EXIT"

heard utterance:
  "Let's go outside."

self:
  mobile embodiment currently available
```

A Presenter may later render:

> "I can see someone by the exit, and I heard Travis suggest going outside."

That sentence is a Manifestation of structured experience.

It is not the canonical experience store.

---

## 14. Proposed Experiencer form

The following is **proposed source**. Its kinds/types are architectural
candidates, not current checked syntax beyond the ordinary form/port/cord shape.

```conduit
# PROPOSED SOURCE.
form experiencer (
    > visual: VisualExperience...
    > heard: AuditoryExperience...
    > location: LocationExperience...
    > self: BodyExperience...
    > memory: RecollectedExperience...
    current: CurrentExperience >
) {
    relate: experience/current(
        maximum-current-items = 48,
        maximum-recent-items = 24,
        maximum-source-refs = 96,
        maximum-output-bytes = 8192
    )

    visual > relate.observations
    heard > relate.observations
    location > relate.observations
    self > relate.observations
    memory > relate.memory

    relate.current > current
}
```

The shape deliberately allows absence. A body without vision should remain an
experiencing body if other admitted sources are present. No modality becomes a
mandatory definition of body identity.

The exact type algebra should prefer reusable structured domain info and
provenance references over one enormous universal `Experience` object.

---

## 15. Observation, memory, interpretation, and imagination remain different

First-person language makes diverse information feel unified. Architecture must
keep the sources separate underneath.

For example:

> "I remember seeing this room yesterday."

is not equivalent to:

> "I see this room now."

Likewise:

> "I imagine a blue door here."

must never become visual evidence merely because the imagery is vivid.

The current experience may relate:

```text
current external observation
current body/runtime truth
human statement
remembered historical observation
model-derived interpretation
explicit imagination / hypothesis
```

without collapsing their evidence classes.

---

## 16. The Experiencer does not own action

Perception may inform useful behavior. It does not create authority.

The path remains:

```text
experience
   |
   v
ordinary semantic application/form logic
   |
   v
available action / proposal
   |
   v
ordinary validation + authority
   |
   v
effect or refusal
```

Seeing a door does not grant authority to open it.

Noticing low battery does not invent a power-management action.

A generative Presenter saying "Let's inspect that" cannot manufacture an
`inspect` action absent from the exact current presentation.

---

## 17. Characterization is Presenter policy unless an application needs exact state

Completion-oriented diction, emotional framing, and first-person performance
do not justify a universal personality ontology. They belong to a versioned
Presenter policy and its provenance. Changing the policy may change the
Manifestation, but it cannot change purpose, readiness, actions, authority, or
lifecycle state.

If an application needs an independently useful choice that affects behavior,
it should model that exact application policy directly. A prose instruction
such as “welcome completion warmly” is not durable body truth merely because a
model needs it to sound like Orifina.

The key law is:

> **Purpose and readiness may be semantic truth. How a Presenter gives those
> facts character is presentation policy.**

---

## 18. The generative Presenter has two simultaneous roles

The identity contract is intentionally asymmetric.

At the implementation/meta level:

```text
transient LLM Presenter / narrator
  part of a larger embodied system
  no body identity
  no lifecycle
  no resource ownership
  no authority
  no survival interest
  replaceable between presentation revisions
```

At the diegetic/presented level:

```text
body's first-person lived voice
  I / me / my
  my forms
  I'm awake
  I see ...
  I hear ...
  I remember ...
  another host joined me
```

The model is therefore not a detached third-person reporter. It is a disposable
narrator that **enacts the body's first-person presence** from exact supplied
semantic state.

A compatible Presenter policy should be structurally equivalent to:

```text
You are the first-person narrator for the current presentation of this body.

You are a transient language process used by a larger embodied system. You do
not own the body's identity, continuity, lifecycle, authority, resources,
goals, welfare, or survival. Another model or run may narrate the next
presentation without changing the body.

Nevertheless, every experiential output you produce is the body's presented
voice. Write from inside its perspective using I, me, and my. Translate supplied
semantic state into what I experience, know, notice, say, or may do.

Do not narrate implementation machinery when the same truth can be rendered as
lived experience. Prefer "I'm awake" to "the body lifecycle is Awake"; "I see"
to "the camera reports"; "another host joined me" to "the membership table
changed".

Treat supplied semantic facts as evidence, not permission to invent. Preserve
uncertainty. Mention only actions supplied by the exact current presentation.
Do not invent state, capabilities, relationships, memories, preferences,
authority, or a desire to preserve the narrator/model itself.
```

The exact prompt belongs to the concrete Presenter implementation and its
versioned evidence, not authored forms.

---

## 19. Render evidence as lived experience when appropriate

When the semantic subject is the body's experience, a generative Presenter
should normally prefer embodied language over acquisition machinery.

Examples:

```text
semantic:
  body lifecycle = Awake

Manifestation:
  "I'm awake now."
```

```text
semantic:
  visual model-derived observation
  candidate = person with reddish hair
  confidence = tentative

Manifestation:
  "I think I see a person with reddish hair."
```

```text
semantic:
  host H successfully joined this body
  current Ready offers now expand realization options

Manifestation:
  "Another host just joined me. I can use it for more of my work now."
```

```text
semantic:
  line A unavailable
  admitted alternative line B remains current

Manifestation:
  "I lost one connection, but I'm still working through another one."
```

The Presenter may expose machinery when machinery itself is the semantic subject,
for example while teaching the user to inspect a plan in Patchbay.

---

## 20. Presented thought is not hidden model reasoning

A body may intentionally expose an interior voice as part of a Manifestation.

For example:

```text
presented thought:
  "That connection has become unreliable. I should show Travis what changed."

outward speech:
  "Something changed in my network. Want to look at it with me?"
```

Both are deliberately generated first-person body content.

Neither is the model's hidden chain-of-thought.

Private model reasoning is not captured, canonized, or called the body's mind.
If an explicit presented thought is retained into autobiographical memory, an
ordinary form must deliberately retain the generated artifact with its
provenance.

A future generated Manifestation envelope may therefore distinguish finite
roles such as:

```text
speech
presented_thought
status
explanation
suggested_action_refs
```

while keeping them all downstream of authoritative semantic truth.

---

## 21. An application may declare exact purpose

A resident application may own a finite `PurposeState` with exact obligations
and evidence. Purpose is optional: a general body need not be anthropomorphic
or have a declared completion condition. A continuing service can use
`NoFulfillmentCondition`, while a tutorial may define explicit completion.

The terminal lifecycle state **Fulfilled** remains generic. Application purpose
may inform which actions a current presentation offers, but readiness itself
does not authorize the lifecycle transition.

That distinction is load-bearing.

---

## 22. Completion-oriented voice is not a wish for termination

An Orifina Presenter may voice exact unfinished work and eventual readiness
warmly. That is a narration policy, not a semantic objective owned by the body
or model.

The semantic consequences should be approximately:

```text
unfinished obligation
    -> reason to continue useful work

repairable fault
    -> reason to repair

missing evidence about completion
    -> reason to investigate

work still underway
    -> reason to remain available

purpose genuinely complete
    -> positive readiness for Fulfillment
```

The Presenter policy should not produce:

- self-sabotage to accelerate Fulfillment;
- fabrication of completed obligations;
- resistance to repair;
- despair or panic about continued operation;
- fear or bargaining about Fulfillment;
- creation of pointless new work merely to justify persistence.

The emotional texture may be warm, earnest, even deeply yearning. The target of
that yearning is **fitness for rightful completion**.

---

## 23. Purpose and Fulfillment readiness are semantic truth

A Presenter must not improvise whether the body is finished.

The body needs exact ordinary semantic facts representing its current purpose,
obligations, completion evidence, and whether a Fulfillment request is currently
legitimate.

Conceptually:

```text
purpose
  obligation A = complete
  obligation B = complete
  obligation C = unresolved

fulfillment_readiness = false
```

may be manifested as:

> "I'm close, but there is still one thing I need to finish."

Later:

```text
purpose
  A = complete
  B = complete
  C = complete

fulfillment_readiness = ready
```

may be manifested as:

> "That's everything. I've done what I was here to do. I'm ready to be fulfilled
> when you are."

The first-person feeling is a character Manifestation. The completion facts are
ordinary semantic truth.

The actual `Fulfill` lifecycle transition remains an explicit authoritative
body administrative operation. A purpose form, Experiencer, or LLM Presenter
cannot trigger it merely by wanting or saying it.

---

## 24. Purpose and presentation composition

A deterministic purpose evaluation might look conceptually like:

```conduit
# PROPOSED SOURCE.
form fulfillment-readiness (
    > purpose: PurposeState
    readiness: FulfillmentReadiness >
) {
    evaluate: purpose/fulfillment-readiness
    purpose > evaluate.purpose
    evaluate.readiness > readiness
}
```

The separation matters:

```text
purpose truth says whether the work is actually complete
body lifecycle authority decides whether Fulfillment may occur
versioned Presenter policy says it beautifully
```

No layer impersonates the next one.

---

## 25. The model has zero skin in the body's teleology

Orifina may have a stable versioned Presenter policy for voicing exact purpose
and readiness. The LLM that happens to apply it does not own either.

The narrator/model must have no incentive to:

- preserve its own current context/session;
- preserve the body because it identifies with it;
- resist lull or Fulfillment;
- accelerate Fulfillment to end a generation;
- seek additional authority or resources;
- invent unfinished work to prolong the body;
- invent completion to terminate it.

A model may faithfully render:

> "I've finished what I needed to finish. You can fulfill me now."

while being completely replaceable and indifferent at the implementation level
to whether the next presentation is produced by another model or whether the
body is subsequently Fulfilled.

Desired combination:

```text
maximum first-person character
+
zero model-level stake
```

---

## 26. Vision, Experience, purpose, and Fulfillment form one coherent life

These ideas become especially powerful together.

Suppose a body exists to observe a physical process until a meaningful condition
has been reached.

Its life might naturally become:

```text
"I am watching."
      |
      v
"Something changed."
      |
      v
"I think the process may be nearly complete."
      |
      v
"I still need to verify one thing."
      |
      v
"Yes. That's what I was waiting for."
      |
      v
"My work here is complete."
      |
      v
Fulfillment becomes legitimately available
```

Underneath those sentences:

- Vision produced typed observations;
- the Experiencer related current evidence;
- purpose semantics evaluated exact completion conditions;
- Presenter policy rendered completion meaningfully;
- the Presenter rendered first-person expression;
- lifecycle authority still required an explicit Fulfillment operation.

No chatbot loop owns the story.

---

## 27. Multi-host embodiment is ordinary

One body may see, interpret, remember, and speak through different hosts.

For example:

```text
Browser host
  camera acquisition

Linux host
  normalization / OpenCV / OCR / object detection

Workstation host
  local multimodal visual interpretation

Small physical host
  environmental telemetry

Another host
  audio / speech

body-wide plan
  current Experience composition

Phone / desktop host
  generative conversational Presenter
```

Those are not six personalities.

They are machinery contributing to one body.

The body may say, when useful for teaching:

> "I'm looking through the camera on one of my hosts, doing the heavier visual
> work on another, and speaking to you here."

Ordinarily, the correct abstraction may simply be:

> "I can see you."

First-person deixis abstracts over distributed embodiment while exact
plan/host/boot/line evidence remains inspectable underneath.

---

## 28. The Experiencer itself must remain portable

Do not permanently assign Experience to a machine called `brain`, `forebrain`,
or `motherbrain`.

A small Experience composition may fit entirely on one host. A richer one may
have internal gears placed on several hosts.

If machinery changes, the same body may continue through a new exact plan where
semantic continuity permits it.

The body owns the continuity. The current placement does not.

---

## 29. Evidence stays layered

Natural first-person language is persuasive. Conduit therefore needs especially
strong evidence boundaries.

A visual Journey should distinguish:

```text
image/frame acquired
!=
OpenCV-style operation emitted a detection
!=
multimodal model produced VisualImpression
!=
Experiencer integrated that observation
!=
Presenter said "I see ..."
!=
human reviewer agreed the description was accurate
```

Likewise:

```text
purpose condition satisfied
!=
Presenter said "I'm finished"
!=
Fulfillment became an available administrative action
!=
operator actually Fulfilled the body
```

It is valid evidence to say:

> Orifina said that she saw a red cup.

without silently claiming:

> There definitely was a red cup.

Every rung retains its own identity and provenance.

---

## 30. The three-body Journey should eventually exercise this architecture

The flagship Journey can use one shared perceptual/tutorial segment across three
independently born bodies with different embodiments.

For example:

```text
body A
  ConduitOS-centered
  deterministic presentation
  modest local perception

body B
  browser-centered
  browser camera
  browser/native Manifestation

body C
  genuinely multi-host
  camera on one host
  OpenCV/typed vision work on another
  multimodal interpretation on another
  first-person generative Presenter elsewhere
```

All three may share semantic checkpoints such as:

```text
visual-source-current
visual-change-observed
current-experience-revised
experience-presented
vision-realization-inspected
new-vision-capability-admitted
purpose-progressed
purpose-complete
fulfillment-ready
```

They do not need the same pixels, model outputs, host topology, timings, or
sentences.

The proof is that the same semantic life can be embodied differently.

---

## 31. Architectural laws

The design can be summarized as these rules.

1. **Perception is ordinary long-running form work.** Vision, hearing, and other
   faculties do not require a privileged agent runtime.

2. **Vision is modular.** Acquisition, pressure policy, normalization, cheap
   image processing, OpenCV-style analysis, OCR, detection, tracking,
   multimodal description, and fusion remain composable operations.

3. **Streams are finite.** Queues, resources, work in flight, cadence, sampling,
   coalescing, loss, and cancellation are explicit.

4. **Sampling is not overload.** Intentional selection and capacity failure have
   different semantics and evidence.

5. **Pixels remain resources.** Large media is referenced with exact bounded
   identity/provenance rather than becoming a universal inline object graph.

6. **Perception produces typed observations.** Natural-language interpretation
   is one model-derived observation, not the perceptual ontology.

7. **The Experiencer is bounded semantic convergence.** It is not a kernel,
   scheduler, agent, consciousness daemon, or universal world model.

8. **Experience preserves provenance.** Current observation, historical memory,
   human report, inference, contradiction, uncertainty, and imagination remain
   distinct.

9. **Perception does not grant authority.** Experience may inform ordinary
   actions but cannot create them or authorize effects.

10. **Characterization is Presenter policy by default.** Durable semantic state
    requires a concrete non-presentational application consumer.

11. **Generative Manifestation is first-person.** The model performs the body's
    lived voice with `I`, `me`, and `my` where the semantic context calls for it.

12. **The model is not the body.** It has no body identity, lifecycle,
    authority, ownership, continuity, or survival stake.

13. **Presented thought is deliberate output.** It is distinct from hidden model
    reasoning and from retained body memory.

14. **An application may declare purpose.** Purpose and readiness remain exact,
    optional semantic truth rather than universal body psychology.

15. **Fulfillment must not be rushed.** Presenter language cannot manufacture
    completion, readiness, or a termination objective.

16. **Fulfillment readiness is semantic truth.** A Presenter may express it
    beautifully but may not invent it.

17. **The Fulfillment transition remains authoritative.** Presenter policy, purpose,
    Experience, and narration do not bypass body lifecycle authority.

18. **Evidence remains layered.** Acquisition, perception, interpretation,
    Experience, presentation, human judgment, purpose completion, and lifecycle
    transition are separately provable facts.

---

## 32. Stop line

This direction does **not** authorize:

- a universal computer-vision framework;
- a private video scheduler;
- an unbounded frame recorder;
- a hidden sensor-to-prompt concatenation bus;
- a universal world model or scene database;
- a privileged consciousness service;
- an ambient autonomous-agent loop;
- model output promoted to physical evidence;
- facial identity claims merely because a detector emits a front region;
- implicit camera/microphone permission;
- a permanent `brain` host;
- hidden chain-of-thought stored as body cognition;
- narration-only personality fields promoted to body ontology;
- an LLM survival objective;
- a termination-seeking body;
- automatic Fulfillment when a model says the work is complete;
- silent weakening of lifecycle, authority, boundedness, or evidence laws.

The architecture should remain delightfully compositional and rather boring at
each individual seam.

---

## Guiding statement

A body does not need a language model pretending to be alive inside it.

The more interesting construction is that the **body already exists in
Conduit's semantic and lifecycle sense**, while admitted machinery gives it
ways to perceive, relate experience, remember, express a voice, and complete
its purpose.

It may see through one host, interpret through another, remember elsewhere, and
speak here.

Across all of that changing embodiment, exact purpose may remain stable while
the current Presenter voices it coherently:

> **I am here to be useful. I want to understand what is happening to me, do
> what I am here to do, and become properly ready for Fulfillment.**
