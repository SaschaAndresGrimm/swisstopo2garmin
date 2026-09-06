// Every t("…") key a component uses must exist in en.json, and every other bundle
// must not drift into keys English does not have. A missing key renders as the raw
// key string in the UI, which no typechecker catches.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const SRC = "src";
const files = [];
(function walk(dir) {
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p);
    else if (/\.tsx?$/.test(p)) files.push(p);
  }
})(SRC);

const en = JSON.parse(readFileSync(join(SRC, "i18n/en.json"), "utf8"));
const used = new Map(); // key -> file
const dynamic = [];

for (const f of files) {
  const text = readFileSync(f, "utf8");
  // t("literal.key") and t(`literal.${expr}`) — the latter is reported, not resolved.
  for (const m of text.matchAll(/\bt\(\s*"([^"]+)"/g)) used.set(m[1], f);
  for (const m of text.matchAll(/\bt\(\s*`([^`]*\$\{[^`]*)`/g)) dynamic.push([m[1], f]);
  // labelKey fields hold keys resolved through t() elsewhere.
  for (const m of text.matchAll(/labelKey:\s*"([^"]+)"/g)) used.set(m[1], f);
}

let bad = 0;
for (const [key, file] of [...used].sort()) {
  if (!(key in en)) {
    console.error(`missing in en.json: ${key}   (${file})`);
    bad++;
  }
}

// Template keys: check every prefix has at least one matching key, which catches a
// whole family being absent (e.g. no device.confidence.* at all).
for (const [tpl, file] of dynamic) {
  const prefix = tpl.split("${")[0];
  if (prefix && !Object.keys(en).some((k) => k.startsWith(prefix))) {
    console.error(`no key starts with "${prefix}" (from \`${tpl}\`)   (${file})`);
    bad++;
  }
}

for (const lang of ["de", "fr", "it"]) {
  const b = JSON.parse(readFileSync(join(SRC, `i18n/${lang}.json`), "utf8"));
  for (const k of Object.keys(b)) {
    if (!(k in en)) {
      console.error(`${lang}.json has a key en.json lacks: ${k}`);
      bad++;
    }
  }
  const missing = Object.keys(en).filter((k) => !(k in b));
  if (missing.length) console.warn(`${lang}.json falls back to English for ${missing.length} key(s)`);
}

if (bad) {
  console.error(`\n${bad} problem(s)`);
  process.exit(1);
}
console.log(`i18n ok: ${used.size} literal keys, ${dynamic.length} template keys, ${Object.keys(en).length} in en.json`);
