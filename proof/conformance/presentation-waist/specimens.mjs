const action = (identity, target, name) => ({
  identity, intent: `encounter/${identity}`, target, name, availability: "available",
});

const specimen = (identity, pressure, subjects, relationships, composition, actions, realization, expected) => ({
  identity,
  pressure,
  ownership: {
    truth: `specimens/${identity}/authoritative-domain-state`,
    selection: `specimens/${identity}/current-relevance`,
    encounter: `specimens/${identity}/presentation`,
    realization,
    show: `specimens/${identity}/finite-occurrence`,
  },
  presentation: { subjects, relationships, composition, actions },
  expected,
});

const s = (identity, name, role = "semantic/item") => ({ identity, name, role });
const r = (source, kind, target) => ({ source, kind, target });
const c = (source, kind, target) => ({ source, kind, target });

/**
 * Bounded adversarial specimens. These are deliberately small semantic
 * counterexamples, not product simulations or proprietary-interface clones.
 */
export const specimens = [
  specimen("authored-argument", "argument order and meaning-changing progressive disclosure",
    [s("claim", "Cells require usable energy", "argument/claim"), s("evidence", "ATP evidence", "argument/evidence"), s("qualification", "Not all energy is immediately usable", "argument/qualification")],
    [r("evidence", "argument/supports", "claim"), r("qualification", "argument/qualifies", "claim")],
    [c("evidence", "Subordinate", "claim"), c("qualification", "RevealAfter", "evidence")],
    [action("continue", "qualification", "Continue")],
    { graphical: "pages and emphasis", spoken: "paced clauses", linear: "ordered semantic records" },
    { understands: ["claim", "evidence supports claim", "qualification follows evidence"], can: ["continue"] }),

  specimen("named-collections", "independent access, focus, and activation",
    [s("tools", "Tools", "collection"), s("editor", "Editor", "application"), s("mail", "Mail", "application")],
    [r("tools", "Contains", "editor"), r("tools", "Contains", "mail"), r("tools", "encounter/focuses", "editor")],
    [c("editor", "Group", "tools"), c("mail", "Group", "tools")],
    [action("activate-editor", "editor", "Open Editor"), action("activate-mail", "mail", "Open Mail")],
    { graphical: "collection with independently focusable items", spoken: "named choice prompt", linear: "collection and action records" },
    { understands: ["Tools contains Editor and Mail", "Editor is focused"], can: ["activate-editor", "activate-mail"] }),

  specimen("source-destination", "simultaneous source/destination context and directional action",
    [s("source", "Archive", "file/location/source"), s("destination", "Backup", "file/location/destination"), s("report", "report.txt", "file")],
    [r("source", "Contains", "report"), r("report", "file/copy-destination", "destination")],
    [c("source", "Juxtapose", "destination"), c("report", "Group", "source")],
    [action("copy-to-destination", "report", "Copy to Backup")],
    { graphical: "two contextual regions", spoken: "source and destination named before command", linear: "role-bearing relationship records" },
    { understands: ["Archive is source", "Backup is destination", "report.txt is selected"], can: ["copy-to-destination"] }),

  specimen("spreadsheet", "addressed values, formula dependency, and recalculation action",
    [s("sheet", "Budget", "table"), s("b2", "B2", "table/cell"), s("c2", "C2", "table/cell"), s("total", "Total", "table/cell")],
    [r("sheet", "Contains", "b2"), r("sheet", "Contains", "c2"), r("sheet", "Contains", "total"), r("b2", "calculation/input-to", "total"), r("c2", "calculation/input-to", "total")],
    [c("b2", "Group", "sheet"), c("c2", "Group", "sheet"), c("total", "Emphasize", "b2")],
    [action("recalculate", "sheet", "Recalculate")],
    { graphical: "grid", spoken: "addressed cell traversal", linear: "stable identity and dependency records" },
    { understands: ["B2 and C2 contribute to Total", "cell addresses are semantic identity"], can: ["recalculate"] }),

  specimen("timeline-editor", "parallel tracks, selected clip, and ordered media",
    [s("session", "Song", "media/session"), s("voice", "Voice", "media/track"), s("music", "Music", "media/track"), s("clip", "Verse take", "media/clip")],
    [r("session", "Contains", "voice"), r("session", "Contains", "music"), r("voice", "Contains", "clip"), r("clip", "media/precedes", "music")],
    [c("voice", "Juxtapose", "music"), c("clip", "Emphasize", "voice")],
    [action("mute-voice", "voice", "Mute Voice"), action("trim-clip", "clip", "Trim Verse take")],
    { graphical: "tracks on a timeline", spoken: "track summary then selected clip", linear: "containment and order records" },
    { understands: ["Voice and Music coexist", "Verse take is selected on Voice"], can: ["mute-voice", "trim-clip"] }),

  specimen("web-article", "document hierarchy, supporting aside, and link action",
    [s("article", "Migration handbook", "document/article"), s("section", "Before migration", "document/section"), s("aside", "Back up first", "document/aside"), s("reference", "Compatibility table", "document/reference")],
    [r("article", "Contains", "section"), r("section", "Contains", "aside"), r("section", "Describes", "reference")],
    [c("aside", "Subordinate", "section"), c("reference", "Associate", "section")],
    [action("open-reference", "reference", "Open compatibility table")],
    { graphical: "article flow with aside", spoken: "heading traversal and aside cue", linear: "document relations" },
    { understands: ["aside qualifies section", "reference belongs with section"], can: ["open-reference"] }),

  specimen("map", "places, route relation, current focus, and navigable destination",
    [s("map", "Downtown", "geography/map"), s("station", "Central station", "geography/place"), s("museum", "Museum", "geography/place"), s("route", "Walking route", "geography/route")],
    [r("map", "Contains", "station"), r("map", "Contains", "museum"), r("route", "route/from", "station"), r("route", "route/to", "museum")],
    [c("route", "Associate", "station"), c("route", "Associate", "museum")],
    [action("start-route", "route", "Start walking route")],
    { graphical: "spatial map", spoken: "turn sequence with named endpoints", linear: "route endpoint records" },
    { understands: ["route goes from station to museum"], can: ["start-route"] }),

  specimen("terminal", "ordered transcript, command/result distinction, and repeat action",
    [s("session", "Build shell", "terminal/session"), s("command", "cargo test", "terminal/command"), s("result", "Tests passed", "terminal/result")],
    [r("session", "Contains", "command"), r("session", "Contains", "result"), r("result", "terminal/result-of", "command")],
    [c("result", "RevealAfter", "command")],
    [action("repeat-command", "command", "Run cargo test again")],
    { graphical: "monospace transcript", spoken: "command and result announcements", linear: "ordered transcript records" },
    { understands: ["Tests passed is result of cargo test"], can: ["repeat-command"] }),

  specimen("screen-reader", "nonvisual landmarks, focus, state, and exact action",
    [s("document", "Account settings", "document"), s("security", "Security", "document/region"), s("mfa", "Multi-factor authentication enabled", "status")],
    [r("document", "Contains", "security"), r("security", "Contains", "mfa"), r("document", "encounter/focuses", "mfa")],
    [c("mfa", "Emphasize", "security")],
    [action("change-mfa", "mfa", "Change multi-factor authentication")],
    { graphical: "landmark and status surface", spoken: "landmark/focus announcement", linear: "landmark, focus, status records" },
    { understands: ["MFA is enabled", "MFA status is focused in Security"], can: ["change-mfa"] }),

  specimen("patchbay", "connected ports, exact direction, current observation, and disconnect action",
    [s("form", "Text transform", "form"), s("input", "Text input", "port/input"), s("output", "Uppercase output", "port/output"), s("cord", "Text flow", "cord")],
    [r("form", "Contains", "input"), r("form", "Contains", "output"), r("cord", "cord/from", "input"), r("cord", "cord/to", "output")],
    [c("input", "Juxtapose", "output"), c("cord", "Associate", "input")],
    [action("disconnect", "cord", "Disconnect text flow")],
    { graphical: "spatial jacks and cord", spoken: "directed connection statement", linear: "exact endpoint records" },
    { understands: ["Text flow connects input to output", "direction is input to output"], can: ["disconnect"] }),
];

