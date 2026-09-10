import { test, expect, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { mkdirSync } from "node:fs";
import { readFile } from "node:fs/promises";
import type { SessionExport } from "../src/lib/types";

async function connect(page: Page) {
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.getByLabel("Vision model", { exact: true }).fill("qwen3-vl:8b");
  await page
    .getByRole("button", { name: "Close settings", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toBeHidden();
}
async function start(page: Page, task = "Draft a welcome note") {
  await page.getByLabel("What would you like me to do?").fill(task);
  await page.getByRole("button", { name: "Let’s do it", exact: true }).click();
  await expect(page.getByText("Starting in a moment")).toBeVisible();
}
async function takeOver(page: Page) {
  await page.mouse.move(15, 15);
  await expect(page.getByLabel("Your refinement")).toBeVisible();
}
async function saveWorkflow(page: Page, name = "My note") {
  await page
    .getByRole("button", { name: /^(Save as workflow|Update workflow)$/ })
    .click();
  await page.getByLabel("Workflow name").fill(name);
  await page.getByRole("button", { name: /Learn & (save|update)/ }).click();
  await expect(page.getByText("Learning from this session…")).toBeVisible();
  await expect(page.getByRole("dialog")).toBeHidden();
  await expect(
    page.getByText(
      `“${name}” learned from this session. The improved start prompt is saved.`,
    ),
  ).toBeVisible();
}
async function savedWorkflows(page: Page) {
  return page.evaluate(() =>
    JSON.parse(localStorage.getItem("klickwerk-preview-workflows") ?? "[]"),
  );
}

test("first launch and useful suggestions are accessible", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByLabel("What would you like me to do?")).toBeFocused();
  await expect(
    page.getByRole("button", { name: "Let’s do it" }),
  ).toBeDisabled();
  await expect(
    page.getByText("Move your mouse or type to take over. Anytime."),
  ).toBeVisible();
  await page.getByRole("button", { name: /Make sense of your tabs/ }).click();
  await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
    /source links and unanswered questions/,
  );
  await expect(page.getByLabel("What would you like me to do?")).toBeFocused();
  await expect(page.getByText("Starting in a moment")).toBeHidden();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("settings order is URL, key, model; model loading waits for model focus", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.getByLabel("Server URL").fill("localhost:1234");
  await page.keyboard.press("Tab");
  await expect(page.locator("#api-key")).toBeFocused();
  await expect(page.getByText(/models found/)).toBeHidden();
  await page.locator("#api-key").fill("not-a-real-secret");
  const order = await page
    .locator("#connection-panel input")
    .evaluateAll((fields) => fields.map((field) => field.id));
  expect(order).toEqual(["server-url", "api-key", "model-id"]);
  await page.getByLabel("Vision model", { exact: true }).click();
  await expect(page.getByText(/3 models found/)).toBeVisible();
  await page.getByLabel("Vision model", { exact: true }).fill("qwen3-vl:8b");
  await page.getByRole("button", { name: "Test connection" }).click();
  await expect(
    page.getByText("Preview connection looks good. No server was contacted."),
  ).toBeVisible();
  await page.getByRole("tab", { name: "Preferences" }).click();
  await expect(page.getByLabel("Reduce motion")).toHaveCount(0);
  await page
    .getByRole("button", { name: "Close settings", exact: true })
    .click();
  expect(
    await page.evaluate(() =>
      localStorage.getItem("klickwerk-preview-settings"),
    ),
  ).not.toContain("not-a-real-secret");
  await page.reload();
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByLabel("Server URL")).toHaveValue(
    "http://localhost:1234/v1",
  );
  await page.getByLabel("Server URL").fill("example.org/proxy/v1");
  await page
    .getByRole("button", { name: "Close settings", exact: true })
    .click();
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByLabel("Server URL")).toHaveValue(
    "https://example.org/proxy/v1",
  );
});

test("settings autosave edits, trap focus, and return to the prompt", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await page.getByRole("button", { name: "Open settings" }).click();
  await page
    .getByLabel("Vision model", { exact: true })
    .fill("autosaved-model");
  for (let i = 0; i < 15; i++) {
    await page.keyboard.press("Tab");
    expect(
      await page.evaluate(
        () => !!document.activeElement?.closest('[role="dialog"]'),
      ),
    ).toBe(true);
  }
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await page.keyboard.press("Escape");
  await expect(page.getByLabel("What would you like me to do?")).toBeFocused();
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByLabel("Vision model", { exact: true })).toHaveValue(
    "autosaved-model",
  );
});

