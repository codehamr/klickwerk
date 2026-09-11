import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const repository = "codehamr/klickwerk";
const executableName = "klickwerk.exe";
const manifestName = "klickwerk-update.json";
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");

export function createManifest(bytes, tag) {
  if (
    !/^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])-([01]\d|2[0-3])[0-5]\d[0-5]\d-\d+-\d+$/.test(
      tag,
    ) ||
    tag.length > 64
  )
    throw new Error(
      "Use a UTC date/time, run number and attempt for the release tag.",
    );
  if (
    bytes.length < 256 ||
    bytes.length > 64 * 1024 * 1024 ||
    bytes.toString("ascii", 0, 2) !== "MZ"
  )
    throw new Error("The release must contain a bounded Windows executable.");
  const offset = bytes.readUInt32LE(60);
  if (
    offset < 64 ||
    offset + 26 > bytes.length ||
    bytes.readUInt32LE(offset) !== 0x4550 ||
    bytes.readUInt16LE(offset + 4) !== 0x8664 ||
    bytes.readUInt16LE(offset + 24) !== 0x20b ||
    (bytes.readUInt16LE(offset + 22) & 0x2002) !== 2
  )
    throw new Error("The release executable must be Windows x64 PE32+.");
  return {
    version: 1,
    tag,
    asset: executableName,
    sha256: hash(bytes),
    size: bytes.length,
  };
}

export function verifyAssets(assets, files) {
  if (assets.length !== files.length)
    throw new Error("The release asset set is incomplete.");
  for (const [name, bytes] of files) {
    const asset = assets.find((item) => item.name === name);
    if (
      !asset ||
      asset.state !== "uploaded" ||
      asset.size !== bytes.length ||
      asset.digest !== `sha256:${hash(bytes)}`
    )
      throw new Error(`Release asset verification failed: ${name}`);
  }
}

async function publish(directory) {
  if (
    process.env.GITHUB_REPOSITORY !== repository ||
    process.env.GITHUB_REF !== "refs/heads/main" ||
    !process.env.GITHUB_TOKEN ||
    !/^[a-f0-9]{40}$/.test(process.env.GITHUB_SHA ?? "")
  )
    throw new Error(
      "Publishing requires the main branch of codehamr/klickwerk and its Actions token.",
    );
  const exe = await readFile(join(directory, executableName));
  const manifestBytes = await readFile(join(directory, manifestName));
  const manifest = JSON.parse(manifestBytes);
  if (
    JSON.stringify(manifest) !==
      JSON.stringify(createManifest(exe, manifest.tag)) ||
    manifest.tag !== process.env.KLICKWERK_RELEASE_TAG
  )
    throw new Error("The manifest does not match this build and release tag.");
  const files = [
    [executableName, exe],
    [manifestName, manifestBytes],
  ];
  async function api(path, method = "GET", body, binary = false) {
    const url = path.startsWith("https://uploads.github.com/")
      ? path
      : `https://api.github.com/repos/${repository}/${path}`;
    const response = await fetch(url, {
      method,
      headers: {
        Authorization: `Bearer ${process.env.GITHUB_TOKEN}`,
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "Content-Type": binary
          ? "application/octet-stream"
          : "application/json",
      },
      body:
        body === undefined ? undefined : binary ? body : JSON.stringify(body),
      signal: AbortSignal.timeout(90_000),
      redirect: "error",
    });
    if (!response.ok)
      throw new Error(`GitHub ${method} failed (${response.status}).`);
    return response.json();
  }
  async function currentHead() {
    return (await api("commits/main")).sha === process.env.GITHUB_SHA;
  }
  if (!(await currentHead())) {
    console.log(
      "A newer commit is on main; this build will not become a release.",
    );
    return;
  }
  const release = await api("releases", "POST", {
    tag_name: manifest.tag,
    target_commitish: process.env.GITHUB_SHA,
    name: `klickwerk · ${manifest.tag.slice(0, 10)} ${manifest.tag.slice(11, 13)}:${manifest.tag.slice(13, 15)} UTC`,
    body: "Download **klickwerk.exe** and open it from a writable folder on Windows 11 (x64). The app checks this public repository for updates on every start and keeps your settings and workflows beside the EXE.\n\nThe small JSON file is the update manifest; users only need the EXE.\n\n[How updates work](https://github.com/codehamr/klickwerk/blob/main/docs/updates.md)",
    generate_release_notes: true,
    draft: true,
    make_latest: "false",
  });
  const upload = release.upload_url.split("{")[0];
  if (
    !upload.startsWith(
      `https://uploads.github.com/repos/${repository}/releases/${release.id}/assets`,
    )
  )
    throw new Error("GitHub returned an unexpected upload destination.");
  for (const [name, bytes] of files)
    await api(
      `${upload}?name=${encodeURIComponent(name)}`,
      "POST",
      bytes,
      true,
    );
  verifyAssets(await api(`releases/${release.id}/assets`), files);
  if (!(await currentHead())) {
    console.log(
      "A newer commit is on main; the verified release stays a draft.",
    );
    return;
  }
  await api(`releases/${release.id}`, "PATCH", {
    draft: false,
    make_latest: "true",
  });
  console.log(`Published: ${release.html_url}`);
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  try {
    const [
      command,
      directory = "build",
      tag = process.env.KLICKWERK_RELEASE_TAG,
    ] = process.argv.slice(2);
    if (command === "prepare") {
      const manifest = createManifest(
        await readFile(join(directory, executableName)),
        tag,
      );
      await writeFile(
        join(directory, manifestName),
        JSON.stringify(manifest, null, 2) + "\n",
      );
      console.log(`Prepared: ${manifest.tag} (${manifest.size} bytes)`);
    } else if (command === "publish") await publish(directory);
    else throw new Error("Use release.mjs prepare|publish [directory] [tag].");
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
