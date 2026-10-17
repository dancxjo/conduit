import { expect, test } from "@playwright/test";

test("portable face nucleus executes in WASM and manifests in Chromium", async ({ page }) => {
  const failures = [];
  page.on("pageerror", (error) => failures.push(error.stack ?? String(error)));
  page.on("console", (message) => {
    if (message.type() === "error") failures.push(message.text());
  });

  await page.goto("/proof/browser/face-nucleus.test.html");
  await expect(page.locator("#result")).toHaveText("ok");
  await expect(page.locator("#nucleus [data-layout-index]")).toHaveCount(3);
  await expect(page.locator("#nucleus [data-graphics-kind]")).toHaveCount(3);
  await expect(page.locator("#nucleus [data-graphics-kind=text]")).toHaveText("ready");
  await expect(page.locator("#nucleus [data-graphics-kind=text]")).toHaveAttribute("data-graphics-text-role", "body");
  await expect(page.locator("#nucleus [data-graphics-kind=icon]")).toHaveAttribute("role", "img");
  await expect(page.locator("#nucleus [data-face-kind=text]")).toHaveText("STRASSE");
  await expect(page.locator("#nucleus [data-application-component=shell]")).toHaveCount(1);
  await expect(page.locator("#nucleus")).toHaveAttribute("data-application-look", "conduit.look/phosphor@1");
  await expect(page.locator("#nucleus")).toHaveCSS("font-size", "16px");
  await expect(page.locator("#nucleus")).toHaveCSS("line-height", "24px");
  const themedButton = page.locator("#nucleus [data-application-component=button]");
  await expect(themedButton).toHaveCSS("border-radius", "6px");
  await themedButton.focus();
  await expect(themedButton).toHaveCSS("outline-width", "3px");
  await expect(page.locator("#nucleus [data-application-key=heading] > h3")).toHaveText("Browser Host face");
  await expect(page.locator("#nucleus [data-application-key=heading]")).toHaveAttribute("data-application-evidence", "succeeded");
  const structured = page.locator("#nucleus [data-face-kind=structured-info]");
  await expect(structured).toHaveAttribute("data-schema", "education/feedback@1");
  await expect(structured).toHaveAttribute("data-variant", "passed");
  await expect(structured).toHaveAttribute("data-quantity-unit", /^physical\/unit\/[a-f0-9]{64}$/);
  await expect(structured).toHaveAttribute("data-quantity", "88e0");
  await expect(structured).toHaveAttribute("data-quantity-unit-symbol", "%");
  expect(failures).toEqual([]);
  expect(await page.evaluate(() => globalThis.__conduitFaceNucleus)).toEqual({
    layoutChildren: 3,
    graphicsKinds: [1, 2, 3],
    text: "STRASSE",
    structured: {
      schema: "education/feedback@1",
      variant: "passed",
      quantityUnit: expect.stringMatching(/^physical\/unit\/[a-f0-9]{64}$/),
      quantityUnitSymbol: "%",
      quantity: "88e0",
    },
    application: {
      revision: 1,
      theme: "conduit.look/phosphor@1",
      heading: "Browser Host face",
      evidenceIdentity: "sign-nucleus-1",
      evidenceProvenance: "plan-nucleus-1/play-nucleus-1",
      artifactTag: "ARTICLE",
      rawLanguage: "json",
      rawText: '{"content":"<script>inert</script>"}',
      executableScripts: 0,
      action: "nucleus.continue",
      encodedEventBytes: 27,
      queuedEvents: 0,
      pressureRefusal: "queue-pressure",
      malformedRefusal: "unsupported-version",
      unchangedAfterRefusal: true,
      controlValue: "gear edited",
      controlLabel: "plot source",
      controlBytePressureRefusal: "queue-pressure",
      semantic: {
        panelTag: "SECTION",
        busy: "busy",
        busyRefusal: "action-busy",
        unavailable: "unavailable",
        unavailableRefusal: "unavailable-action",
        warning: "warning",
        failureRole: "alert",
        disclosureOpen: true,
        disclosureEvents: 0,
        staleRefusal: "stale-revision",
        retiredVersion: 9,
        retiredRefusal: "unsupported-version",
      },
    },
    missingContext: true,
  });
});

