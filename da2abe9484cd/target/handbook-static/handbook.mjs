import { enhanceSvgMasks } from "./svg-viewport.js";

const article = document.querySelector(".handbook-article");
if (article) await enhanceSvgMasks(article, "img");
