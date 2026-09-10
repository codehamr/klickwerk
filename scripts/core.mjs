import { spawnSync } from "node:child_process";
import { homedir } from "node:os";
import { join } from "node:path";
import { existsSync } from "node:fs";

const env = { ...process.env };
if (process.platform === "win32" && !existsSync("dist/index.html")) {
  const build = spawnSync("npm", ["run", "build"], {
    stdio: "inherit",
    shell: true,
  });
  if (build.status !== 0) process.exit(build.status ?? 1);
}
if (process.platform !== "win32" && !env.CARGO_TARGET_DIR)
  env.CARGO_TARGET_DIR = join(homedir(), ".cache/klickwerk/target");
const result = spawnSync(
  "cargo",
  ["test", "--locked", "--manifest-path", "src-tauri/Cargo.toml", "--lib"],
  { stdio: "inherit", env },
);
if (result.error) console.error(result.error.message);
process.exitCode = result.status ?? 1;
