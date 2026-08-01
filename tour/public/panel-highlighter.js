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

export function highlightPanelSource(
  source,
  selectedRange = null,
  metadata = null,
  diagnosticRanges = [],
) {
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
  const relatedRanges = Array.isArray(selectedRange?.related)
    ? selectedRange.related.filter((range) =>
      Number.isInteger(range?.start) &&
      Number.isInteger(range?.end) &&
      range.start < range.end
    )
    : [];
  const activeDiagnosticRanges = Array.isArray(diagnosticRanges)
    ? diagnosticRanges.filter((range) =>
      Number.isInteger(range?.start) &&
      Number.isInteger(range?.end) &&
      range.start < range.end
    )
    : [];
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
  let openLayers = [];
  const layerMarkup = {
    selection: '<mark class="panel-source-selection">',
    diagnostic: '<mark class="panel-source-diagnostic">',
    related: '<span class="panel-source-endpoint">',
  };
  const closeMarkup = {
    selection: "</mark>",
    diagnostic: "</mark>",
    related: "</span>",
  };

  const transitionLayers = (nextLayers) => {
    let shared = 0;
    while (shared < openLayers.length && shared < nextLayers.length &&
        openLayers[shared] === nextLayers[shared]) {
      shared += 1;
    }
    for (let index = openLayers.length - 1; index >= shared; index -= 1) {
      output += closeMarkup[openLayers[index]];
    }
    for (let index = shared; index < nextLayers.length; index += 1) {
      output += layerMarkup[nextLayers[index]];
    }
    openLayers = nextLayers;
  };

  for (const { text, tokenStart, kind, attributes } of runs) {
    const tokenEnd = tokenStart + text.length;
    const boundaries = [tokenStart, tokenEnd];
    if (hasSelection && selectionStart > tokenStart && selectionStart < tokenEnd) {
      boundaries.push(selectionStart);
    }
    if (hasSelection && selectionEnd > tokenStart && selectionEnd < tokenEnd) {
      boundaries.push(selectionEnd);
    }
    for (const range of relatedRanges) {
      if (range.start > tokenStart && range.start < tokenEnd) {
        boundaries.push(range.start);
      }
      if (range.end > tokenStart && range.end < tokenEnd) {
        boundaries.push(range.end);
      }
    }
    for (const range of activeDiagnosticRanges) {
      if (range.start > tokenStart && range.start < tokenEnd) {
        boundaries.push(range.start);
      }
      if (range.end > tokenStart && range.end < tokenEnd) {
        boundaries.push(range.end);
      }
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
    for (let index = boundaries.length - 1; index > 0; index -= 1) {
      if (boundaries[index] === boundaries[index - 1]) {
        boundaries.splice(index, 1);
      }
    }

    for (let index = 0; index < boundaries.length - 1; index += 1) {
      const fragmentStart = boundaries[index];
      const fragmentEnd = boundaries[index + 1];
      const selected = hasSelection &&
        fragmentStart >= selectionStart &&
        fragmentEnd <= selectionEnd;
      const related = relatedRanges.some((range) =>
        fragmentStart >= range.start && fragmentEnd <= range.end
      );
      const diagnostic = activeDiagnosticRanges.some((range) =>
        fragmentStart >= range.start && fragmentEnd <= range.end
      );
      transitionLayers([
        ...(selected ? ["selection"] : []),
        ...(diagnostic ? ["diagnostic"] : []),
        ...(related ? ["related"] : []),
      ]);

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
      const rendered = renderedKind
        ? `<span class="panel-token-${renderedKind}"${renderedAttributes}>${escaped}</span>`
        : escaped;
      output += rendered;
    }
  }
  transitionLayers([]);

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
      textarea.sourceDiagnosticRanges,
    );
    highlight.dataset.semanticMetadata =
      metadata?.semantic_available === true ? "available" : "unavailable";
    syncScroll();
  };

  textarea.dataset.highlighting = "panel";
  textarea.syncHighlight = sync;
  textarea.setSourceHighlightRange = (start, end) => {
    textarea.sourceHighlightRange = Number.isInteger(start) && Number.isInteger(end)
      ? { start, end, related: textarea.sourceHighlightRange?.related || [] }
      : null;
    sync();
  };
  textarea.setSourceRelatedRanges = (ranges) => {
    const related = Array.isArray(ranges)
      ? ranges.filter(Boolean).map((range) => ({
        start: range.start_utf16,
        end: range.end_utf16,
      }))
      : [];
    textarea.sourceHighlightRange = textarea.sourceHighlightRange
      ? { ...textarea.sourceHighlightRange, related }
      : { related };
    sync();
  };
  textarea.setSourceDiagnosticRanges = (ranges) => {
    textarea.sourceDiagnosticRanges = Array.isArray(ranges)
      ? ranges.filter(Boolean).map((range) => ({
        start: range.start_utf16,
        end: range.end_utf16,
      }))
      : [];
    sync();
  };
  textarea.addEventListener("input", () => {
    // The old projection belongs to the previous source revision. Remove it
    // synchronously; the editor controller supplies freshly compiled ranges.
    textarea.sourceHighlightRange = null;
    textarea.sourceDiagnosticRanges = [];
    sync();
  });
  textarea.addEventListener("scroll", syncScroll);
  sync();
  return sync;
}
