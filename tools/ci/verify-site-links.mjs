#!/usr/bin/env node

// Check the assembled Pages tree before promotion. A browser can follow a
// relative link only if its target is present in this exact publication.
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const PUBLIC_ROOT = 'https://dancxjo.github.io/conduit/';
const ATTRIBUTES = /\b(?:href|src)\s*=\s*(?:"([^"]*)"|'([^']*)')/gi;

export function verifySiteLinks(root) {
  const site = path.resolve(root);
  const failures = [];
  const htmlCache = new Map();
  let pages = 0;
  let references = 0;

  function visit(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(file);
      else if (entry.isFile() && entry.name.endsWith('.html')) checkPage(file);
    }
  }

  function checkPage(file) {
    pages += 1;
    const html = readFileSync(file, 'utf8');
    const source = path.relative(site, file).split(path.sep).join('/');
    const base = new URL(source, PUBLIC_ROOT);
    for (const match of html.matchAll(ATTRIBUTES)) {
      const reference = (match[1] ?? match[2]).trim();
      if (!reference) continue;
      let url;
      try { url = new URL(reference, base); }
      catch { failures.push(`${source}: invalid URL ${reference}`); continue; }
      if (url.origin !== base.origin || !url.pathname.startsWith('/conduit/')) continue;
      references += 1;
      let target;
      try { target = path.join(site, decodeURIComponent(url.pathname.slice('/conduit/'.length))); }
      catch { failures.push(`${source}: invalid path ${reference}`); continue; }
      const relative = path.relative(site, target);
      if (relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
        failures.push(`${source}: path escapes the site ${reference}`);
        continue;
      }
      if (existsSync(target) && statSync(target).isDirectory()) target = path.join(target, 'index.html');
      if (!existsSync(target) || !statSync(target).isFile()) {
        failures.push(`${source}: ${reference} is absent`);
        continue;
      }
      if (url.hash && target.endsWith('.html')) {
        let anchor;
        try { anchor = decodeURIComponent(url.hash.slice(1)); }
        catch { failures.push(`${source}: invalid anchor ${reference}`); continue; }
        if (!htmlCache.has(target)) htmlCache.set(target, readFileSync(target, 'utf8'));
        const destination = htmlCache.get(target);
        if (anchor && !destination.includes(`id="${anchor}"`)
          && !destination.includes(`id='${anchor}'`)
          && !destination.includes(`name="${anchor}"`)
          && !destination.includes(`name='${anchor}'`)) {
          failures.push(`${source}: ${reference} has no anchor`);
        }
      }
    }
  }

  visit(site);
  if (pages === 0) failures.push('site contains no HTML pages');
  if (failures.length) throw new Error(`Assembled site has ${failures.length} broken local links:\n${failures.join('\n')}`);
  return { pages, references };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  if (process.argv.length !== 3) throw new Error('usage: verify-site-links.mjs SITE');
  const result = verifySiteLinks(process.argv[2]);
  console.log(`Verified ${result.references} local references in ${result.pages} Pages documents`);
}
