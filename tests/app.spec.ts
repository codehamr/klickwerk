import { test, expect, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { mkdirSync } from "node:fs";

async function connect(page: Page) {
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.getByLabel("Vision model", { exact: true }).fill("qwen3-vl:8b");
  await page.getByRole("button", { name: "Save settings" }).click();
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
  await page.getByRole("button", { name: "Save settings" }).click();
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
  await page.getByRole("button", { name: "Save settings" }).click();
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByLabel("Server URL")).toHaveValue(
    "https://example.org/proxy/v1",
  );
});

test("settings cancel edits, trap focus, and return to the prompt", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.getByLabel("Vision model", { exact: true }).fill("unsaved-model");
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
    "qwen3-vl:8b",
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
    await page.getByLabel("Sprache", { exact: true }).selectOption("en");
    await page.getByRole("button", { name: "Einstellungen speichern" }).click();
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
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
    await page.getByLabel("Language", { exact: true }).selectOption("system");
    await page.getByRole("button", { name: "Save settings" }).click();
    await expect(page.locator("html")).toHaveAttribute("lang", "de");
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
  await page.getByLabel("Appearance").selectOption("dark");
  await page.getByRole("button", { name: "Save settings" }).click();
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