test("shared plots and navigation preserve exact keyboard interaction across revisions", async ({ page }) => {
  await page.goto("/proof/browser/face-nucleus.test.html");
  await expect(page.locator("#result")).toHaveText("ok");
  const field = page.locator('[data-application-key="source-field"]');
  const label = field.locator('[data-application-key="source-label"]');
  const control = field.locator('[data-application-key="source-control"]');
  const help = field.locator('[data-application-key="source-help"]');
  const error = field.locator('[data-application-key="source-error"]');
  await expect(label).toHaveAttribute("for", await control.getAttribute("id"));
  await expect(field.getByLabel("plot source")).toHaveCount(1);
  await expect(control).toHaveAttribute("aria-describedby", `${await help.getAttribute("id")} ${await error.getAttribute("id")}`);
  await expect(control).toHaveAttribute("aria-errormessage", await error.getAttribute("id"));
  await expect(control).toHaveAttribute("aria-invalid", "true");

  await control.focus();
  await control.evaluate((element) => element.setSelectionRange(2, 7, "forward"));
  await page.evaluate(() => globalThis.__conduitFocusProof.rerender());
  const revised = field.locator('[data-application-key="source-control"]');
  await expect(revised).toBeFocused();
  expect(await revised.evaluate((element) => ({ start: element.selectionStart, end: element.selectionEnd, direction: element.selectionDirection })))
    .toEqual({ start: 2, end: 7, direction: "forward" });
  await expect(revised).not.toHaveAttribute("aria-invalid", "true");
  await page.evaluate(() => globalThis.__conduitFocusProof.dispatchDetached());
  expect(await page.evaluate(() => globalThis.__conduitFocusProof.host.lastRefusal("focus"))).toBe("stale-revision");
  expect(await page.evaluate(() => globalThis.__conduitFocusProof.host.nextEvent("focus"))).toBeNull();

  const navigation = page.locator('[data-application-key="navigation"]');
  const previous = navigation.locator('[data-application-key="page-one"]');
  const next = navigation.locator('[data-application-key="page-two"]');
  await expect(next).toHaveAttribute("tabindex", "0");
  await expect(previous).toHaveAttribute("tabindex", "-1");
  await next.focus();
  await next.press("Home");
  await expect(previous).toBeFocused();
  await previous.press("End");
  await expect(next).toBeFocused();
  await next.press("ArrowLeft");
  await expect(previous).toBeFocused();
  await previous.press("ArrowDown");
  await expect(next).toBeFocused();

  const stepper = page.locator('[data-application-key="stepper"]');
  await expect(stepper).toHaveAttribute("data-application-current", "2");
  await expect(stepper.locator('[data-application-key="step-two"]')).toHaveAttribute("tabindex", "0");
  const progress = page.locator('progress[data-application-key="progress"]');
  await expect(progress).toHaveAttribute("value", "2");
  await expect(progress).toHaveAttribute("max", "3");
  await revised.focus();
  await page.evaluate(() => globalThis.__conduitFocusProof.removeFocused());
  await expect(page.locator('[data-application-key="page-two"]')).toBeFocused();
});