test("mouse takeover clearly pauses the countdown and allows unchanged continuation", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page, "Write Grüße 世界 in an editor");
  await takeOver(page);
  await expect(
    page.getByText("You took over. The agent is paused.", { exact: true }),
  ).toBeVisible();
  await expect(page.getByLabel("Your refinement")).toBeFocused();
  await page.waitForTimeout(2300);
  await expect(page.getByLabel("Your refinement")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Continue", exact: true }),
  ).toBeEnabled();
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page.getByText("Starting in a moment")).toBeVisible();
  await takeOver(page);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("keyboard takeover and corrections keep session history available on demand", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await expect(
    page.getByText("Focus the document editor", { exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Shift");
  await expect(page.getByLabel("Your refinement")).toBeFocused();
  await expect(
    page.getByRole("region", { name: "Action history" }),
  ).toBeHidden();
  await page.getByRole("button", { name: "View actions" }).click();
  await page.getByText("Click (420, 280)", { exact: true }).click();
  await expect(page.getByText("Desktop: (630, 420)")).toBeVisible();
  await page
    .getByLabel("Your refinement")
    .fill("Use the top search field instead of the sidebar.");
  await page.getByRole("button", { name: "Refine & continue" }).click();
  await expect(page.getByText("Starting in a moment")).toBeVisible();
  await expect(
    page.getByText("Use the top search field instead of the sidebar.", {
      exact: true,
    }),
  ).toBeVisible();
});

test("success returns control and allows refinement of the same completed task", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await expect(page.getByText("Done. Back to you.")).toBeVisible();
  await expect(page.getByLabel("Your refinement")).toBeFocused();
  await expect(
    page.getByText("Draft a welcome note", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Continue", exact: true }),
  ).toBeDisabled();
  await page
    .getByLabel("Your refinement")
    .fill("Add a friendly closing sentence.");
  await page.getByRole("button", { name: "Refine & continue" }).click();
  await expect(
    page.getByText("Add a friendly closing sentence.", { exact: true }),
  ).toBeVisible();
  await takeOver(page);
  await page.getByRole("button", { name: "New task" }).click();
  await expect(page.getByLabel("What would you like me to do?")).toBeEmpty();
  await expect(
    page.getByText("Draft a welcome note", { exact: true }),
  ).toBeHidden();
});

test("questions require an answer and failures allow retry or refinement", async ({
  page,
}) => {
  await page.goto("/?scenario=ask");
  await connect(page);
  await start(page, "Create a document");
  await expect(page.getByLabel("Your answer")).toBeFocused();
  await expect(
    page.getByRole("button", { name: "Continue", exact: true }),
  ).toBeDisabled();
  await page.getByLabel("Your answer").fill("Use Documents");
  await page.getByRole("button", { name: "Refine & continue" }).click();
  await expect(page.getByText("Done. Back to you.")).toBeVisible();
  await page.goto("/?scenario=error");
  await start(page, "Help with a task");
  await expect(page.getByText("Couldn’t finish. Back to you.")).toBeVisible();
  await expect(page.getByLabel("Your refinement")).toBeFocused();
  await expect(
    page.getByRole("button", { name: "Continue", exact: true }),
  ).toBeEnabled();
  await page.getByRole("button", { name: "Check connection" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
});

for (const outcome of ["done", "error", "interrupted"]) {
  test(`saving a ${outcome} run always finalizes and stores only a start prompt`, async ({
    page,
  }) => {
    await page.goto(outcome === "error" ? "/?scenario=error" : "/");
    await connect(page);
    await start(page);
    if (outcome === "interrupted") await takeOver(page);
    else
      await expect(
        page.getByText(
          outcome === "done"
            ? "Done. Back to you."
            : "Couldn’t finish. Back to you.",
        ),
      ).toBeVisible();
    await saveWorkflow(page, `${outcome} workflow`);
    const saved = await savedWorkflows(page);
    expect(saved).toHaveLength(1);
    expect(Object.keys(saved[0]).sort()).toEqual([
      "id",
      "name",
      "prompt",
      "updated_at",
    ]);
    expect(saved[0].prompt).toContain("verify each result");
    await page
      .getByRole("button", { name: `${outcome} workflow`, exact: false })
      .first()
      .click();
    await expect(
      page
        .getByRole("dialog")
        .getByText(/Included history|What the agent will remember/),
    ).toHaveCount(0);
    await expect(page.getByLabel("Start prompt")).toHaveValue(
      /Draft a welcome note/,
    );
  });
}

test("unsent corrections are consolidated and later refinements update the same workflow", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await takeOver(page);
  await page.getByLabel("Your refinement").fill("Use Documents.");
  await saveWorkflow(page);
  await page
    .getByLabel("Your refinement")
    .fill("Use Documents and add a greeting.");
  await page.getByRole("button", { name: "Refine & continue" }).click();
  await expect(page.getByText("Starting in a moment")).toBeVisible();
  await takeOver(page);
  await saveWorkflow(page, "My improved note");
  const saved = await savedWorkflows(page);
  expect(saved).toHaveLength(1);
  expect(saved[0].prompt).toContain("Use Documents.");
  expect(saved[0].prompt).toContain("Use Documents and add a greeting.");
  await page
    .getByRole("button", { name: /My improved note/ })
    .first()
    .click();
  await page.getByRole("button", { name: "Use workflow" }).click();
  await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
    /Use Documents/,
  );
  await expect(
    page.getByRole("region", { name: "Action history" }),
  ).toBeHidden();
});

test("deleting a selected workflow clears its prompt, session, and persistent entry", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await takeOver(page);
  await saveWorkflow(page);
  await page
    .getByRole("button", { name: /My note/, exact: false })
    .first()
    .click();
  await page.getByRole("button", { name: "Use workflow" }).click();
  await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
    /Draft a welcome note/,
  );
  await page
    .getByRole("button", { name: "Delete My note", exact: true })
    .click();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(
    page.getByLabel("What would you like me to do?"),
  ).not.toBeEmpty();
  await page
    .getByRole("button", { name: "Delete My note", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Delete workflow", exact: true })
    .click();
  await expect(page.getByLabel("What would you like me to do?")).toBeEmpty();
  await expect(page.locator(".selected-workflow")).toBeHidden();
  expect(await savedWorkflows(page)).toEqual([]);
  await page.reload();
  await expect(page.locator(".workflow-card")).toHaveCount(0);
});

test("deleting the current session workflow removes its result and temporary history", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await takeOver(page);
  await saveWorkflow(page);
  await page
    .getByRole("button", { name: /My note/ })
    .first()
    .click();
  await page.getByRole("button", { name: "Delete", exact: true }).click();
  await page
    .getByRole("button", { name: "Delete workflow", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toBeHidden();
  await expect(page.getByLabel("What would you like me to do?")).toBeEmpty();
  await expect(page.getByLabel("Your refinement")).toBeHidden();
  expect(await savedWorkflows(page)).toEqual([]);
});

test("editing a saved workflow always learns again and cancel keeps saved content", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await takeOver(page);
  await saveWorkflow(page);
  await page
    .getByRole("button", { name: /My note/ })
    .first()
    .click();
  await page.getByLabel("Start prompt").fill("Unsaved edits");
  await page.keyboard.press("Escape");
  await page
    .getByRole("button", { name: /My note/ })
    .first()
    .click();
  await expect(page.getByLabel("Start prompt")).not.toHaveValue(
    "Unsaved edits",
  );
  await page
    .getByLabel("Start prompt")
    .fill("Use the editor and verify the title.");
  await page.getByRole("button", { name: "Learn & update" }).click();
  await expect(page.getByText("Learning from this session…")).toBeVisible();
  await expect(page.getByRole("dialog")).toBeHidden();
  expect((await savedWorkflows(page))[0].prompt).toContain("verify the title");
});

test("failed learning preserves the session and permits an explicit unlearned save", async ({
  page,
}) => {
  await page.goto("/?learning=error");
  await connect(page);
  await start(page);
  await takeOver(page);
  await page.getByLabel("Your refinement").fill("Keep the source file.");
  await page.getByRole("button", { name: "Save as workflow" }).click();
  await page.getByLabel("Workflow name").fill("My note");
  await page.getByRole("button", { name: "Learn & save" }).click();
  await expect(page.getByText("Nothing saved yet.")).toBeVisible();
  expect(await savedWorkflows(page)).toEqual([]);
  await expect(page.getByLabel("Start prompt")).toHaveValue(
    "Draft a welcome note",
  );
  await page.getByRole("button", { name: "Save current prompt" }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
  await expect(
    page.getByText(
      "“My note” is saved with your corrections. Learning was unavailable.",
    ),
  ).toBeVisible();
  expect((await savedWorkflows(page))[0].prompt).toContain(
    "Keep the source file.",
  );
});

test("closing during learning never saves a late result", async ({ page }) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await takeOver(page);
  await page.getByRole("button", { name: "Save as workflow" }).click();
  await page.getByRole("button", { name: "Learn & save" }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();
  await page.waitForTimeout(600);
  expect(await savedWorkflows(page)).toEqual([]);
});

test.describe("German desktop", () => {
  test.use({ locale: "de-DE" });
  test("auto-detection, localized suggestions, override, and persistence", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(page.locator("html")).toHaveAttribute("lang", "de");
    await page.getByRole("button", { name: /Ein Wochenende nach Maß/ }).click();
    await expect(page.getByLabel("Was soll ich für dich tun?")).toHaveValue(
      /Frage zuerst nach Startort/,
    );
    await page.getByRole("button", { name: "Einstellungen öffnen" }).click();
    await page.getByRole("tab", { name: "Allgemein" }).click();
    await page.getByLabel("Sprache", { exact: true }).click();
    await page.getByRole("option", { name: "English", exact: true }).click();
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    await expect(page.getByText("All changes saved")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
      /Frage zuerst nach Startort/,
    );
    await page.reload();
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    await expect(
      page.getByRole("button", { name: /A weekend worth planning/ }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Open settings" }).click();
    await page.getByRole("tab", { name: "Preferences" }).click();
    await page.getByLabel("Language", { exact: true }).click();
    await page.getByRole("option", { name: /^Automatic/ }).click();
    await expect(page.locator("html")).toHaveAttribute("lang", "de");
    await expect(page.getByText("Alle Änderungen gespeichert")).toBeVisible();
    await page.keyboard.press("Escape");
    expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  });
});

test("light, dark, narrow, and handoff layouts render accessibly without overflow", async ({
  page,
}) => {
  mkdirSync("build/visual", { recursive: true });
  await page.goto("/");
  await expect(page.getByText("A few things to try")).toBeVisible();
  await page.screenshot({
    path: "build/visual/home-light.png",
    fullPage: true,
    animations: "disabled",
  });
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.screenshot({
    path: "build/visual/settings.png",
    fullPage: true,
    animations: "disabled",
  });
  await page.getByLabel("Vision model", { exact: true }).fill("qwen3-vl:8b");
  await page.getByRole("tab", { name: "Preferences" }).click();
  await page.getByLabel("Appearance", { exact: true }).click();
  await page.getByRole("option", { name: "Dark", exact: true }).click();
  await page
    .getByRole("button", { name: "Close settings", exact: true })
    .click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await page.screenshot({
    path: "build/visual/home-dark.png",
    fullPage: true,
    animations: "disabled",
  });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await start(page);
  await takeOver(page);
  await page.screenshot({
    path: "build/visual/refinement.png",
    fullPage: true,
    animations: "disabled",
  });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("prepared workflows expire when a session is reset or its workflow is deleted", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await takeOver(page);
  const result = await page.evaluate(async () => {
    const path = "/src/lib/bridge.ts";
    const { api } = await import(path);
    const initial = await api.bootstrap();
    const draft = await api.prepareWorkflow(
      { name: "Transient", prompt: "Write a note" },
      crypto.randomUUID(),
      initial.run.id,
    );
    await api.reset();
    try {
      await api.saveWorkflow(draft.token);
      return "unexpected save";
    } catch {
      return "expired";
    }
  });
  expect(result).toBe("expired");
  expect(await savedWorkflows(page)).toEqual([]);
  await start(page);
  await takeOver(page);
  await saveWorkflow(page);
  const deletion = await page.evaluate(async () => {
    const path = "/src/lib/bridge.ts";
    const { api } = await import(path);
    const snapshot = await api.bootstrap();
    const id = snapshot.workflows[0].id;
    const draft = await api.prepareWorkflow(
      { name: "Transient", prompt: "Write a note" },
      crypto.randomUUID(),
      undefined,
      undefined,
      id,
    );
    await api.deleteWorkflow(id);
    try {
      await api.saveWorkflow(draft.token);
      return "unexpected resurrection";
    } catch {
      return "expired";
    }
  });
  expect(deletion).toBe("expired");
  expect(await savedWorkflows(page)).toEqual([]);
});

test("autosave coalesces typing and flushes newer edits behind a slow save on close", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await page.evaluate(async () => {
    const path = "/src/lib/bridge.ts";
    const { api } = await import(path);
    const save = api.save;
    let inFlight = 0;
    let peak = 0;
    const writes: { model: string; hasKey: boolean; baseUrl: string }[] = [];
    Object.assign(window, { settingsSaveStats: () => ({ peak, writes }) });
    api.save = async (
      settings: { model: string; base_url: string },
      key: string | null,
    ) => {
      inFlight++;
      peak = Math.max(peak, inFlight);
      writes.push({
        model: settings.model,
        hasKey: !!key,
        baseUrl: settings.base_url,
      });
      await new Promise((resolve) => setTimeout(resolve, 800));
      try {
        return await save(settings, key);
      } finally {
        inFlight--;
      }
    };
  });
  const stats = () =>
    page.evaluate(() =>
      (
        window as typeof window & {
          settingsSaveStats: () => {
            peak: number;
            writes: { model: string; hasKey: boolean; baseUrl: string }[];
          };
        }
      ).settingsSaveStats(),
    );
  await page.getByRole("button", { name: "Open settings" }).click();
  await page
    .getByLabel("Vision model", { exact: true })
    .fill("temporary-model");
  await page.getByLabel("Vision model", { exact: true }).fill("saved-model");
  await page.locator("#api-key").fill("fixture-key");
  await expect.poll(async () => (await stats()).writes.length).toBe(1);
  expect((await stats()).writes[0]).toMatchObject({
    model: "saved-model",
    hasKey: true,
  });
  await page.getByLabel("Server URL").fill("example.org/v1");
  await page.getByLabel("Vision model", { exact: true }).fill("latest-model");
  await page.getByRole("button", { name: "Close settings" }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
  const saved = await page.evaluate(() =>
    JSON.parse(localStorage.getItem("klickwerk-preview-settings") ?? "{}"),
  );
  expect(saved).toMatchObject({
    model: "latest-model",
    base_url: "https://example.org/v1",
    api_key: "",
  });
  expect((await stats()).peak).toBe(1);
  expect((await stats()).writes.at(-1)).toMatchObject({
    model: "latest-model",
    hasKey: false,
  });
});

test("autosave failures preserve the draft and allow retry or discarding only unsaved changes", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await page.evaluate(async () => {
    const path = "/src/lib/bridge.ts";
    const { api } = await import(path);
    const save = api.save;
    let fail = true;
    window.addEventListener("allow-settings-saves", () => {
      fail = false;
    });
    api.save = (...args: unknown[]) =>
      fail ? Promise.reject(new Error("Disk is full.")) : save(...args);
  });
  await page.getByRole("button", { name: "Open settings" }).click();
  await page
    .getByLabel("Vision model", { exact: true })
    .fill("keep-this-model");
  await expect(page.getByText("Disk is full.")).toBeVisible();
  await page.getByRole("button", { name: "Close settings" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByLabel("Vision model", { exact: true })).toHaveValue(
    "keep-this-model",
  );
  await page.evaluate(() =>
    window.dispatchEvent(new Event("allow-settings-saves")),
  );
  await page.getByRole("button", { name: "Try again", exact: true }).click();
  await expect(page.getByText("All changes saved")).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        JSON.parse(localStorage.getItem("klickwerk-preview-settings") ?? "{}")
          .model,
    ),
  ).toBe("keep-this-model");
  await page.getByLabel("Server URL").fill("https://");
  await expect(page.getByText("Enter a valid server address.")).toBeVisible();
  await page.getByRole("button", { name: "Discard unsaved changes" }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByLabel("Vision model", { exact: true })).toHaveValue(
    "keep-this-model",
  );
  await expect(page.getByLabel("Server URL")).toHaveValue(
    "http://localhost:11434/v1",
  );
});

for (const locale of ["en-US", "de-DE"]) {
  test.describe(`autosave preferences in ${locale}`, () => {
    test.use({ locale });
    test("numbers and units have spaces and preferences persist without closing", async ({
      page,
    }) => {
      const german = locale === "de-DE";
      await page.goto("/");
      await page
        .getByRole("button", {
          name: german ? "Einstellungen öffnen" : "Open settings",
        })
        .click();
      await page
        .getByRole("tab", { name: german ? "Allgemein" : "Preferences" })
        .click();
      const timeout = page.getByLabel(
        german ? "Antwortzeitlimit" : "Response timeout",
      );
      const limit = page.getByLabel(german ? "Schrittlimit" : "Task limit");
      const seconds = german ? "Sekunden" : "seconds";
      const steps = german ? "Schritte" : "steps";
      await timeout.click();
      await expect(page.getByRole("option")).toHaveText(
        [30, 60, 120, 180, 300].map((count) => `${count} ${seconds}`),
      );
      await page
        .getByRole("option", { name: `180 ${seconds}`, exact: true })
        .click();
      await limit.click();
      await expect(page.getByRole("option")).toHaveText(
        [10, 25, 50, 100].map((count) => `${count} ${steps}`),
      );
      await page
        .getByRole("option", { name: `25 ${steps}`, exact: true })
        .click();
      await expect(
        page.getByText(
          german ? "Alle Änderungen gespeichert" : "All changes saved",
        ),
      ).toBeVisible();
      await expect(page.getByRole("dialog")).toBeVisible();
      await expect(
        page.getByRole("button", {
          name: german ? "Einstellungen speichern" : "Save settings",
          exact: true,
        }),
      ).toHaveCount(0);
      expect(
        await page.evaluate(() =>
          JSON.parse(
            localStorage.getItem("klickwerk-preview-settings") ?? "{}",
          ),
        ),
      ).toMatchObject({ request_timeout_seconds: 180, max_steps: 25 });
      await page.reload();
      await page
        .getByRole("button", {
          name: german ? "Einstellungen öffnen" : "Open settings",
        })
        .click();
      await page
        .getByRole("tab", { name: german ? "Allgemein" : "Preferences" })
        .click();
      await expect(timeout).toHaveText(`180 ${seconds}`);
      await expect(limit).toHaveText(`25 ${steps}`);
      await page.setViewportSize({ width: 390, height: 844 });
      expect(
        await page
          .getByRole("dialog")
          .evaluate((dialog) => dialog.scrollWidth <= dialog.clientWidth),
      ).toBe(true);
      mkdirSync("build/visual", { recursive: true });
      await page.screenshot({
        path: `build/visual/settings-autosave-${locale}-narrow.png`,
        animations: "disabled",
      });
      expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
    });
  });
}

test("settings need no completion button and outside dismissal flushes edits", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(
    page.getByText("Make it yours. Changes save automatically."),
  ).toBeVisible();
  await expect(page.locator(".settings-footer button")).toHaveCount(0);
  await page.getByLabel("Server URL").fill("localhost:9876");
  await page.mouse.click(10, 10);
  await expect(page.getByRole("dialog")).toBeHidden();
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByLabel("Server URL")).toHaveValue(
    "http://localhost:9876/v1",
  );
});

test("preference menus support keyboard selection, Escape and dialog focus", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.getByRole("tab", { name: "Preferences" }).click();
  const appearance = page.getByLabel("Appearance", { exact: true });
  await appearance.focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("listbox")).toBeVisible();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");
  await expect(appearance).toHaveText("Dark");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(appearance).toBeFocused();
  await appearance.click();
  await expect(
    page.getByRole("option", { name: "Dark", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  mkdirSync("build/visual", { recursive: true });
  await page.screenshot({
    path: "build/visual/dropdown-dark.png",
    animations: "disabled",
  });
  await page.keyboard.press("Escape");
  await expect(page.getByRole("listbox")).toBeHidden();
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(appearance).toBeFocused();
  for (let i = 0; i < 10; i++) {
    await page.keyboard.press("Tab");
    expect(
      await page.evaluate(
        () => !!document.activeElement?.closest('[role="dialog"]'),
      ),
    ).toBe(true);
  }
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();
});

test("models can be searched, selected with keyboard or mouse, and entered freely", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open settings" }).click();
  const model = page.getByRole("combobox", {
    name: "Vision model",
    exact: true,
  });
  await model.click();
  await expect(page.getByRole("option")).toHaveCount(3);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await model.fill("qwen");
  await expect(page.getByRole("option")).toHaveCount(1);
  await model.press("ArrowDown");
  await expect(model).toHaveAttribute("aria-activedescendant", /.+/);
  await model.press("Enter");
  await expect(model).toHaveValue("qwen3-vl:8b");
  await expect(page.getByRole("listbox")).toBeHidden();
  await expect(model).toBeFocused();
  await page.getByRole("button", { name: "Show models" }).click();
  await expect(page.getByRole("option")).toHaveCount(3);
  await expect(
    page.getByRole("option", { name: "qwen3-vl:8b", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  const another = page.getByRole("option").last();
  const selected = await another.innerText();
  await another.click();
  await expect(model).toHaveValue(selected);
  await model.fill("my-private-vision-model");
  await expect(
    page.getByText("No matching models. You can enter an exact model ID."),
  ).toBeVisible();
  await model.press("Escape");
  await expect(page.getByRole("listbox")).toBeHidden();
  await expect(page.getByRole("dialog")).toBeVisible();
  await model.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(model).toHaveValue("my-private-vision-model");
});

test("a late model list keeps typed IDs and remains available for filtering", async ({
  page,
}) => {
  await page.goto("/");
  await page.evaluate(async () => {
    const path = "/src/lib/bridge.ts";
    const { api } = await import(path);
    api.models = async () => {
      await new Promise((resolve) => setTimeout(resolve, 700));
      return { models: ["server-vision-model"], partial: false };
    };
  });
  await page.getByRole("button", { name: "Open settings" }).click();
  const model = page.getByRole("combobox", {
    name: "Vision model",
    exact: true,
  });
  await model.click();
  await expect(page.getByText("Loading models…")).toBeVisible();
  await model.fill("my-custom-model");
  await expect(page.getByText(/1 models found/)).toBeVisible();
  await expect(model).toHaveValue("my-custom-model");
  await model.fill("server");
  await expect(
    page.getByRole("option", { name: "server-vision-model", exact: true }),
  ).toBeVisible();
});

async function exportReport(page: Page, label = "Export history (JSON)") {
  const downloading = page.waitForEvent("download");
  await page.getByRole("button", { name: label, exact: true }).click();
  const download = await downloading;
  expect(download.suggestedFilename()).toMatch(
    /^klickwerk-session-\d+-\d+\.json$/,
  );
  const path = await download.path();
  expect(path).not.toBeNull();
  const text = await readFile(path!, "utf8");
  expect(text).not.toContain('"api_key"');
  return JSON.parse(text) as SessionExport;
}

test("JSON export preserves actions, all corrections, attempts and the learned workflow", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page, "Write Grüße 世界 in an editor");
  await expect(
    page.getByText("Focus the document editor", { exact: true }),
  ).toBeVisible();
  await takeOver(page);
  await page
    .getByLabel("Your refinement")
    .fill("Use the main editor, then add Grüße 世界.");
  await page.getByRole("button", { name: "Refine & continue" }).click();
  await expect(page.getByText("Done. Back to you.")).toBeVisible();
  await page
    .getByLabel("Your refinement")
    .fill("Next time, add a closing sentence.");
  const report = await exportReport(page);
  expect(report.schema_version).toBe(3);
  expect(report.app).toMatchObject({ name: "klickwerk", platform: "preview" });
  expect(report.run.task).toBe("Write Grüße 世界 in an editor");
  expect(report.run.phase).toBe("done");
  expect(report.run.attempts.map((a) => a.phase)).toEqual(["stopped", "done"]);
  expect(report.run.attempts.every((a) => a.finished_at! >= a.started_at)).toBe(
    true,
  );
  expect(
    report.run.steps
      .filter((s) => s.actor === "user")
      .map((s) => s.description),
  ).toEqual(["Use the main editor, then add Grüße 世界."]);
  expect(
    report.run.steps.some(
      (s) => s.action?.type === "text" && s.action.text.includes("Grüße 世界"),
    ),
  ).toBe(true);
  expect(report.unsent_refinement).toBe("Next time, add a closing sentence.");
  expect(report.workflow).toBeNull();
  expect(report.coverage).toEqual({
    history: "all_recorded_steps_and_attempts",
    screenshots: "unavailable_in_preview",
    raw_model_responses: "unavailable_in_preview",
    controller_revision: "preview",
    capture_backend: "unavailable_in_preview",
    clock: "timestamps_are_unix_ms",
  });
  expect(await savedWorkflows(page)).toHaveLength(0);
  await expect(page.getByLabel("Your refinement")).toHaveValue(
    report.unsent_refinement!,
  );
  await saveWorkflow(page, "Learned note");
  const learned = await exportReport(page);
  expect(learned.run.id).toBe(report.run.id);
  expect(learned.workflow?.id).toBe(learned.run.workflow_id);
  expect(learned.workflow?.prompt).toContain("closing sentence");
  expect(learned.warm_start_prompt).toBe(learned.workflow?.prompt);
  expect(learned.run.steps.filter((s) => s.actor === "user")).toHaveLength(2);
  expect(learned.run.attempts).toEqual(report.run.attempts);
});

test("JSON export keeps earlier failures and each attempt's model settings", async ({
  page,
}) => {
  await page.goto("/?scenario=error");
  await connect(page);
  await start(page);
  await expect(page.getByText("Couldn’t finish. Back to you.")).toBeVisible();
  await page.getByRole("button", { name: "Open settings" }).click();
  await page
    .getByRole("combobox", { name: "Vision model", exact: true })
    .fill("retry-model");
  await page.getByRole("button", { name: "Close settings" }).click();
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page.getByText("Starting in a moment")).toBeVisible();
  await expect(page.getByText("Couldn’t finish. Back to you.")).toBeVisible();
  const report = await exportReport(page);
  expect(report.run.phase).toBe("error");
  expect(report.run.attempts.map((a) => a.phase)).toEqual(["error", "error"]);
  expect(report.run.attempts.map((a) => a.settings.model)).toEqual([
    "qwen3-vl:8b",
    "retry-model",
  ]);
  expect(report.run.attempts[0].message).toContain("Cannot reach");
  expect(report.run.attempts[1].user_reply).toBe("");
  expect(
    report.run.steps.some((s) =>
      s.description.includes("without a correction"),
    ),
  ).toBe(true);
  expect(report.unsent_refinement).toBeNull();
});

test("cancelled or failed exports preserve the session and reset rejects stale exports", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await takeOver(page);
  await page.evaluate(async () => {
    const path = "/src/lib/bridge.ts";
    const { api } = await import(path);
    const original = api.exportSession;
    let attempt = 0;
    api.exportSession = (...args: unknown[]) => {
      attempt++;
      if (attempt === 1) return Promise.resolve(null);
      if (attempt === 2)
        return Promise.reject(
          new Error(
            "The history could not be saved. Check the folder permissions and available space.",
          ),
        );
      return original(...args);
    };
  });
  const button = page.getByRole("button", {
    name: "Export history (JSON)",
    exact: true,
  });
  await button.click();
  await expect(button).toBeEnabled();
  await expect(page.getByText(/History exported to/)).toBeHidden();
  await expect(page.getByLabel("Your refinement")).toBeVisible();
  await button.click();
  await expect(
    page.getByText(
      "The history could not be saved. Check the folder permissions and available space.",
    ),
  ).toBeVisible();
  const report = await exportReport(page);
  expect(report.run.phase).toBe("stopped");
  await page.getByRole("button", { name: "New task" }).click();
  await expect(button).toBeHidden();
  const error = await page.evaluate(async (id) => {
    const path = "/src/lib/bridge.ts";
    const { api } = await import(path);
    try {
      await api.exportSession(id, "");
      return "unexpected success";
    } catch (error) {
      return String(error);
    }
  }, report.run.id);
  expect(error).toContain("no longer available to export");
});

