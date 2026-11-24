import { expect, test } from "@playwright/test";
import { installPatchbayBody } from "./installed-patchbay-fixture.mjs";
import { startStaticProduct } from "./static-product-server.mjs";

test("the installed Body opens Patchbay, inspects a resident graph and returns", async ({ page, context }, info) => {
  test.setTimeout(120_000);
  const fixture = await installPatchbayBody(process.env.CONDUIT_PATCHBAY_INSTALL_BUNDLE, info);
  const { state, command, patchbay, ambient, first: initial, errors } = fixture;
  let entrance;
  try {
    entrance = await startStaticProduct(process.env.CONDUIT_PATCHBAY_HANDBOOK);
    await page.goto(entrance.url + "?participate=owner#your-handbook");
    await page.locator("[data-owner-key]").waitFor();
    const browser = () => page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
    const identity = await page.evaluate(() => globalThis.__conduitOwnerParticipation.admissionIdentity());
    const window = command("body","browser-window","--state-dir",state,
      "--expected-host-id", identity.hostId, "--new-host-verifying-key", JSON.stringify(identity.verifyingKey),
      "--maximum-millis","60000","--authorize-window");
    await page.getByLabel("Body ID").fill(window.body_id);
    await page.getByLabel("Owner window URL").fill(window.url);
    await page.getByRole("button", {name:"Join this Body", exact:true}).click();
    const face = page.locator("[data-owner-face-document]");
    await expect.poll(() => page.evaluate(() => globalThis.__conduitOwnerParticipation.presence()),
      {timeout: 12_000}).toBe("available");
    await expect(page.locator("[data-owner-face-status]")).toHaveText("The browser is showing the owner’s current Face.");
    fixture.startPlay();
    await expect.poll(() => {
      try { return command("body", "status", "--state-dir", state, "--json").realization?.play?.active_play_id; }
      catch { return ""; }
    }, {timeout: 15_000}).toMatch(/^body-play\//);
    await expect.poll(() => {
      try {
        return command("body", "face", "--state-dir", state, "--json").face.actions
          .find(action => action.intent === "conduit.intent/open-inspection@1")?.availability;
      } catch { return null; }
    }, {timeout:15_000}).toBe("Available");
    const idleShow = (await browser()).show_id;
    await page.getByRole("button", {name:"Refresh this Face", exact:true}).click();
    await expect.poll(async () => {
      const shown = await browser();
      return Boolean(shown?.show_state === "available" && shown.show_id !== idleShow);
    }).toBe(true);
    const actionResult = page.locator("[data-owner-action-result]");
    const root = page.locator("[data-handbook-application]");
    const shows = [];
    const first = await browser();
    await info.attach("home-navigation-admission", {contentType:"application/json",
      body:JSON.stringify({state:first.show_state, admitted:first.interactions_admitted,
        actions:first.actions.filter(action => action.name === "Open inspection" || action.name === "Open Body overview")})});

    const actions = [
      action => action.intent === "conduit.intent/open-inspection@1" && action.target === "plot/" + patchbay,
      action => action.intent === "patchbay/select-plot@1" && action.target === "plot/" + ambient,
      action => action.intent === "conduit.intent/open-body-overview@1",
    ];
    for (const select of actions) {
      await page.getByRole("button", {name:"Refresh this Face", exact:true}).click();
      await expect(page.locator("[data-owner-face-status]")).toHaveText("The browser is showing the owner’s current Face.");
      const closedBodyMenu = face.locator("details[data-body-menu]:not([open]) > summary");
      if (await closedBodyMenu.isVisible()) {
        await closedBodyMenu.click();
      }
      const before = await browser();
      const selected = before.actions.find(select);
      expect(selected).toBeTruthy();
      const action = face.locator(`[data-owner-action="${selected.identity}"]`).getByRole("button", {name:selected.name, exact:true});
      await expect(action).toBeEnabled();
      await expect(root).toHaveAttribute("data-owner-show-acknowledged", before.show_id);
      if (selected.intent === "conduit.intent/open-body-overview@1") {
        await action.focus();
        await expect(action).toBeFocused();
        await page.keyboard.press("Enter");
      } else {
        await action.click();
      }
      await expect(actionResult).toHaveText(`The owner accepted ${selected.name}.`);
      await expect.poll(async () => {
        const current = await browser();
        return current?.show_state === "available" && current.show_id !== before.show_id;
      }).toBe(true);
      const after = await browser();
      expect(after.body_id).toBe(window.body_id);
      expect(after.show_state).toBe("available");
      expect(after.show_id).not.toBe(before.show_id);
      if (selected.intent === "patchbay/select-plot@1") {
        const current = command("body", "face", "--state-dir", state, "--json").face;
        expect(current.properties).toContainEqual(expect.objectContaining({
          subject: selected.target, name: "patchbay-selected", value: {Flag: true},
        }));
        expect(current.subjects.filter(subject => subject.role === "Gear").length).toBeGreaterThan(0);
      }
      await expect(root).toHaveAttribute("data-owner-show-acknowledged", after.show_id);
      shows.push({action:selected.identity,before:before.show_id,after:after.show_id});
    }
    expect(shows[1].after).not.toBe(shows[0].before);
    const owner = fixture.status();
    expect(owner.realization.play.active_play_id).not.toBe(initial.realization.play.active_play_id);
    const originalCredential = await page.evaluate(() => globalThis.__conduitOwnerParticipation.credential());
    const originalShow = (await browser()).show_id;
    await page.getByRole("button", {name:"Leave this window", exact:true}).click();
    await expect.poll(() => page.evaluate(() => globalThis.__conduitOwnerParticipation.presence())).not.toBe("available");
    await page.close();
    command("body", "lull", "--state-dir", state);
    await expect.poll(() => {
      try {
        const truth = command("body", "status", "--state-dir", state, "--json");
        return truth.realization === null && truth.execution_persistence_pending === false &&
          truth.biography.membership.parts.some(part => part.part_id === originalCredential.part_id && part.current === null);
      } catch { return false; }
    }, {timeout:15_000}).toBe(true);
    const recovered = await context.newPage();
    await recovered.goto(entrance.url + "?participate=owner#your-handbook");
    await recovered.locator("[data-owner-key]").waitFor();
    const renewed = await recovered.evaluate(() => globalThis.__conduitOwnerParticipation.admissionIdentity());
    expect(renewed.hostId).toBe(identity.hostId);
    expect(renewed.bootId).not.toBe(identity.bootId);
    let ownerWindow = window.url;
    try {
      const reopened = command("body","browser-window","--state-dir",state,
        "--expected-host-id",renewed.hostId,"--new-host-verifying-key",JSON.stringify(renewed.verifyingKey),
        "--maximum-millis","60000","--authorize-window");
      expect(reopened.body_id).toBe(window.body_id);
      ownerWindow = reopened.url;
    } catch (error) {
      expect(String(error.stderr)).toContain("another browser admission window is active");
    }
    await recovered.getByLabel("Body ID").fill(window.body_id);
    await recovered.getByLabel("Owner window URL").fill(ownerWindow);
    await recovered.getByRole("button", {name:"Join this Body", exact:true}).click();
    await expect(recovered.locator("[data-owner-face-status]")).toHaveText("The browser is showing the owner’s current Face.");
    const current = await recovered.evaluate(() => ({face:globalThis.__conduitOwnerParticipation.face(),
      credential:globalThis.__conduitOwnerParticipation.credential()}));
    expect(current.credential.part_id).toBe(originalCredential.part_id);
    expect(current.credential.body_id).toBe(window.body_id);
    expect(current.face.show_id).not.toBe(originalShow);
    await expect(recovered.locator("[data-handbook-application]"))
      .toHaveAttribute("data-owner-show-acknowledged", current.face.show_id);
    const reentry = {part:originalCredential.part_id,previousBoot:identity.bootId,
      currentBoot:renewed.bootId,previousShow:originalShow,currentShow:current.face.show_id};

    await info.attach("installed-browser-patchbay-navigation", {contentType:"application/json",
      body:JSON.stringify({body_id:window.body_id,owner_host:owner.host.host_id,owner_boot:owner.host.boot_id,
        owner_play:owner.realization.play.active_play_id,browser_host:identity.hostId,browser_boot:identity.bootId,shows,reentry})});
  } finally {
    await info.attach("service-stderr",{contentType:"text/plain",body:errors.join("")});
    await info.attach("face-status", {contentType:"text/plain",
      body:await page.locator("[data-owner-face-status]").textContent({timeout:2000}).catch(() => "")});
    entrance?.child.kill();
    await fixture.stopService();
  }
});
