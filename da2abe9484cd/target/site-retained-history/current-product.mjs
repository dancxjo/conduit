const root = document.querySelector("#product-truth");

try {
  const response = await fetch("./product-truth.json", { cache: "no-store" });
  if (!response.ok) throw new Error(`status ${response.status}`);
  render(await response.json());
} catch (error) {
  root.setAttribute("aria-busy", "false");
  root.replaceChildren(paragraph(
    "Publication truth is unavailable here. The artifact is installed only by accepted-release publication.",
    "truth-error",
  ));
}

function render(truth) {
  if (truth?.schema !== "conduit.product-truth/v1") throw new Error("unknown product truth");
  const cards = document.createElement("div");
  cards.className = "truth-grid";
  cards.append(
    identityCard("Latest development", truth.development.commit, truth.development.integration_run_url),
    identityCard("Latest accepted release", truth.accepted_release.main_commit,
      `https://github.com/${truth.repository}/pull/${truth.accepted_release.pull_request}`,
      `source ${short(truth.accepted_release.source_commit)}`),
    identityCard("Published Pages", truth.publication.release_main_commit,
      truth.publication.deployment_url, truth.publication.pages_url),
  );
  const lag = document.createElement("p");
  lag.className = truth.lag.accepted_release_behind_development
    || truth.lag.publication_behind_accepted_release ? "truth-lag behind" : "truth-lag current";
  lag.textContent = lagText(truth.lag);
  const heading = document.createElement("h2");
  heading.textContent = "Latest proof receipts";
  const list = document.createElement("ul");
  list.className = "truth-evidence";
  for (const evidence of truth.evidence) {
    const item = document.createElement("li");
    const link = document.createElement("a");
    link.href = evidence.receipt_url;
    link.textContent = evidence.surface;
    item.append(link, document.createTextNode(
      ` — ${evidence.proof_class} at ${short(evidence.commit)}`,
    ));
    list.append(item);
  }
  root.replaceChildren(lag, cards, heading, list);
  root.setAttribute("aria-busy", "false");
}

function identityCard(title, commit, href, detail = "") {
  const article = document.createElement("article");
  article.className = "truth-card";
  const heading = document.createElement("h2");
  heading.textContent = title;
  const link = document.createElement("a");
  link.href = href;
  link.textContent = short(commit);
  const copy = paragraph(detail);
  article.append(heading, link, copy);
  return article;
}

function lagText(lag) {
  if (!lag.accepted_release_behind_development && !lag.publication_behind_accepted_release) {
    return "Published Pages, the accepted release, and development name the same source.";
  }
  const gaps = [];
  if (lag.accepted_release_behind_development) gaps.push("the accepted release is behind development");
  if (lag.publication_behind_accepted_release) gaps.push("published Pages are behind the accepted release");
  return `Lag is explicit: ${gaps.join("; ")}.`;
}

function short(commit) {
  return typeof commit === "string" ? commit.slice(0, 12) : "invalid identity";
}

function paragraph(text, className = "") {
  const node = document.createElement("p");
  node.className = className;
  node.textContent = text;
  return node;
}