test("semantic product and same-site links retain exact bounded destinations without actions", async ({ page }) => {
  await page.goto("/proof/browser/face-nucleus.test.html");
  await page.evaluate(async () => {
    const { encodeApplicationView, manifestApplicationView } = await import(
      "/targets/browser/host/assets/application-face.mjs"
    );
    const root = document.createElement("div");
    root.id = "navigation-link-proof";
    document.body.append(root);
    manifestApplicationView(encodeApplicationView({
      revision: 1,
      actions: [],
      nodes: [
        { parent: null, component: "shell", key: "shell", text: "", action: null },
        { parent: 0, component: "navigation", key: "product-navigation", text: "Conduit products", value: "tour", valueCapacity: 16, action: null },
        { parent: 1, component: "navigation-link", key: "tour", text: "Tour", value: "tour", valueCapacity: 16, action: null },
      ],
    }), root);
    const handoff = document.createElement("div");
    handoff.id = "same-site-link-proof";
    document.body.append(handoff);
    manifestApplicationView(encodeApplicationView({
      revision: 1,
      actions: [],
      nodes: [{
        parent: null, component: "link", key: "handoff", text: "Add to new body",
        value: "/conduit/workspace/?plot=memory_lantern", valueCapacity: 2_048, action: null,
      }],
    }), handoff);
    let externalRefusal = "";
    try {
      encodeApplicationView({
        revision: 1,
        actions: [],
        nodes: [{
          parent: null, component: "link", key: "external", text: "External",
          value: "https://example.invalid/", valueCapacity: 2_048, action: null,
        }],
      });
    } catch (error) { externalRefusal = error.code; }
    globalThis.__sameSiteLinkRefusal = externalRefusal;
  });
  const link = page.locator('#navigation-link-proof [data-application-key="tour"]');
  await expect(link).toHaveText("Tour");
  await expect(link).toHaveAttribute("href", "/conduit/workspace/");
  await expect(link).toHaveAttribute("aria-current", "page");
  const handoff = page.locator('#same-site-link-proof [data-application-key="handoff"]');
  await expect(handoff).toHaveAttribute("href", /\/conduit\/workspace\/\?plot=memory_lantern$/u);
  expect(await handoff.evaluate((element) => element.onclick)).toBeNull();
  expect(await page.evaluate(() => globalThis.__sameSiteLinkRefusal)).toBe("invalid-control-value");
});

test("one finite theme mechanism preserves contrast and responsive layout across products", async ({ page }) => {
  await page.setViewportSize({ width: 680, height: 900 });
  await page.goto("/proof/browser/face-nucleus.test.html");
  await expect(page.locator("#result")).toHaveText("ok");
  for (const product of ["tour", "creche", "patchbay"]) {
    const root = page.locator(`[data-conduit-product="${product}"]`);
    await expect(root).toHaveAttribute("data-application-look", "conduit.look/phosphor@1");
    await expect(root.locator('[data-application-component="panel"]')).toHaveCSS("border-radius", "9px");
    await expect(root.locator('[data-application-component="panel"]')).toHaveCSS("border-color", "rgb(244, 196, 0)");
    await expect(root.locator('[data-application-component="grid"]')).toHaveCSS("grid-template-columns", /\d+px/);
    expect(await root.evaluate((element) => getComputedStyle(element).getPropertyValue("--conduit-structure-secondary").trim()))
      .toBe("#f4c400");
  }
});


test("generic no-Look encounter clears prior owned paint without changing meaning", async ({ page }) => {
  await page.goto("/proof/browser/face-nucleus.test.html");
  await expect(page.locator("#result")).toHaveText("ok");
  const result = await page.evaluate(async () => {
    const { encodeApplicationView, manifestApplicationView } = await import("/targets/browser/host/assets/application-face.mjs");
    const hex = await fetch("/proof/conformance/face-look/phosphor-look-v1.hex").then(response => response.text());
    const look = Uint8Array.from(hex.replace(/\s/gu, "").match(/../gu), byte => Number.parseInt(byte, 16));
    const root = document.createElement("section"); document.body.append(root);
    const view = encodeApplicationView({ revision: 1, actions: [], nodes: [
      { parent: null, component: "panel", action: null, key: "look-proof", text: "unchanged meaning" },
    ] });
    manifestApplicationView(view, root, { look });
    const before = root.textContent;
    root.style.setProperty("--renderer-private-sentinel", "kept");
    manifestApplicationView(view, root);
    return { before, after: root.textContent, look: root.dataset.applicationLook ?? null,
      background: root.style.getPropertyValue("--conduit-background"),
      sentinel: root.style.getPropertyValue("--renderer-private-sentinel") };
  });
  expect(result.before).toBe(result.after);
  expect(result).toMatchObject({ look: null, background: "", sentinel: "kept" });
});


test("current publication shell consumes canonical Look CSS hooks", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/site/index.html");
  await page.addStyleTag({ url: "/targets/browser/host/assets/conduit.css" });
  await expect(page.locator("body")).toHaveAttribute("data-application-look", "conduit.look/phosphor@1");
  await expect(page.locator("body")).not.toHaveAttribute("data-application-theme");
  await expect(page.locator("body")).toHaveCSS("background-color", "rgb(5, 7, 11)");
  await expect(page.locator("body")).toHaveCSS("font-size", "16px");
});

