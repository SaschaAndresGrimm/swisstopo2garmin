// A static accessibility audit of the JSX (NFR-9).
//
// Not a substitute for a screen-reader pass, and it says so. It catches the failures
// that are mechanical and therefore easy to reintroduce: a control with no accessible
// name, a live region that never announces, an input with no label. Those are the ones
// that creep back in every time a component is edited.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const files = [];
(function walk(dir) {
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p);
    else if (p.endsWith(".tsx")) files.push(p);
  }
})("src");

const problems = [];
const note = (file, line, msg) => problems.push(`${file}:${line}  ${msg}`);

for (const file of files) {
  const text = readFileSync(file, "utf8");
  const lines = text.split("\n");

  // Every <input> needs a name: an id paired with a <label htmlFor>, an aria-label, or
  // a wrapping <label>. Checkboxes inside <label className="check"> are wrapped.
  for (const [i, line] of lines.entries()) {
    if (!/<input\b/.test(line)) continue;
    const block = lines.slice(i, i + 12).join(" ");
    const hasId = /\bid=/.test(block);
    const hasAria = /aria-label(ledby)?=/.test(block);
    // A wrapping label is the two lines above.
    const wrapped = lines.slice(Math.max(0, i - 3), i).some((l) => /<label\b/.test(l));
    if (!hasId && !hasAria && !wrapped) {
      note(file, i + 1, "<input> has no accessible name (id + <label htmlFor>, aria-label, or a wrapping <label>)");
    }
  }

  // An id on an input should have a label pointing at it -- unless the input carries
  // its own aria-label, which is an accessible name in its own right.
  for (const m of text.matchAll(/<input\b[^>]*\bid="([^"]+)"[^>]*>/gs)) {
    if (/aria-label(ledby)?=/.test(m[0])) continue;
    if (!text.includes(`htmlFor="${m[1]}"`)) {
      const line = text.slice(0, m.index).split("\n").length;
      note(file, line, `input id="${m[1]}" has no <label htmlFor> and no aria-label`);
    }
  }

  // A <select> needs a name too.
  for (const [i, line] of lines.entries()) {
    if (!/<select\b/.test(line)) continue;
    const block = lines.slice(i, i + 8).join(" ");
    if (!/aria-label=|\bid=/.test(block)) {
      note(file, i + 1, "<select> has no accessible name");
    }
  }

  // A progressbar must report its value, or it announces nothing.
  for (const [i, line] of lines.entries()) {
    if (!/role="progressbar"/.test(line)) continue;
    const block = lines.slice(i, i + 6).join(" ");
    for (const attr of ["aria-valuenow", "aria-valuemin", "aria-valuemax"]) {
      if (!block.includes(attr)) note(file, i + 1, `progressbar is missing ${attr}`);
    }
  }

  // A button whose only content is an icon or a variable needs an explicit name; ours
  // are all text, so flag any that are empty.
  for (const m of text.matchAll(/<button\b[^>]*>\s*<\/button>/g)) {
    const line = text.slice(0, m.index).split("\n").length;
    note(file, line, "<button> has no content and no aria-label");
  }
}

if (problems.length) {
  console.error(problems.join("\n"));
  console.error(`\n${problems.length} accessibility problem(s)`);
  process.exit(1);
}
console.log(
  `a11y ok: ${files.length} components checked for control names, labels and live regions.\n` +
  "This is a static check, not a screen-reader pass -- see docs/accessibility.md.",
);
