import { readFile, writeFile } from "node:fs/promises";
import { browserDestinationHref } from "../host/assets/application-presentation.mjs";
import { productMastheadDescription } from "../../../semantics/presentation/assets/product-masthead.mjs";

const [input, output, current, status = ""] = process.argv.slice(2);
if (!input || !output) throw new TypeError("usage: render-product-masthead.mjs INPUT OUTPUT CURRENT [STATUS]");

const escape = (value) => String(value)
  .replaceAll("&", "&amp;")
  .replaceAll('"', "&quot;")
  .replaceAll("<", "&lt;")
  .replaceAll(">", "&gt;");
const description = productMastheadDescription({ revision: 1, current, status });
const navigation = description.nodes.find((node) => node.component === "navigation");
const links = description.nodes.filter((node) => node.parent === description.nodes.indexOf(navigation));
const statusNode = description.nodes.find((node) => node.key === "product-status");
const markup = `<header data-application-key="product-masthead" data-application-component="masthead"><nav data-application-key="product-navigation" data-application-component="navigation" aria-label="${escape(navigation.text)}">${links.map((link) => `<a data-application-key="${escape(link.key)}" data-application-component="navigation-link" href="${escape(browserDestinationHref(link.value))}"${link.value === "home" ? ' aria-label="Conduit home"' : ""}${link.key === navigation.value ? ' aria-current="page"' : ""}>${escape(link.text)}</a>`).join("")}</nav><output id="host-state" data-application-key="product-status" data-application-component="status" role="status" aria-live="polite">${escape(statusNode.text)}</output></header>`;
const source = await readFile(input, "utf8");
const marker = "<!-- conduit-product-masthead -->";
if (source.split(marker).length !== 2) throw new Error("Pages source must contain one product masthead marker");
await writeFile(output, source.replace(marker, markup));