for (const sameRevision of [false, true]) {
  test(`replaced generic manifestation cannot create new events (same revision: ${sameRevision})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async sameRevision => {
      const { encodeApplicationView, manifestApplicationView } = await import('/targets/browser/host/assets/application-face.mjs');
      const root = document.createElement('section'); root.id = 'manifest-custody'; document.body.append(root);
      const description = { revision: 1, actions: [{ id: 'fixture.activate', event: 'activate' }],
        nodes: [{ parent: null, component: 'button', key: 'activate', text: 'Current action', action: 0 }] };
      globalThis.manifestCalls = [];
      const options = { onEvent: event => globalThis.manifestCalls.push(event.action) };
      const old = manifestApplicationView(encodeApplicationView(description), root, options);
      const obsolete = root.querySelector('button');
      const current = manifestApplicationView(encodeApplicationView({ ...description, revision: sameRevision ? 1 : 2 }), root, options);
      // Adversarial retained listener and explicit old-handle request.
      obsolete.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      old.requestAction('activate');
      globalThis.manifestProof = { old, current };
    }, sameRevision);
    expect(await page.evaluate(() => ({ calls: globalThis.manifestCalls,
      queued: globalThis.manifestProof.old.queuedEvents(), refusal: globalThis.manifestProof.old.lastRefusal() })))
      .toEqual({ calls: [], queued: 0, refusal: 'stale-revision' });
    await page.getByRole('button', { name: 'Current action', exact: true }).focus();
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => ({ calls: globalThis.manifestCalls,
      queued: globalThis.manifestProof.current.queuedEvents() }))).toEqual({ calls: ['fixture.activate'], queued: 1 });
  });
}

for (const input of ['keyboard', 'pointer']) {
  test(`nested generic action bindings emit only the nearest action (${input})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async () => {
      const { encodeApplicationView, manifestApplicationView } = await import('/targets/browser/host/assets/application-face.mjs');
      const root = document.createElement('section'); root.id = 'nested-action-proof'; document.body.append(root);
      globalThis.nestedActions = [];
      globalThis.nestedManifest = manifestApplicationView(encodeApplicationView({ revision: 1,
        actions: [{ id: 'fixture.panel', event: 'activate' }, { id: 'fixture.button', event: 'activate' }], nodes: [
          { parent: null, component: 'panel', key: 'parent', text: 'Panel action', action: 0 },
          { parent: 0, component: 'button', key: 'child', text: 'Run choice', action: 1 },
          { parent: 1, component: 'code', key: 'decoration', text: 'Decorative', action: null },
        ] }), root, { onEvent: event => globalThis.nestedActions.push(event.action) });
    });
    if (input === 'keyboard') {
      await page.getByRole('button', { name: 'Run choiceDecorative', exact: true }).focus();
      await page.keyboard.press('Enter');
    } else await page.locator('#nested-action-proof code').click();
    expect(await page.evaluate(() => ({ actions: globalThis.nestedActions, queued: globalThis.nestedManifest.queuedEvents() })))
      .toEqual({ actions: ['fixture.button'], queued: 1 });
  });
}

