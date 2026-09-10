import { rmSync } from "node:fs";
for (const directory of ["dist", "playwright-report", "test-results"]) {
  try {
    rmSync(directory, { recursive: true, force: true });
  } catch (error) {
    console.error(`Could not remove ${directory}: ${error.message}`);
    process.exitCode = 1;
  }
}
