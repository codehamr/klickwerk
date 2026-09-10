import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { homedir } from "node:os";

const windows = process.platform === "win32";
const target = "x86_64-pc-windows-msvc";
const targetDir =
  process.env.CARGO_TARGET_DIR ||
  (windows
    ? resolve("src-tauri/target")
    : join(homedir(), ".cache/klickwerk/target"));
const env = { ...process.env, CARGO_TARGET_DIR: targetDir };
function run(command, args, overrides = {}) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    env,
    ...overrides,
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`${command} failed (exit ${result.status}).`);
}
try {
  if (!existsSync("dist/index.html"))
    run(windows ? "npm.cmd" : "npm", ["run", "build"], { shell: windows });
  run("cargo", [
    ...(windows ? [] : ["xwin"]),
    "build",
    "--locked",
    "--release",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "--target",
    target,
    "--features",
    "safety-tests,tauri/custom-protocol",
    "--bin",
    "klickwerk-safety-tests",
  ]);
  const exe = join(targetDir, target, "release", "klickwerk-safety-tests.exe");
  const args = process.argv.includes("--with-disposable-input")
    ? ["--with-disposable-input"]
    : [];
  const runs = [args];
  const replayIndex = process.argv.indexOf("--replay-export");
  if (replayIndex !== -1) {
    const exportPath = process.argv[replayIndex + 1];
    if (!exportPath || !existsSync(exportPath))
      throw new Error(
        "Provide an existing session export after --replay-export.",
      );
    const path = resolve(exportPath);
    runs.push([
      "--recovery-replay",
      windows ? path : `Z:${path.replaceAll("/", "\\")}`,
    ]);
  }
  if (windows) {
    for (const args of runs) run(exe, args);
  } else {
    const wine = process.env.WINE_BINARY || "/usr/lib/wine/wine64";
    if (!existsSync(wine))
      throw new Error(
        "Install the optional wine64, xvfb, xauth, and fonts-dejavu-core packages first.",
      );
    for (const args of runs)
      run(
        "xvfb-run",
        ["-a", "-s", "-screen 0 1440x900x24", wine, exe, ...args],
        {
          env: {
            ...env,
            WINEPREFIX: join(homedir(), ".cache/klickwerk/wine-tests"),
            WINEARCH: "win64",
            WINEDEBUG: "-all",
            WINEDLLOVERRIDES: "mscoree,mshtml=",
          },
          timeout: 120_000,
        },
      );
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
