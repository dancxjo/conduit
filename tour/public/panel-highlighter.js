let panelSyntaxWords = new Set();
let panelReservedWords = new Set();
let panelIdentifierCompatibleSyntaxWords = new Set();

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
  if (metadata?.schema !== "conduit.panel-language/v1" ||
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

export function highlightPanelSource(source, selectedRange = null) {
  const runs = [];
  let cursor = 0;
  let expectTypeName = false;
  let inImplementsList = false;
  let cordEndpoint = null;

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
    } else if ((match = rest.match(/^(?:->|[{}()[\],:=])/))) {
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

      if (kind === "keyword" && text === "cord") {
        cordEndpoint = "output";
      } else if (kind === "operator" && text === "->") {
        cordEndpoint = "input";
      }
    }

    const tokenStart = cursor;
    const endpointSelector = kind === "identifier" && cordEndpoint
      ? match[0].match(/^(.+)(\.(?:in|out))$/)
      : null;
    if (endpointSelector) {
      const [, node, selector] = endpointSelector;
      renderText(node, tokenStart, "identifier");
      const direction = selector === ".in" ? "input" : "output";
      renderText(
        selector,
        tokenStart + node.length,
        `port panel-token-port-${direction}`,
        ` data-token-label="${direction} port" aria-label="${direction} port"` +
          ` title="${direction[0].toUpperCase()}${direction.slice(1)} port"`,
      );
      cordEndpoint = null;
    } else {
      renderText(match[0], tokenStart, kind);
      if (kind === "identifier" && cordEndpoint) cordEndpoint = null;
    }
    cursor += match[0].length;
  }

  const selectionStart = selectedRange?.start;
  const selectionEnd = selectedRange?.end;
  const hasSelection = Number.isInteger(selectionStart) &&
    Number.isInteger(selectionEnd) &&
    selectionStart < selectionEnd;
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
      output += kind
        ? `<span class="panel-token-${kind}"${attributes}>${escaped}</span>`
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
    highlight.innerHTML = highlightPanelSource(
      textarea.value,
      textarea.sourceHighlightRange,
    );
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
