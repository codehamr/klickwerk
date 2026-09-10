import { spawnSync } from "node:child_process";
import { mkdirSync, copyFileSync, statSync, renameSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { homedir } from "node:os";

const started = Date.now();
const windows = process.platform === "win32";
const target = "x86_64-pc-windows-msvc";
const targetDir =
  process.env.CARGO_TARGET_DIR ||
  (windows
    ? resolve("src-tauri/target")
    : join(homedir(), ".cache/klickwerk/target"));
const env = { ...process.env, CARGO_TARGET_DIR: targetDir };
const staged = resolve(`build/klickwerk.next-${process.pid}.exe`);
function run(command, args) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    env,
    shell: windows,
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`${command} failed (exit ${result.status}).`);
}
try {
  run("npm", ["run", "build"]);
  // cargo-xwin prompts for the Microsoft SDK license on the first cross-build.
  run("cargo", [
    ...(windows ? [] : ["xwin"]),
    "build",
    "--locked",
    "--release",
    "--features",
    "tauri/custom-protocol",
    "--bin",
    "klickwerk",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "--target",
    target,
  ]);
  mkdirSync("build", { recursive: true });
  const source = join(targetDir, target, "release", "klickwerk.exe");
  const destination = resolve("build/klickwerk.exe");
  copyFileSync(source, staged);
  renameSync(staged, destination);
  console.log(
    `Built: ${destination} (${(statSync(destination).size / 1024 / 1024).toFixed(2)} MiB)`,
  );
  console.log("Settings are created at first launch: build/config.cfg");
} catch (error) {
  console.error(`Build failed: ${error.message}`);
  console.error("The last working EXE and config.cfg were preserved.");
  process.exitCode = 1;
} finally {
  rmSync(staged, { force: true });
  console.log(`Build time: ${((Date.now() - started) / 1000).toFixed(1)}s`);
}
