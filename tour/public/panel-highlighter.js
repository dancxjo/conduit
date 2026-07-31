let panelSyntaxWords = new Set();
let panelReservedWords = new Set();
let panelIdentifierCompatibleSyntaxWords = new Set();
let panelSourceMetadata = null;

const PANEL_LITERALS = new Set([
  "abort", "block", "coalesce", "drain", "drop-disposable", "fail",
  "fail_together", "false", "isolate", "queue_bounded", "ready", "reject",
  "restart_bounded", "sample", "true",
]);

function escapeHtml(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

export function configurePanelLanguage(metadata) {
  if (metadata?.schema !== "conduit.panel-language" ||
      !Array.isArray(metadata.syntax_words) ||
      !Array.isArray(metadata.reserved_words) ||
      !Array.isArray(metadata.identifier_compatible_syntax_words)) {
    throw new Error("invalid parser-owned panel language metadata");
  }
  panelSyntaxWords = new Set(metadata.syntax_words);
  panelReservedWords = new Set(metadata.reserved_words);
  panelIdentifierCompatibleSyntaxWords =
    new Set(metadata.identifier_compatible_syntax_words);
}

export function configurePanelSourceMetadata(resolver) {
  if (typeof resolver !== "function") {
    throw new Error("panel source metadata resolver must be a function");
  }
  panelSourceMetadata = resolver;
}

function semanticAttributes(annotation) {
  const direction = annotation.direction;
  const label = escapeHtml(annotation.accessible_label || `${direction} port`);
  const path = escapeHtml(annotation.semantic_path || "");
  return ` data-token-label="${direction} port" aria-label="${label}"` +
    ` title="${label}" data-semantic-path="${path}"`;
}

export function highlightPanelSource(source, selectedRange = null, metadata = null) {
  const runs = [];
  let cursor = 0;
  let expectTypeName = false;
  let inImplementsList = false;

  const renderText = (text, tokenStart, kind, attributes = "") => {
    runs.push({ text, tokenStart, kind, attributes });
  };

  while (cursor < source.length) {
    const rest = source.slice(cursor);
    let match;
    let kind = "";
    let isTrivia = false;

    if ((match = rest.match(/^[ \t\r\n]+/))) {
      // Whitespace is deliberately unwrapped so source bytes remain obvious.
      isTrivia = true;
    } else if ((match = rest.match(/^#[^\n]*/))) {
      kind = "comment";
      isTrivia = true;
    } else if ((match = rest.match(/^"(?:\\[\s\S]|[^"\\])*"?/))) {
      kind = "string";
    } else if ((match = rest.match(/^-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/))) {
      kind = "number";
    } else if ((match = rest.match(/^(?:->|<-|[><{}()[\],:=])/))) {
      kind = "operator";
    } else if ((match = rest.match(/^[-./@A-Za-z_][-./@A-Za-z0-9_[\]]*/))) {
      if (expectTypeName) kind = "type";
      else if (panelReservedWords.has(match[0])) kind = "keyword";
      else if (panelSyntaxWords.has(match[0]) &&
          !panelIdentifierCompatibleSyntaxWords.has(match[0])) kind = "keyword";
      else if (PANEL_LITERALS.has(match[0])) kind = "literal";
      else kind = "identifier";
    } else {
      match = [rest[0]];
    }

    if (!isTrivia) {
      const text = match[0];
      const wasExpectedType = expectTypeName && kind === "type";

      if (kind === "operator" && text === ":") {
        expectTypeName = true;
        inImplementsList = false;
      } else if (kind === "keyword" && text === "implements") {
        expectTypeName = true;
        inImplementsList = true;
      } else if (kind === "operator" && text === "," && inImplementsList) {
        expectTypeName = true;
      } else {
        expectTypeName = false;
        if (inImplementsList && !wasExpectedType) inImplementsList = false;
      }

    }

    const tokenStart = cursor;
    renderText(match[0], tokenStart, kind);
    cursor += match[0].length;
  }

  const selectionStart = selectedRange?.start;
  const selectionEnd = selectedRange?.end;
  const hasSelection = Number.isInteger(selectionStart) &&
    Number.isInteger(selectionEnd) &&
    selectionStart < selectionEnd;
  const semanticAnnotations = metadata?.semantic_available === true &&
      Array.isArray(metadata.annotations)
    ? metadata.annotations.filter((annotation) =>
      Number.isInteger(annotation.start_utf16) &&
      Number.isInteger(annotation.end_utf16) &&
      annotation.start_utf16 < annotation.end_utf16 &&
      ["receiving", "outgoing"].includes(annotation.direction) &&
      ["port-name", "port-sigil"].includes(annotation.kind)
    )
    : [];
  let output = "";
  let selectionOpen = false;

  for (const { text, tokenStart, kind, attributes } of runs) {
    const tokenEnd = tokenStart + text.length;
    const boundaries = [tokenStart, tokenEnd];
    if (hasSelection && selectionStart > tokenStart && selectionStart < tokenEnd) {
      boundaries.push(selectionStart);
    }
    if (hasSelection && selectionEnd > tokenStart && selectionEnd < tokenEnd) {
      boundaries.push(selectionEnd);
    }
    for (const annotation of semanticAnnotations) {
      if (annotation.start_utf16 > tokenStart &&
          annotation.start_utf16 < tokenEnd) {
        boundaries.push(annotation.start_utf16);
      }
      if (annotation.end_utf16 > tokenStart &&
          annotation.end_utf16 < tokenEnd) {
        boundaries.push(annotation.end_utf16);
      }
    }
    boundaries.sort((left, right) => left - right);

    for (let index = 0; index < boundaries.length - 1; index += 1) {
      const fragmentStart = boundaries[index];
      const fragmentEnd = boundaries[index + 1];
      const selected = hasSelection &&
        fragmentStart >= selectionStart &&
        fragmentEnd <= selectionEnd;

      if (selected && !selectionOpen) {
        output += '<mark class="panel-source-selection">';
        selectionOpen = true;
      } else if (!selected && selectionOpen) {
        output += "</mark>";
        selectionOpen = false;
      }

      const escaped = escapeHtml(
        text.slice(fragmentStart - tokenStart, fragmentEnd - tokenStart),
      );
      const annotation = semanticAnnotations.find((candidate) =>
        fragmentStart >= candidate.start_utf16 &&
        fragmentEnd <= candidate.end_utf16
      );
      const semanticKind = annotation?.kind === "port-sigil"
        ? `port-sigil panel-token-port-sigil-${annotation.direction}`
        : annotation
          ? `port panel-token-port-${annotation.direction}`
          : null;
      const renderedKind = semanticKind || kind;
      const renderedAttributes = annotation
        ? semanticAttributes(annotation)
        : attributes;
      output += renderedKind
        ? `<span class="panel-token-${renderedKind}"${renderedAttributes}>${escaped}</span>`
        : escaped;
    }
  }
  if (selectionOpen) output += "</mark>";

  // A final newline needs a visible line box to keep overlay scrolling aligned.
  return source.endsWith("\n") ? `${output} ` : output;
}

export function attachPanelSourceHighlighting(textarea) {
  if (!textarea || textarea.dataset.highlighting === "panel") {
    return textarea?.syncHighlight ?? (() => {});
  }

  const highlight = textarea.parentElement?.querySelector(".panel-source-highlight");
  if (!highlight) return () => {};

  const syncScroll = () => {
    highlight.scrollTop = textarea.scrollTop;
    highlight.scrollLeft = textarea.scrollLeft;
  };
  const sync = () => {
    let metadata = null;
    if (panelSourceMetadata) {
      try {
        metadata = JSON.parse(panelSourceMetadata(textarea.value));
      } catch {
        metadata = null;
      }
    }
    highlight.innerHTML = highlightPanelSource(
      textarea.value,
      textarea.sourceHighlightRange,
      metadata,
    );
    highlight.dataset.semanticMetadata =
      metadata?.semantic_available === true ? "available" : "unavailable";
    syncScroll();
  };

  textarea.dataset.highlighting = "panel";
  textarea.syncHighlight = sync;
  textarea.setSourceHighlightRange = (start, end) => {
    textarea.sourceHighlightRange = Number.isInteger(start) && Number.isInteger(end)
      ? { start, end }
      : null;
    sync();
  };
  textarea.addEventListener("input", sync);
  textarea.addEventListener("scroll", syncScroll);
  sync();
  return sync;
}
