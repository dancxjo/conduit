import { test, expect } from '@playwright/test';

test('syntax overlay preserves BOM spans and oversized editing falls back before encoding', async ({ page }) => {
  await page.goto('/proof/browser/signal-dom-host.test.html');
  await page.evaluate(async () => {
    const { attachConduitSyntaxEditor } = await import('/targets/browser/host/assets/application-syntax-face.mjs');
    const { syntaxProjectionFixture } = await import('/proof/browser/syntax-projection-fixture.mjs');
    const fixture = syntaxProjectionFixture([[0, 1, 3], [1, 4, 0], [4, 5, 3]]);
    const textarea = document.createElement('textarea');
    textarea.setAttribute('aria-label', 'Exact source'); textarea.value = 'x\ufeffy'; document.body.append(textarea);
    attachConduitSyntaxEditor(textarea, fixture.runtime);
    globalThis.syntaxFixture = fixture;
  });
  const editor = page.getByRole('textbox', { name: 'Exact source', exact: true });
  const overlay = page.locator('.syntax-highlight code');
  expect(await overlay.textContent()).toBe('x\ufeffy');
  await expect(editor).toHaveAttribute('data-syntax-disposition', 'accepted');
  await page.evaluate(() => {
    const original = TextEncoder.prototype.encode;
    globalThis.oversizedSyntaxEncodings = 0;
    TextEncoder.prototype.encode = function(value) {
      if (value.length > 8192) { globalThis.oversizedSyntaxEncodings++; throw new Error('oversized encoding attempted'); }
      return original.call(this, value);
    };
    globalThis.restoreSyntaxEncoder = () => { TextEncoder.prototype.encode = original; };
  });
  await editor.fill('x'.repeat(8193));
  await expect(editor).toHaveAttribute('data-syntax-disposition', 'refused');
  expect(await overlay.textContent()).toBe('x'.repeat(8193));
  expect(await page.evaluate(() => ({ encodings: globalThis.oversizedSyntaxEncodings, calls: globalThis.syntaxFixture.calls() })))
    .toEqual({ encodings: 0, calls: 1 });
  await page.evaluate(() => globalThis.restoreSyntaxEncoder());
  await editor.fill('x\ufeffy');
  await expect(editor).toHaveAttribute('data-syntax-disposition', 'accepted');
  expect(await overlay.textContent()).toBe('x\ufeffy');
});
