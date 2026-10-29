// Exercise offered library navigation against the installed owner, never a
// renderer-owned document or constructed Face.
import assert from 'node:assert/strict';
import path from 'node:path';
import { writeFile } from 'node:fs/promises';

export async function proveNotebookNavigation(page, bodyId, notebook, output, ordinal) {
  const view = () => page.evaluate(() => globalThis.__conduitOwnerParticipation.face());
  const before = await view(), document = notebook(['show']);
  const navigate = async (label, context) => {
    const surface = page.locator('[data-owner-face]');
    const menu = surface.locator('details[data-body-menu]');
    if (await menu.count() && await menu.getAttribute('open') === null) {
      await menu.locator(':scope > summary').click();
    }
    try {
      await surface.getByRole('button', { name: label, exact: true }).click();
    } catch (error) {
      await writeFile(path.join(output, `notebook-navigation-failure-${ordinal}.json`), JSON.stringify(await view(), null, 2));
      await page.screenshot({ path: path.join(output, `notebook-navigation-failure-${ordinal}.png`), fullPage: true });
      throw error;
    }
    await page.waitForFunction(context => globalThis.__conduitOwnerParticipation.face()?.subjects?.some(subject =>
      subject.properties?.includes(`operator-context: ${context}`)), context);
    const face = await view();
    assert.equal(face.body_id, bodyId);
    assert.deepEqual(notebook(['show']), document, 'navigation preserves the owner document');
    return face;
  };
  const library = await navigate('Open Plots library', 'library');
  const resident = library.subjects.find(subject => subject.name === 'notebook/transition' && subject.role === 'Plot');
  assert.ok(resident, 'library exposes the real resident Notebook Plot');
  const checked = resident.properties.find(property => property.startsWith('checked-plot-id: ')).slice('checked-plot-id: '.length);
  assert.equal(resident.identity, `plot/${checked}`);
  const open = library.actions.find(action => action.intent === 'conduit.intent/open-resident-plot@1' && action.target === resident.identity);
  assert.equal(open.availability, 'available');
  await page.locator('[data-owner-face]').screenshot({ path: path.join(output, `notebook-library-${ordinal}.png`) });
  const opened = await navigate('Open resident Plot', 'resident-plot');
  const overview = await navigate('Open Body overview', 'overview');
  assert.ok(BigInt(library.face_revision) > BigInt(before.face_revision));
  assert.ok(BigInt(opened.face_revision) > BigInt(library.face_revision));
  assert.ok(BigInt(overview.face_revision) > BigInt(opened.face_revision));
  return { body_id: bodyId, resident, open_action: open, before, library, opened, overview, document };
}
