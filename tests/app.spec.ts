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

test("first launch has a simple prompt and an unobtrusive takeover hint", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    "What can I takeoff your hands?",
  );
  await expect(page.getByLabel("What would you like me to do?")).toBeFocused();
  await expect(
    page.getByRole("button", { name: "Let’s do it" }),
  ).toBeDisabled();
  await expect(
    page.getByText("Move your mouse or type to take over. Anytime."),
  ).toBeVisible();
  await expect(page.getByText(/Ctrl.*Alt.*F8/)).toHaveCount(0);
  await expect(page.getByRole("button", { name: /stop task/i })).toHaveCount(0);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("suggestions fill the task without starting it", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /Do the small things/ }).click();
  await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
    /18% of 245/,
  );
  await expect(page.getByLabel("What would you like me to do?")).toBeFocused();
  await page.getByRole("button", { name: "Let’s do it" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
});

test("one URL flow handles any server and automatically loads models", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByText("On this PC", { exact: true })).toHaveCount(0);
  await expect(page.getByText("Custom server", { exact: true })).toHaveCount(0);
  await page.getByLabel("Server URL").fill("localhost:1234");
  await page.getByLabel("Vision model", { exact: true }).click();
  await expect(page.getByText(/3 models found/)).toBeVisible();
  await page.getByLabel("Vision model", { exact: true }).fill("qwen3-vl:8b");
  await page.getByRole("button", { name: "Save settings" }).click();
  await page.reload();
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByLabel("Server URL")).toHaveValue(
    "http://localhost:1234/v1",
  );
  await page.getByLabel("Server URL").fill("example.org/proxy/v1");
  await page.getByLabel("Vision model", { exact: true }).click();
  await page.getByRole("button", { name: "Save settings" }).click();
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByLabel("Server URL")).toHaveValue(
    "https://example.org/proxy/v1",
  );
});

test("settings discard unsaved edits and return focus to the prompt", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await expect(page.getByLabel("What would you like me to do?")).toBeFocused();
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.getByLabel("Vision model", { exact: true }).fill("unsaved-model");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByLabel("Vision model", { exact: true })).toHaveValue(
    "qwen3-vl:8b",
  );
});

test("connection fixtures keep secrets out of browser storage", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.getByLabel("Vision model", { exact: true }).fill("qwen3-vl:8b");
  await page.locator("#api-key").fill("not-a-real-secret");
  await page.getByRole("button", { name: "Test connection" }).click();
  await expect(
    page.getByText("Preview connection looks good. No server was contacted."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Save settings" }).click();
  expect(
    await page.evaluate(() =>
      localStorage.getItem("klickwerk-preview-settings"),
    ),
  ).not.toContain("not-a-real-secret");
});

test("mouse movement cancels the countdown and invites a correction", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page, "Write Grüße 世界 in an editor");
  await expect(
    page.getByRole("button", { name: "Open settings" }),
  ).toBeDisabled();
  await takeOver(page);
  await expect(
    page.getByText("You took over. What should I do differently?"),
  ).toBeVisible();
  await expect(page.getByLabel("Your refinement")).toBeFocused();
  await page.waitForTimeout(2400);
  await expect(page.getByLabel("Your refinement")).toBeVisible();
  await expect(
    page.getByText("Write Grüße 世界 in an editor", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("region", { name: "Action history" }),
  ).toBeVisible();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("any keyboard key interrupts a running agent", async ({ page }) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await expect(
    page.getByText("Working on your desktop", { exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Shift");
  await expect(page.getByLabel("Your refinement")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Continue", exact: true }),
  ).toBeDisabled();
});

test("refining preserves actions and exact click coordinates across attempts", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await expect(
    page.getByText("Focus the document editor", { exact: true }),
  ).toBeVisible();
  await takeOver(page);
  await page.getByText("Click at (420, 280)", { exact: true }).click();
  await expect(page.getByText("Desktop: (630, 420)")).toBeVisible();
  await page
    .getByLabel("Your refinement")
    .fill("Use the top search field instead of the sidebar.");
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page.getByText("Starting in a moment")).toBeVisible();
  await expect(
    page.getByText("Your refinement", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Use the top search field instead of the sidebar.", {
      exact: true,
    }),
  ).toBeVisible();
  await takeOver(page);
  await expect(
    page.getByText("Focus the document editor", { exact: true }),
  ).toBeVisible();
});

test("completion clears the prompt and a new task removes the previous result", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await expect(page.getByText("Done. Back to you.")).toBeVisible();
  await expect(page.getByLabel("What would you like me to do?")).toBeEmpty();
  await expect(
    page.getByRole("region", { name: "Action history" }),
  ).toBeHidden();
  await page.getByRole("button", { name: "View actions" }).click();
  await page.getByText("Type text", { exact: true }).click();
  await expect(
    page.getByText("Welcome to the team!\nGrüße 世界", { exact: true }),
  ).toBeVisible();
  await page
    .getByLabel("What would you like me to do?")
    .fill("A completely new task");
  await expect(page.getByText("Done. Back to you.")).toBeHidden();
  await expect(
    page.getByRole("region", { name: "Action history" }),
  ).toBeHidden();
});

