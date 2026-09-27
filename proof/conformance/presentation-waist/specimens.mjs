import { readFileSync } from "node:fs";

/** Shared bounded fixtures consumed by both Node and Rust conformance. */
export const specimens = JSON.parse(
  readFileSync(new URL("./specimens.json", import.meta.url), "utf8"),
);

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
