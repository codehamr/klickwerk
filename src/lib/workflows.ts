import type { Learning } from "./types";

export function workflowSlug(name: string) {
  let slug =
    name
      .toLowerCase()
      .replaceAll("ä", "ae")
      .replaceAll("ö", "oe")
      .replaceAll("ü", "ue")
      .replaceAll("ß", "ss")
      .split(/[^\p{L}\p{N}]+/u)
      .filter(Boolean)
      .slice(0, 4)
      .map((word) => [...word].slice(0, 16).join(""))
      .join("-") || "workflow";
  if (/^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/.test(slug)) slug += "-workflow";
  return slug;
}
export function validateLearning(value: Learning) {
  const bytes = (text: string) => new TextEncoder().encode(text).length;
  if (
    typeof value.name !== "string" ||
    typeof value.prompt !== "string" ||
    !value.name.trim() ||
    !value.prompt.trim() ||
    bytes(value.name) > 200 ||
    bytes(value.prompt) > 32768
  )
    throw new Error(
      "Use a name up to 200 bytes and a start prompt up to 32 KiB.",
    );
}
export function parseWorkflow(contents: string): Learning {
  if (new TextEncoder().encode(contents).length > 65536)
    throw new Error("Workflow files must be smaller than 64 KiB.");
  let value;
  try {
    value = JSON.parse(contents);
  } catch {
    throw new Error("Choose a valid klickwerk workflow JSON file.");
  }
  if (
    !value ||
    value.format !== "klickwerk-workflow" ||
    value.version !== 1 ||
    Object.keys(value).some(
      (key) => !["format", "version", "name", "prompt"].includes(key),
    )
  )
    throw new Error("Choose a valid klickwerk workflow JSON file.");
  validateLearning(value);
  return { name: value.name, prompt: value.prompt };
}
export function downloadWorkflow(learning: Learning) {
  const name = `${workflowSlug(learning.name)}.json`;
  const url = URL.createObjectURL(
    new Blob(
      [
        JSON.stringify(
          {
            format: "klickwerk-workflow",
            version: 1,
            name: learning.name,
            prompt: learning.prompt,
          },
          null,
          2,
        ),
      ],
      { type: "application/json" },
    ),
  );
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  document.body.appendChild(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  return name;
}