const relationshipStatement = ({ source, kind, target }) => `${source} ${kind} ${target}`;
const compositionStatement = ({ source, kind, target }) => `${source} ${kind} ${target}`;

export function semanticAccount(specimen) {
  return {
    subjects: specimen.presentation.subjects.map(({ identity }) => identity),
    relationships: specimen.presentation.relationships.map(relationshipStatement),
    composition: specimen.presentation.composition.map(compositionStatement),
    actions: specimen.presentation.actions.map(({ identity }) => identity),
  };
}

// These projectors intentionally produce different Shows. Their shared output
// is only the independently inspectable semantic account.
export function graphicalProject(specimen) {
  const account = semanticAccount(specimen);
  return {
    medium: "graphical", technique: specimen.ownership.realization.graphical,
    nodes: account.subjects.map(identity => ({ semanticIdentity: identity })),
    edges: account.relationships.map(statement => ({ semanticStatement: statement })),
    rhetoric: account.composition.map(statement => ({ semanticStatement: statement })),
    controls: account.actions.map(intent => ({ semanticIntent: intent })),
  };
}

export function spokenProject(specimen) {
  const account = semanticAccount(specimen);
  return {
    medium: "spoken",
    technique: specimen.ownership.realization.spoken,
    utterances: [
      ...account.subjects.map(value => `subject:${value}`),
      ...account.relationships.map(value => `relationship:${value}`),
      ...account.composition.map(value => `composition:${value}`),
      ...account.actions.map(value => `action:${value}`),
    ],
  };
}

export function linearProject(specimen) {
  const account = semanticAccount(specimen);
  return {
    medium: "deterministic-linear",
    technique: specimen.ownership.realization.linear,
    records: [...account.subjects.map(value => `SUBJECT ${value}`), ...account.relationships.map(value => `RELATIONSHIP ${value}`), ...account.composition.map(value => `COMPOSITION ${value}`), ...account.actions.map(value => `ACTION ${value}`)],
  };
}

export function accountGraphical(show) {
  return {
    subjects: show.nodes.map(({ semanticIdentity }) => semanticIdentity),
    relationships: show.edges.map(({ semanticStatement }) => semanticStatement),
    composition: show.rhetoric.map(({ semanticStatement }) => semanticStatement),
    actions: show.controls.map(({ semanticIntent }) => semanticIntent),
  };
}

export function accountSpoken(show) {
  const values = kind => show.utterances.filter(value => value.startsWith(`${kind}:`)).map(value => value.slice(kind.length + 1));
  return { subjects: values("subject"), relationships: values("relationship"), composition: values("composition"), actions: values("action") };
}

export function accountLinear(show) {
  const values = kind => show.records.filter(value => value.startsWith(`${kind} `)).map(value => value.slice(kind.length + 1));
  return { subjects: values("SUBJECT"), relationships: values("RELATIONSHIP"), composition: values("COMPOSITION"), actions: values("ACTION") };
}
