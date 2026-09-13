import { createTourStage } from "./tour-state.mjs";

const CRECHE_MARKERS = Object.freeze(new Map([
  ["<!-- conduit-physical-host -->", "Add a physical Host in the Crèche"],
  ["<!-- conduit-first-host -->", "Admit a first Host in the Crèche"],
  ["<!-- conduit-graduation -->", "Open graduation in the Crèche"],
]));

function fence(lines, index) {
  const source = [];
  for (let cursor = index + 1; cursor < lines.length; cursor += 1) {
    if (lines[cursor] === "```") return { source: source.join("\n"), end: cursor };
    source.push(lines[cursor]);
  }
  throw new Error("Tour chapter contains an unterminated source fence");
}

/**
 * Admit authored chapter syntax into a finite, presentation-independent model.
 * The browser shell may arrange these blocks, but cannot reinterpret comments
 * or runnable fences as lifecycle truth.
 */
export function admitTourChapter(page) {
  const lines = page.markdown.replaceAll("\r\n", "\n").split("\n");
  const blocks = [];
  let paragraph = [];
  let declaredStageIndex = 0;
  const flush = () => {
    if (paragraph.length === 0) return;
    blocks.push(Object.freeze({ kind: "paragraph", text: paragraph.join(" ") }));
    paragraph = [];
  };
  const admitStage = (source, mode) => {
    const stage = createTourStage(source, mode);
    const declared = page.stages[declaredStageIndex++];
    if (!declared || declared.identity !== stage.identity || declared.mode !== stage.mode) {
      throw new Error("Tour runnable source does not match its admitted page stage");
    }
    blocks.push(Object.freeze({ kind: "stage", stage }));
  };

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (line === "```conduit birth") {
      flush();
      const captured = fence(lines, index);
      blocks.push(Object.freeze({ kind: "birth-example", source: captured.source }));
      index = captured.end;
    } else if (["```conduit run", "```conduit run recursive", "```conduit compare",
      "```conduit run two-host", "```conduit run two-host plan"].includes(line)) {
      flush();
      const captured = fence(lines, index);
      const mode = line === "```conduit run recursive" ? "recursive"
        : line === "```conduit compare" ? "compare"
          : line === "```conduit run two-host" ? "two-host"
            : line === "```conduit run two-host plan" ? "two-host-plan" : "run";
      admitStage(captured.source, mode);
      index = captured.end;
    } else if (line === "```text") {
      flush();
      const captured = fence(lines, index);
      blocks.push(Object.freeze({ kind: "diagram", text: captured.source }));
      index = captured.end;
    } else if (line === "<!-- conduit-host-inventory -->") {
      flush();
      blocks.push(Object.freeze({ kind: "inventory" }));
    } else if (CRECHE_MARKERS.has(line)) {
      flush();
      blocks.push(Object.freeze({ kind: "creche-handoff", label: CRECHE_MARKERS.get(line) }));
    } else if (line.startsWith("# ") || line.startsWith("## ")) {
      flush();
      const level = line.startsWith("## ") ? 2 : 1;
      blocks.push(Object.freeze({ kind: "heading", level, text: line.slice(level + 1) }));
    } else if (line.trim() === "") {
      flush();
    } else {
      paragraph.push(line.trim());
    }
  }
  flush();
  if (declaredStageIndex !== page.stages.length) {
    throw new Error("Tour page declares a stage with no runnable source");
  }
  return Object.freeze({
    schema: "conduit.tour/admitted-chapter@1",
    pageIdentity: page.identity,
    blocks: Object.freeze(blocks),
  });
}
