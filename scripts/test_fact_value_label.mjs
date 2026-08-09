// Quick sanity check: verify factValueLabel handles all serde-serialized
// FactValue variants without throwing.
//
// Run: bun scripts/test_fact_value_label.mjs

function factValueLabel(v) {
  if (v == null) return "—";
  if (typeof v === "string") return v.toLowerCase();
  if (typeof v !== "object") return String(v);
  if ("Bool" in v) return v.Bool ? "true" : "false";
  if ("Str" in v) return `"${v.Str}"`;
  if ("Int" in v) return String(v.Int);
  if ("Float" in v) return v.Float.toFixed(2);
  if ("Entity" in v) return `→${v.Entity}`;
  if ("List" in v) return `[${v.List.map(factValueLabel).join(", ")}]`;
  try { return JSON.stringify(v); } catch { return "?"; }
}

const cases = [
  ["Bool(true)",   { Bool: true },            "true"],
  ["Bool(false)",  { Bool: false },           "false"],
  ["Str(\"x\")",   { Str: "x" },              "\"x\""],
  ["Int(5)",       { Int: 5 },                "5"],
  ["Float(3.14)",  { Float: 3.14 },           "3.14"],
  ["EntityRef",    { Entity: "alice" },       "→alice"],
  ["List",         { List: [{Int:1},{Str:"a"}] }, "[1, \"a\"]"],
  ["Unknown",      "Unknown",                 "unknown"],
  ["null",         null,                      "—"],
  ["undefined",    undefined,                 "—"],
];

let failures = 0;
for (const [name, input, expected] of cases) {
  let actual;
  try {
    actual = factValueLabel(input);
  } catch (e) {
    console.error(`FAIL ${name}: threw ${e.constructor.name}: ${e.message}`);
    failures++;
    continue;
  }
  if (actual !== expected) {
    console.error(`FAIL ${name}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
    failures++;
  } else {
    console.log(`OK   ${name} → ${JSON.stringify(actual)}`);
  }
}

if (failures > 0) {
  console.error(`\n${failures} failure(s)`);
  process.exit(1);
} else {
  console.log(`\nAll ${cases.length} cases passed`);
}
