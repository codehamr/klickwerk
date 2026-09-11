import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

async function library(page: import("@playwright/test").Page) {
  return page.evaluate(() =>
    JSON.parse(localStorage.getItem("klickwerk-preview-workflows") ?? "[]"),
  );
}

test("an unrun prompt saves without a model or learning request and survives reload", async ({
  page,
}) => {
  await page.goto("/?learning=error");
  await page
    .getByLabel("What would you like me to do?")
    .fill("Prepare the monthly report");
  await page
    .getByRole("button", { name: "Save as workflow", exact: true })
    .click();
  await page.getByLabel("Workflow name").fill("März Bericht");
  await expect(
    page.getByText("workflows/maerz-bericht.json", { exact: false }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Save workflow", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toBeHidden();
  const saved = await library(page);
  expect(saved).toHaveLength(1);
  expect(saved[0].prompt).toBe("Prepare the monthly report");
  await expect(page.getByText("Starting in a moment")).toBeHidden();
  await page.reload();
  await expect(
    page.getByRole("button", { name: /^März Bericht/ }),
  ).toBeVisible();
});

test("demonstration is explicit, pauses, and produces an editable draft before saving", async ({
  page,
}) => {
  await page.goto("/");
  await page
    .getByLabel("What would you like me to do?")
    .fill("Write a welcome note");
  await page.getByRole("button", { name: "Show me how", exact: true }).click();
  await expect(
    page.getByLabel("Include typed text", { exact: false }),
  ).toBeChecked();
  await expect(
    page.getByLabel("Include occasional screenshots", { exact: false }),
  ).not.toBeChecked();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await page
    .getByRole("button", { name: "Start recording", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Recording your example" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Pause recording", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Recording paused" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open settings" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Resume recording" }).click();
  await page.getByRole("button", { name: "Finish recording" }).last().click();
  await expect(page.getByText("Learning from this session…")).toBeVisible();
  await expect(
    page.getByText(
      "Your example is now a reusable prompt. Adjust anything before saving.",
    ),
  ).toBeVisible();
  expect(await library(page)).toHaveLength(0);
  await page.getByLabel("Workflow name").fill("Welcome");
  await page
    .getByLabel("Start prompt", { exact: true })
    .fill("Write a welcome note. Verify the recipient first.");
  await page
    .getByRole("button", { name: "Save workflow", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toBeHidden();
  expect((await library(page))[0].prompt).toBe(
    "Write a welcome note. Verify the recipient first.",
  );
  await expect(page.getByText("Working on your desktop")).toBeHidden();
});

test("failed training analysis retains the session and offers an explicit prompt fallback", async ({
  page,
}) => {
  await page.goto("/?learning=error");
  await page
    .getByLabel("What would you like me to do?")
    .fill("Create a report");
  await page.getByRole("button", { name: "Show me how", exact: true }).click();
  await page
    .getByRole("button", { name: "Start recording", exact: true })
    .click();
  await page.getByRole("button", { name: "Finish recording" }).last().click();
  await expect(
    page.getByRole("button", { name: "Save current prompt" }),
  ).toBeVisible();
  expect(await library(page)).toHaveLength(0);
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await page
    .getByRole("button", { name: "Save as workflow", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Save current prompt" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Save current prompt" }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
  expect((await library(page))[0].prompt).toBe("Create a report");
});

test("import reviews a portable file, exports it without evidence, and rejects invalid input", async ({
  page,
}) => {
  await page.goto("/");
  const portable = {
    format: "klickwerk-workflow",
    version: 1,
    name: "März Bericht",
    prompt: "Summarize the report. Grüße 世界",
  };
  await page.getByLabel("Choose workflow file").setInputFiles({
    name: "report.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(portable)),
  });
  await expect(
    page.getByText(
      "Review this imported prompt before saving. It will be added as a separate workflow.",
    ),
  ).toBeVisible();
  expect(await library(page)).toHaveLength(0);
  await page
    .getByRole("button", { name: "Save workflow", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toBeHidden();
  // Home starts a fresh session without running the imported prompt.
  await page.getByLabel("klickwerk home").click();
  await page.getByRole("button", { name: /^März Bericht/ }).click();
  const downloadPromise = page.waitForEvent("download");
  await page
    .getByRole("button", { name: "Export workflow", exact: true })
    .click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe("maerz-bericht.json");
  const stream = await download.createReadStream();
  const chunks = [];
  for await (const chunk of stream!) chunks.push(chunk);
  expect(JSON.parse(Buffer.concat(chunks).toString())).toEqual(portable);
  await page.keyboard.press("Escape");
  await page.getByLabel("Choose workflow file").setInputFiles({
    name: "bad.json",
    mimeType: "application/json",
    buffer: Buffer.from('{"format":"klickwerk-workflow","version":99}'),
  });
  await expect(
    page.getByText("Choose a valid klickwerk workflow JSON file."),
  ).toBeVisible();
  expect(await library(page)).toHaveLength(1);
  await page.getByLabel("Choose workflow file").setInputFiles({
    name: "huge.json",
    mimeType: "application/json",
    buffer: Buffer.alloc(65537),
  });
  await expect(
    page.getByText("Workflow files must be smaller than 64 KiB."),
  ).toBeVisible();
});

test("German demonstration controls and unrun workflow saving are translated", async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      "klickwerk-preview-settings",
      JSON.stringify({ language: "de" }),
    ),
  );
  await page.goto("/");
  await page.locator("#task").fill("Erstelle einen Bericht");
  await page.getByRole("button", { name: "Vormachen", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Aufnahme starten" }),
  ).toBeVisible();
  await expect(
    page.getByText("Gelegentliche Screenshots aufnehmen"),
  ).toBeVisible();
  await page.getByRole("button", { name: "Aufnahme starten" }).click();
  await page.getByRole("button", { name: "Aufnahme beenden" }).last().click();
  await expect(
    page.getByRole("button", { name: "Workflow speichern", exact: true }),
  ).toBeVisible();
});

test("closing training analysis never persists a late result and recording invalidates older drafts", async ({
  page,
}) => {
  await page.goto("/");
  await page.locator("#task").fill("Prepare sample notes");
  await page.getByRole("button", { name: "Show me how", exact: true }).click();
  await page
    .getByRole("button", { name: "Start recording", exact: true })
    .click();
  await page.getByRole("button", { name: "Finish recording" }).last().click();
  await expect(page.getByText("Learning from this session…")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();
  await page.waitForTimeout(600);
  expect(await library(page)).toEqual([]);
  const result = await page.evaluate(async () => {
    const module = "/src/lib/bridge.ts";
    const { api } = await import(module);
    const before = await api.bootstrap();
    const draft = await api.prepareWorkflow(
      { name: "Old draft", prompt: before.run.task },
      crypto.randomUUID(),
      before.run.id,
    );
    await api.startTraining(
      before.run.task,
      { text: false, screenshots: false },
      before.run.id,
    );
    try {
      await api.saveWorkflow(draft.token);
      return "unexpected save";
    } catch {
      return "expired";
    } finally {
      await api.stop();
    }
  });
  expect(result).toBe("expired");
  expect(await library(page)).toEqual([]);
});