for (const unavailable of ['disabled', 'fieldset', 'inert', 'hidden', 'detached']) {
  test(`unavailable generic DOM controls refuse queueing (${unavailable})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async unavailable => {
      const { encodeApplicationView, manifestApplicationView } = await import('/targets/browser/host/assets/application-face.mjs');
      const outer = document.createElement('fieldset'); document.body.append(outer);
      const root = document.createElement('section'); outer.append(root);
      globalThis.availableCalls = [];
      const manifest = manifestApplicationView(encodeApplicationView({ revision: 1,
        actions: [{ id: 'fixture.available', event: 'activate' }], nodes: [
          { parent: null, component: 'button', key: 'action', text: 'Available action', action: 0 },
        ] }), root, { onEvent: event => globalThis.availableCalls.push(event.action) });
      const button = root.querySelector('button');
      if (unavailable === 'disabled') button.disabled = true;
      else if (unavailable === 'fieldset') outer.disabled = true;
      else if (unavailable === 'inert') outer.inert = true;
      else if (unavailable === 'hidden') outer.hidden = true;
      else root.remove();
      // Adversarial event; successful activation below uses native keyboard input.
      button.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      globalThis.availableProof = { manifest, restore() {
        button.disabled = false; outer.disabled = false; outer.inert = false; outer.hidden = false;
        if (!root.isConnected) outer.append(root);
      } };
    }, unavailable);
    expect(await page.evaluate(() => ({ calls: globalThis.availableCalls,
      queued: globalThis.availableProof.manifest.queuedEvents(), refusal: globalThis.availableProof.manifest.lastRefusal() })))
      .toEqual({ calls: [], queued: 0, refusal: 'unavailable-action' });
    await page.evaluate(() => globalThis.availableProof.restore());
    await page.getByRole('button', { name: 'Available action', exact: true }).focus();
    await page.keyboard.press('Enter');
    expect(await page.evaluate(() => globalThis.availableCalls)).toEqual(['fixture.available']);
  });
}

for (const forged of ['dom-option', 'string', 'bytes', 'malformed-bytes', 'bom-bytes']) {
  test(`generic select refuses undeclared values (${forged})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async forged => {
      const { encodeApplicationView, manifestApplicationView } = await import('/targets/browser/host/assets/application-face.mjs');
      const root = document.createElement('section'); document.body.append(root);
      globalThis.choiceCalls = [];
      const manifest = manifestApplicationView(encodeApplicationView({ revision: 1,
        actions: [{ id: 'fixture.choice', event: 'change' }], nodes: [
          { parent: null, component: 'select', key: 'choice', text: 'Exact choice', value: 'one', valueCapacity: 16, action: 0 },
          { parent: 0, component: 'option', key: 'one', text: 'One', value: 'one', valueCapacity: 16, action: null },
          { parent: 0, component: 'option', key: 'two', text: 'Unicode', value: 'é😀', valueCapacity: 16, action: null },
        ] }), root, { onEvent: event => globalThis.choiceCalls.push(new TextDecoder().decode(event.value)) });
      if (forged === 'dom-option') {
        const select = root.querySelector('select');
        const option = document.createElement('option'); option.value = 'forged'; option.textContent = 'Forged'; select.append(option);
        select.value = 'forged'; select.dispatchEvent(new Event('change', { bubbles: true }));
        option.remove(); select.value = 'one';
      } else manifest.requestAction('choice', forged === 'string' ? 'forged'
        : forged === 'bytes' ? new TextEncoder().encode('forged')
        : forged === 'bom-bytes' ? new TextEncoder().encode('\ufeffone') : Uint8Array.of(0xff));
      globalThis.choiceProof = manifest;
    }, forged);
    expect(await page.evaluate(() => ({ calls: globalThis.choiceCalls, queued: globalThis.choiceProof.queuedEvents(),
      refusal: globalThis.choiceProof.lastRefusal() }))).toEqual({ calls: [], queued: 0, refusal: 'invalid-control-value' });
    await page.getByRole('combobox', { name: 'Exact choice', exact: true }).focus();
    await page.keyboard.press('ArrowDown');
    expect(await page.evaluate(() => globalThis.choiceCalls)).toEqual(['é😀']);
  });
}

