import { writeFileSync } from "node:fs";
import { prepareBaseline } from "./baseline";
const output = process.argv[2];
if (!output) throw new Error("Usage: node --import tsx evals/semantic-retrieval/prepare.ts OUTPUT.json");
writeFileSync(output, JSON.stringify(prepareBaseline(), null, 2) + "\n");