test("answers continue the same task with its earlier actions", async ({
  page,
}) => {
  await page.goto("/?scenario=ask");
  await connect(page);
  await start(page, "Create a document");
  await expect(page.getByLabel("Your answer")).toBeFocused();
  await page.getByLabel("Your answer").fill("Use my Documents folder");
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page.getByText("Starting in a moment")).toBeVisible();
  await expect(
    page.getByText("Use my Documents folder", { exact: true }),
  ).toBeVisible();
  await expect(page.getByText("Done. Back to you.")).toBeVisible();
});

test("errors preserve the task and offer connection recovery", async ({
  page,
}) => {
  await page.goto("/?scenario=error");
  await connect(page);
  await start(page, "Help with a task");
  await expect(
    page.getByRole("button", { name: "Check connection" }),
  ).toBeVisible();
  await expect(
    page.getByText("Help with a task", { exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Check connection" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
});

test("saved workflows include unsent corrections, editable memory, and a fresh start", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page, "Write Grüße 世界 in an editor");
  await takeOver(page);
  await page
    .getByLabel("Your refinement")
    .fill("Use Documents and leave the editor open.");
  await page.getByRole("button", { name: "Save as workflow" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByLabel("Start prompt")).toHaveValue(/Use Documents/);
  await dialog.getByLabel("Workflow name").fill("Welcome note");
  await dialog
    .getByLabel("Start prompt")
    .fill("Write a welcome note in Documents and leave it open for review.");
  await dialog
    .getByText("What the agent will remember", { exact: true })
    .click();
  await expect(dialog.getByLabel("Workflow instructions")).toHaveValue(
    /Use Documents/,
  );
  await dialog
    .getByLabel("Workflow instructions")
    .fill("Use Documents. Verify the selected folder before typing.");
  await dialog.getByText(/Included history/).click();
  await expect(
    dialog.getByText("Use Documents and leave the editor open.", {
      exact: true,
    }),
  ).toBeVisible();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await dialog
    .getByRole("button", { name: "Save workflow", exact: true })
    .click();
  await page.reload();
  await expect(
    page.getByRole("region", { name: "Action history" }),
  ).toBeHidden();
  await page.getByRole("button", { name: /Welcome note/ }).click();
  await expect(dialog.getByLabel("Start prompt")).toHaveValue(
    "Write a welcome note in Documents and leave it open for review.",
  );
  await dialog.getByRole("button", { name: "Use workflow" }).click();
  await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
    /Write a welcome note in Documents/,
  );
  await expect(page.getByText("Starting in a moment")).toBeHidden();
  await page.getByRole("button", { name: "Let’s do it" }).click();
  await takeOver(page);
  await page
    .getByLabel("Your refinement")
    .fill("Add a friendly closing sentence.");
  await page.getByRole("button", { name: "Save as workflow" }).click();
  await dialog.getByText(/Included history/).click();
  await expect(
    dialog.getByText("Use Documents and leave the editor open.", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    dialog.getByText("Add a friendly closing sentence.", { exact: true }),
  ).toBeVisible();
});

test("workflow editing can be cancelled and workflows can be removed", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await takeOver(page);
  await page.getByRole("button", { name: "Save as workflow" }).click();
  await page.getByLabel("Workflow name").fill("A reusable note");
  await page
    .getByRole("button", { name: "Save workflow", exact: true })
    .click();
  await page.getByRole("button", { name: "New task" }).click();
  await page.getByRole("button", { name: /A reusable note/ }).click();
  await page.getByLabel("Start prompt").fill("Unsaved changes");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: /A reusable note/ }).click();
  await expect(page.getByLabel("Start prompt")).not.toHaveValue(
    "Unsaved changes",
  );
  await page
    .getByRole("button", { name: "Delete workflow", exact: true })
    .click();
  await page.getByRole("button", { name: "Confirm delete workflow" }).click();
  await page.reload();
  await expect(
    page.getByRole("button", { name: /A reusable note/ }),
  ).toBeHidden();
});

test("saving another refinement updates the same workflow", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await start(page);
  await takeOver(page);
  await page.getByLabel("Your refinement").fill("Use Documents.");
  await page.getByRole("button", { name: "Save as workflow" }).click();
  await page.getByLabel("Workflow name").fill("My note");
  await page
    .getByRole("button", { name: "Save workflow", exact: true })
    .click();
  await expect(
    page.getByText("“My note” is saved in Your workflows."),
  ).toBeVisible();
  await page
    .getByLabel("Your refinement")
    .fill("Use Documents and add a greeting.");
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page.getByText("Starting in a moment")).toBeVisible();
  await takeOver(page);
  await page.getByRole("button", { name: "Save as workflow" }).click();
  await page.getByLabel("Workflow name").fill("My improved note");
  await page
    .getByRole("dialog")
    .getByText(/Included history/)
    .click();
  await expect(
    page.getByRole("dialog").getByText("Use Documents.", { exact: true }),
  ).toBeVisible();
  await expect(
    page
      .getByRole("dialog")
      .getByText("Use Documents and add a greeting.", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Save workflow", exact: true })
    .click();
  await page.getByRole("button", { name: "New task" }).click();
  await expect(page.locator(".workflow-card")).toHaveCount(1);
  await expect(
    page.getByRole("button", { name: /My improved note/ }),
  ).toBeVisible();
});

test("settings trap focus and remain accessible", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open settings" }).click();
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
});

test("light, dark, settings, narrow, and refinement layouts render without overflow", async ({
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
  await page.getByLabel("Reduce motion").click();
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
  await page.screenshot({
    path: "build/visual/narrow.png",
    fullPage: true,
    animations: "disabled",
  });
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
