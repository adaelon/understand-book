// Read a built mathematical index by source LID or printed label.
// tsx skills/build/math-structure-query.ts <book-workspace> <label-or-lid>
import { readFileSync } from "node:fs";
import path from "node:path";
import { MathStructureIndexZ, queryMathStructure } from "../../packages/core/src/math-structure";

const [workspace, ...terms] = process.argv.slice(2);
if (!workspace || !terms.length) {
  console.error("usage: math-structure-query.ts <book-workspace> <label-or-lid>");
  process.exit(2);
}
const index = MathStructureIndexZ.parse(JSON.parse(readFileSync(path.join(workspace, "math_structure.json"), "utf8")));
console.log(JSON.stringify({ book_id: index.header.book_id, items: queryMathStructure(index, terms.join(" ")) }, null, 2));