for (const mutation of ['input', 'callback', 'returned', 'transfer']) {
  test(`generic event queue retains private admitted bytes (${mutation})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const result = await page.evaluate(async mutation => {
      const { encodeApplicationView, encodeApplicationEvent, manifestApplicationView } = await import('/targets/browser/host/assets/application-face.mjs');
      const root = document.createElement('section'); document.body.append(root);
      const expected = encodeApplicationEvent({ revision: 1, action: 'fixture.custody', kind: 2, value: new TextEncoder().encode('yes') });
      const manifest = manifestApplicationView(encodeApplicationView({ revision: 1,
        actions: [{ id: 'fixture.custody', event: 'change' }], nodes: [
          { parent: null, component: 'select', key: 'choice', text: 'Private choice', value: 'yes', valueCapacity: 8, action: 0 },
          { parent: 0, component: 'option', key: 'yes', text: 'Yes', value: 'yes', valueCapacity: 8, action: null },
        ] }), root, { onEvent(event) {
          if (mutation === 'callback') { event.value.fill(0); event.encoded.fill(0); }
          if (mutation === 'transfer') structuredClone(event.encoded, { transfer: [event.encoded.buffer] });
        } });
      const input = new TextEncoder().encode('yes');
      const returned = manifest.requestAction('choice', input);
      if (mutation === 'input') input.fill(0);
      if (mutation === 'returned') { returned.value.fill(0); returned.encoded.fill(0); }
      const queued = manifest.nextEvent();
      return { value: new TextDecoder().decode(queued.value), exactEncoding: Array.from(queued.encoded).join() === Array.from(expected).join(),
        queued: manifest.queuedEvents(), queuedBytes: manifest.queuedEventBytes() };
    }, mutation);
    expect(result).toEqual({ value: 'yes', exactEncoding: true, queued: 0, queuedBytes: 0 });
  });
}

for (const component of ['text-input', 'textarea', 'independent-choice', 'exclusive-choice']) {
for (const invalid of ['string', 'bytes']) {
  test(`generic typed values refuse malformed input (${component}, ${invalid})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    const boolean = component.endsWith('choice');
    await page.evaluate(async ({ component, invalid, boolean }) => {
      const { encodeApplicationView, manifestApplicationView } = await import('/targets/browser/host/assets/application-face.mjs');
      const root = document.createElement('section'); document.body.append(root);
      const control = { parent: boolean ? 2 : null, component, key: 'control', text: 'Exact text',
        value: boolean ? 'false' : '', valueCapacity: 16, action: 0 };
      const nodes = boolean ? [
        { parent: null, component: 'choice-group', key: 'group', text: 'Choices', action: null },
        { parent: 0, component: 'choice-group-label', key: 'legend', text: 'Choices', action: null },
        { parent: 0, component: 'choice-option-label', key: 'label', text: 'Choice', action: null }, control,
      ] : [control];
      globalThis.typedCalls = [];
      const manifest = manifestApplicationView(encodeApplicationView({ revision: 1,
        actions: [{ id: 'fixture.typed', event: boolean ? 'change' : 'input' }], nodes }), root,
        { onEvent: event => globalThis.typedCalls.push(new TextDecoder().decode(event.value)) });
      manifest.requestAction('control', invalid === 'bytes' ? Uint8Array.of(0xff) : boolean ? 'yes' : '\ud800');
      globalThis.typedProof = manifest;
    }, { component, invalid, boolean });
    expect(await page.evaluate(() => ({ calls: globalThis.typedCalls, queued: globalThis.typedProof.queuedEvents(),
      refusal: globalThis.typedProof.lastRefusal() }))).toEqual({ calls: [], queued: 0, refusal: 'invalid-control-value' });
    if (boolean) {
      await page.getByRole(component === 'independent-choice' ? 'checkbox' : 'radio', { name: 'Choice', exact: true }).focus();
      await page.keyboard.press('Space');
    } else {
      await page.getByRole('textbox', { name: 'Exact text', exact: true }).focus();
      await page.keyboard.insertText('é😀');
    }
    expect(await page.evaluate(() => globalThis.typedCalls)).toEqual([boolean ? 'true' : 'é😀']);
  });
}
}

for (const input of ['keyboard', 'pointer']) {
  test(`generic action buttons do not submit an embedding form (${input})`, async ({ page }) => {
    await page.goto('/proof/browser/signal-dom-host.test.html');
    await page.evaluate(async () => {
      const { encodeApplicationView, manifestApplicationView } = await import('/targets/browser/host/assets/application-face.mjs');
      const form = document.createElement('form'); document.body.append(form);
      const root = document.createElement('section'); form.append(root);
      globalThis.embeddedProof = { actions: [], submissions: 0 };
      form.addEventListener('submit', event => { event.preventDefault(); globalThis.embeddedProof.submissions++; });
      manifestApplicationView(encodeApplicationView({ revision: 1,
        actions: [{ id: 'fixture.activate', event: 'activate' }], nodes: [
          { parent: null, component: 'button', key: 'action', text: 'Embedded action', action: 0 },
        ] }), root, { onEvent: event => globalThis.embeddedProof.actions.push(event.action) });
    });
    const button = page.getByRole('button', { name: 'Embedded action', exact: true });
    if (input === 'keyboard') { await button.focus(); await page.keyboard.press('Enter'); }
    else await button.click();
    expect(await page.evaluate(() => globalThis.embeddedProof)).toEqual({ actions: ['fixture.activate'], submissions: 0 });
  });
}
