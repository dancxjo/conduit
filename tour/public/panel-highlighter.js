const PANEL_KEYWORDS = new Set([
  "admission", "admission_queue", "as", "bind", "capacity", "cleanup",
  "coalescer", "composite", "cord", "deadline_ms", "export", "fallback",
  "high_watermark", "idle_timeout_ms", "implements", "import", "indexed",
  "input", "interface", "keyed", "low_watermark", "max", "max_queued_bytes",
  "max_value_bytes", "maximum", "member", "node", "optional", "output",
  "panel", "pin", "pool", "pressure", "restart_attempts",
  "restart_backoff_ms", "root", "sample_every", "sample_offset",
  "supervision", "using",
]);

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

export function highlightPanelSource(source, selectedRange = null) {
  let output = "";
  let cursor = 0;
  let expectTypeName = false;
  let inImplementsList = false;

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
      else if (PANEL_KEYWORDS.has(match[0])) kind = "keyword";
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
    const tokenEnd = cursor + match[0].length;
    const selectionStart = Math.max(tokenStart, selectedRange?.start ?? tokenEnd);
    const selectionEnd = Math.min(tokenEnd, selectedRange?.end ?? tokenStart);
    const fragments = selectionStart < selectionEnd
      ? [
          [match[0].slice(0, selectionStart - tokenStart), false],
          [match[0].slice(selectionStart - tokenStart, selectionEnd - tokenStart), true],
          [match[0].slice(selectionEnd - tokenStart), false],
        ]
      : [[match[0], false]];
    const text = fragments
      .filter(([fragment]) => fragment.length > 0)
      .map(([fragment, selected]) => {
        const escaped = escapeHtml(fragment);
        return selected
          ? `<mark class="panel-source-selection">${escaped}</mark>`
          : escaped;
      })
      .join("");
    output += kind ? `<span class="panel-token-${kind}">${text}</span>` : text;
    cursor += match[0].length;
  }

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
