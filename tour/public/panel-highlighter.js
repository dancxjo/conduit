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

export function highlightPanelSource(source) {
  let output = "";
  let cursor = 0;

  while (cursor < source.length) {
    const rest = source.slice(cursor);
    let match;
    let kind = "";

    if ((match = rest.match(/^[ \t\r\n]+/))) {
      // Whitespace is deliberately unwrapped so source bytes remain obvious.
    } else if ((match = rest.match(/^#[^\n]*/))) {
      kind = "comment";
    } else if ((match = rest.match(/^"(?:\\[\s\S]|[^"\\])*"?/))) {
      kind = "string";
    } else if ((match = rest.match(/^-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?/))) {
      kind = "number";
    } else if ((match = rest.match(/^(?:->|[{}()[\],:=])/))) {
      kind = "operator";
    } else if ((match = rest.match(/^[A-Za-z_][A-Za-z0-9_-]*/))) {
      if (PANEL_KEYWORDS.has(match[0])) kind = "keyword";
      else if (PANEL_LITERALS.has(match[0])) kind = "literal";
      else kind = "identifier";
    } else {
      match = [rest[0]];
    }

    const text = escapeHtml(match[0]);
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
    highlight.innerHTML = highlightPanelSource(textarea.value);
    syncScroll();
  };

  textarea.dataset.highlighting = "panel";
  textarea.syncHighlight = sync;
  textarea.addEventListener("input", sync);
  textarea.addEventListener("scroll", syncScroll);
  sync();
  return sync;
}
