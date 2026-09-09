import { test } from "@playwright/test";
import { registerPatternComparisonTests } from "./browser-pattern-comparison.cases.mjs";
import { startTour } from "./tour-test-server.mjs";

let entrance;

test.beforeEach(async () => { entrance = await startTour(); });
test.afterEach(() => entrance?.child.kill());

registerPatternComparisonTests(() => entrance);
