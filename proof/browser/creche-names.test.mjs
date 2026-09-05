import assert from "node:assert/strict";
import test from "node:test";

import {
  nameFor,
  NAMING_SYSTEM_OPTIONS,
  NAME_NAMESPACE_SIZE,
  PERSONA_CATALOG_VERSION,
  PERSONA_SYSTEM_COUNTS,
  PERSONA_SYSTEMS,
} from "../../products/creche/browser/creche-names.mjs";

const encoder = new TextEncoder();
const UUID = "550e8400-e29b-41d4-a716-446655440000";
const systemIds = PERSONA_SYSTEMS.map(({ id }) => id);
const romanizedName = /^[\p{Script=Latin}\p{Mark} .'-]+$/u;

test("the versioned catalog retains its declared systems, stock order, and counts", () => {
  assert.equal(PERSONA_CATALOG_VERSION, 7);
  assert.deepEqual(systemIds, ["roman", "chinese", "american", "mexican", "icelandic", "japanese", "arabic", "french", "british", "classic-anglophone", "korean", "vietnamese", "yoruba", "ukrainian", "ancient-hebrew", "amharic", "portuguese", "tamil", "indonesian", "welsh", "kurmanji", "targus", "elvish"]);
  assert.deepEqual(NAMING_SYSTEM_OPTIONS.map(({ id }) => id), ["surprise", ...systemIds]);
  assert.ok(NAME_NAMESPACE_SIZE > 15_000_000n, `${NAME_NAMESPACE_SIZE} combinations is too small`);
  assert.deepEqual(Object.keys(PERSONA_SYSTEM_COUNTS), systemIds);
  assert.deepEqual(PERSONA_SYSTEMS[0].stocks[0].entries.slice(0, 3), ["Aulus", "Appius", "Decimus"]);
  assert.deepEqual(PERSONA_SYSTEMS[1].stocks[0].entries.slice(0, 3), ["Wáng", "Li", "Zhang"]);
  assert.deepEqual(PERSONA_SYSTEMS[4].stocks[2].entries, ["son", "dóttir", "bur"]);
  const targus = PERSONA_SYSTEMS.find(({ id }) => id === "targus");
  assert.equal(PERSONA_SYSTEM_COUNTS.targus, 1n);
  assert.deepEqual(targus.stocks.map(({ entries }) => entries), [["TARGUS"], ["TARGUS"]]);
  for (const [systemId, stockId] of [
    ["chinese", "chinese_given"],
    ["american", "american_given"],
    ["french", "french_given"],
    ["british", "british_given"],
    ["classic-anglophone", "classic_anglophone_given"],
    ["japanese", "japanese_family"],
    ["korean", "korean_given"],
    ["vietnamese", "vietnamese_given"],
    ["ukrainian", "ukrainian_given"],
    ["amharic", "amharic_personal"],
    ["portuguese", "portuguese_first_given"],
    ["tamil", "tamil_personal"],
    ["indonesian", "indonesian_mononym"],
    ["welsh", "welsh_family"],
    ["kurmanji", "kurmanji_personal"],
  ]) {
    const stock = PERSONA_SYSTEMS.find(({ id }) => id === systemId).stocks.find(({ id }) => id === stockId);
    assert.ok(stock.entries.length >= 64, `${stockId} lost its expanded diversity floor`);
  }
  for (const system of PERSONA_SYSTEMS) {
    for (const stock of system.stocks) {
      for (const entry of stock.entries) assert.match(entry, romanizedName, `${stock.id} contains a non-Latin component`);
    }
  }
});

test("same UUID, requested system, version, and variation reproduce name and indexes", async () => {
  const first = await nameFor(UUID, "surprise", 7);
  const second = await nameFor(UUID.toUpperCase(), "surprise", 7);
  assert.deepEqual(first, second);
  assert.equal(first.version, PERSONA_CATALOG_VERSION);
  assert.equal(first.variation, 7);
});

test("variations produce stable additional candidates without changing the UUID", async () => {
  const actual = await Promise.all(Array.from({ length: 4 }, (_, variation) => nameFor(UUID, "surprise", variation)));
  assert.deepEqual(actual.map(({ name, system_id }) => [name, system_id]), [
    ["Gonçalo Pacheco Guerreiro", "portuguese"],
    ["Đoàn Hồng Dũng", "vietnamese"],
    ["Caeluil Glenlight", "elvish"],
    ["Matvii Pavlenko", "ukrainian"],
  ]);
});

test("each naming system has a reviewable snapshot with its declared grammar", async () => {
  const actual = {};
  for (const systemId of systemIds) actual[systemId] = (await nameFor(UUID, systemId, 0)).name;
  assert.deepEqual(actual, {
    roman: "Lucius Livius Lentulus",
    chinese: "Feng Mingyu",
    american: "Ellis W. Henderson",
    mexican: "Manuel Enrique Gutiérrez Rojas",
    icelandic: "Davíð Margrétardóttir",
    japanese: "Takahashi Mei",
    arabic: "Amin ibn Nadir Rahman",
    french: "Claude Barbier Perrin",
    british: "Theo John Owen",
    "classic-anglophone": "Janet Rose Lewis",
    korean: "Cho Seung-hyeon",
    vietnamese: "Đinh Gia Nga",
    yoruba: "Fọláṣadé Ọní",
    ukrainian: "Roksolana Zozulia",
    "ancient-hebrew": "Avner ben Yehuda",
    amharic: "Nahom Alula Tsehay",
    portuguese: "Rafael Luís Azevedo Machado",
    tamil: "Devan Murugan",
    indonesian: "Nur Rahayu",
    welsh: "Elin Walters",
    kurmanji: "Zozan Silêmanî",
    targus: "TARGUS TARGUS",
    elvish: "Arvar Brightheart",
  });
});

test("classic Anglophone variations explicitly retain Bob and Susan", async () => {
  assert.equal((await nameFor(UUID, "classic-anglophone", 62)).name, "Bob Joseph Young");
  assert.equal((await nameFor(UUID, "classic-anglophone", 105)).name, "Susan Irene Lewis");
});

test("multi-given Western forms use coherent traditional pairing tracks", () => {
  for (const systemId of ["mexican", "british", "classic-anglophone", "portuguese"]) {
    const system = PERSONA_SYSTEMS.find(({ id }) => id === systemId);
    const pairedForms = system.forms.filter(({ slots }) => slots.filter((slot) => /given|middle|additional/.test(slot)).length > 1);
    assert.ok(pairedForms.length >= 2, `${systemId} lost its paired forms`);
    for (const form of pairedForms) {
      const track = form.id.includes("masculine") ? "masculine" : form.id.includes("feminine") ? "feminine" : null;
      assert.ok(track, `${form.id} has no explicit pairing track`);
      for (const slot of form.slots.filter((id) => /given|middle|additional/.test(id))) {
        assert.ok(slot.includes(track), `${form.id} crosses its ${track} pairing track through ${slot}`);
      }
    }
  }
});

test("the TARGUS family has exactly one possible name", async () => {
  const names = await Promise.all(Array.from({ length: 64 }, (_, variation) =>
    nameFor(UUID, "targus", variation).then(({ name }) => name)
  ));
  assert.deepEqual(new Set(names), new Set(["TARGUS TARGUS"]));
});

test("fixed surprise personas repeat only the deliberately singleton TARGUS family", async () => {
  const names = await Promise.all(Array.from({ length: 250 }, (_, index) => {
    const uuid = `00000000-0000-4000-8000-${index.toString(16).padStart(12, "0")}`;
    return nameFor(uuid, "surprise", 0).then(({ name }) => name);
  }));
  const counts = new Map();
  for (const name of names) counts.set(name, (counts.get(name) ?? 0) + 1);
  assert.deepEqual([...counts].filter(([, count]) => count > 1).map(([name]) => name), ["TARGUS TARGUS"]);
});

test("all forms render and sampled selections stay typed, bounded, and in range", async () => {
  const seenForms = new Set();
  for (const systemId of ["surprise", ...systemIds]) {
    for (let variation = 0; variation < 512; variation += 1) {
      const generated = await nameFor(UUID, systemId, variation);
      const system = PERSONA_SYSTEMS.find(({ id }) => id === generated.system_id);
      const form = system.forms.find(({ id }) => id === generated.form_id);
      seenForms.add(`${system.id}/${form.id}`);
      assert.match(generated.name, romanizedName, `${generated.name} is not rendered in Latin script`);
      assert.ok(encoder.encode(generated.name).length <= 64, `${generated.name} exceeded the byte bound`);
      assert.deepEqual(generated.stock_indexes.map(({ stock_id }) => stock_id), form.slots);
      for (const { stock_id, index } of generated.stock_indexes) {
        const stock = system.stocks.find(({ id }) => id === stock_id);
        assert.ok(index >= 0 && index < stock.entries.length, `${stock_id}[${index}] is outside its stock`);
        assert.ok(stock_id.startsWith(`${system.id.replaceAll("-", "_")}_`), `${stock_id} escaped ${system.id}`);
      }
      if (systemId !== "surprise") assert.equal(generated.system_id, systemId);
    }
  }
  const declaredForms = PERSONA_SYSTEMS.flatMap((system) => system.forms.map((form) => `${system.id}/${form.id}`));
  assert.deepEqual([...seenForms].sort(), declaredForms.sort());
});

test("persona collisions remain labels and never collapse distinct UUID inputs", async () => {
  const seen = new Map();
  let collision = null;
  const chineseNamespaceSize = Number(PERSONA_SYSTEM_COUNTS.chinese);
  for (let index = 0; index <= chineseNamespaceSize && collision === null; index += 1) {
    const uuid = `00000000-0000-0000-0000-${index.toString(16).padStart(12, "0")}`;
    const generated = await nameFor(uuid, "chinese", 0);
    if (seen.has(generated.name)) collision = { first: seen.get(generated.name), second: uuid, name: generated.name };
    else seen.set(generated.name, uuid);
  }
  assert.ok(collision, `the finite Chinese stock must collide across ${chineseNamespaceSize + 1} UUIDs`);
  assert.notEqual(collision.first, collision.second);
});

test("malformed UUIDs, variations, and systems refuse rather than changing derivation", async () => {
  await assert.rejects(nameFor("not-a-uuid"), /canonical UUID/);
  await assert.rejects(nameFor(UUID, "unknown"), /unknown persona naming system/);
  await assert.rejects(nameFor(UUID, "roman", -1), /unsigned 32-bit integer/);
  await assert.rejects(nameFor(UUID, "roman", 0, null), /SHA-256 provider/);
});