test.describe("German history export", () => {
  test.use({ locale: "de-DE" });
  test("the export is available at a question and keeps German user text", async ({
    page,
  }) => {
    await page.goto("/?scenario=ask");
    await page.getByRole("button", { name: "Einstellungen öffnen" }).click();
    await page
      .getByRole("combobox", { name: "Vision-Modell", exact: true })
      .fill("qwen3-vl:8b");
    await page.getByRole("button", { name: "Einstellungen schließen" }).click();
    await expect(page.getByRole("dialog")).toBeHidden();
    await page.locator("#task").fill("Schreibe eine Begrüßung mit Grüße 世界.");
    await page.locator(".start-button").click();
    await expect(
      page.getByRole("button", { name: "Verlauf exportieren (JSON)" }),
    ).toBeVisible();
    await page.locator("#task").fill("Bitte im Ordner Dokumente speichern.");
    const report = await exportReport(page, "Verlauf exportieren (JSON)");
    expect(report.run.phase).toBe("waiting");
    expect(report.run.question).not.toBe("");
    expect(report.run.task).toContain("Grüße 世界");
    expect(report.unsent_refinement).toBe(
      "Bitte im Ordner Dokumente speichern.",
    );
    expect(report.run.attempts[0].settings.language).toBe("German");
    await expect(
      page.getByText(/Verlauf nach klickwerk-session-/),
    ).toBeVisible();
    expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
    await page.screenshot({
      path: "build/visual/history-export-de-DE.png",
      animations: "disabled",
    });
  });
});
