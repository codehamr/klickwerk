import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createManifest, verifyAssets } from "./release.mjs";

function fixture() {
  const image = Buffer.alloc(512);
  image.write("MZ");
  image.writeUInt32LE(64, 60);
  image.writeUInt32LE(0x4550, 64);
  image.writeUInt16LE(0x8664, 68);
  image.writeUInt16LE(2, 86);
  image.writeUInt16LE(0x20b, 88);
  return image;
}
test("the published manifest pins the exact portable executable", () => {
  const image = fixture();
  assert.deepEqual(createManifest(image, "2026-09-11-123456-12-1"), {
    version: 1,
    tag: "2026-09-11-123456-12-1",
    asset: "klickwerk.exe",
    sha256: createHash("sha256").update(image).digest("hex"),
    size: image.length,
  });
  for (const tag of [
    "latest",
    "../main",
    "2026-09-11",
    "2026-13-11-120000-1-1",
    "2026-09-11-240000-1-1",
  ])
    assert.throws(() => createManifest(image, tag));
  assert.throws(() =>
    createManifest(Buffer.from("not an executable"), "2026-09-11-123456-12-1"),
  );
  image.writeUInt16LE(0x014c, 68);
  assert.throws(() => createManifest(image, "2026-09-11-123456-12-1"));
});
test("only complete uploaded assets with matching GitHub digests may go live", () => {
  const image = fixture();
  const manifest = Buffer.from(
    JSON.stringify(createManifest(image, "2026-09-11-123456-12-1")),
  );
  const files = [
    ["klickwerk.exe", image],
    ["klickwerk-update.json", manifest],
  ];
  const assets = files.map(([name, bytes]) => ({
    name,
    size: bytes.length,
    state: "uploaded",
    digest: `sha256:${createHash("sha256").update(bytes).digest("hex")}`,
  }));
  verifyAssets(assets, files);
  assert.throws(() => verifyAssets(assets.slice(0, 1), files));
  for (const change of [
    { digest: "sha256:wrong" },
    { size: 1 },
    { name: "renamed.exe" },
    { state: "starter" },
  ])
    assert.throws(() =>
      verifyAssets([{ ...assets[0], ...change }, assets[1]], files),
    );
});
