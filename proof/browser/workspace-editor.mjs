import { expect } from "@playwright/test";

export function memoryEditor(page) {
  return page.locator("[data-plot-output] textarea[data-conduit-text-editor]:visible");
}

export async function enterMemoryText(page, text) {
  const editor = memoryEditor(page);
  await expect(editor).toBeEditable();
  await editor.focus();
  await expect(editor).toBeFocused();
  await editor.fill(text);
  await expect(editor).toHaveAttribute("data-edit-disposition", "authoritative");
  await expect(editor).toHaveValue(text);
}
