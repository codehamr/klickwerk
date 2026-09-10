import { test, expect, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { mkdirSync } from "node:fs";

async function connect(page: Page) {
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.getByLabel("Vision model", { exact: true }).fill("qwen3-vl:8b");
  await page.getByRole("button", { name: "Save settings" }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
}

test("first launch is clear, keyboard accessible, and has no accessibility violations", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    "What can I takeoff your hands?",
  );
  await expect(page.getByLabel("What would you like me to do?")).toBeFocused();
  await expect(
    page.getByRole("button", { name: "Let's do it" }),
  ).toBeDisabled();
  await expect(
    page.getByText("Browser preview · all desktop actions are simulated"),
  ).toBeVisible();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("suggestions fill the task without starting it", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /Make it easier/ }).click();
  await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
    /18% of 245/,
  );
  await expect(page.getByLabel("What would you like me to do?")).toBeFocused();
  await expect(
    page.getByRole("region", { name: "Task progress" }),
  ).toBeHidden();
  await page.getByRole("button", { name: "Let's do it" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
});

test("settings save, reload, and return focus to the prompt", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await expect(page.getByLabel("What would you like me to do?")).toBeFocused();
  await page.reload();
  await expect(page.getByRole("button", { name: "qwen3-vl:8b" })).toBeVisible();
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByLabel("Vision model", { exact: true })).toHaveValue(
    "qwen3-vl:8b",
  );
  await page.getByLabel("Vision model", { exact: true }).fill("unsaved-model");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Open settings" }).click();
  await expect(page.getByLabel("Vision model", { exact: true })).toHaveValue(
    "qwen3-vl:8b",
  );
});

test("connection test uses preview fixtures and keeps secrets out of browser storage", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.getByRole("button", { name: "Load models" }).click();
  await expect(page.getByText(/3 models found/)).toBeVisible();
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

test("stop during countdown is final and preserves the task", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await page
    .getByLabel("What would you like me to do?")
    .fill("Write Grüße 世界 in an editor");
  await page.keyboard.press("Control+Enter");
  await expect(page.getByText("Your desktop is next")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open settings" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Stop task", exact: true }).click();
  await expect(page.getByText("Back in your hands")).toBeVisible();
  await page.waitForTimeout(3500);
  await expect(page.getByText("Back in your hands")).toBeVisible();
  await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
    "Write Grüße 世界 in an editor",
  );
});

test("preview shortcut stops while another control has focus", async ({
  page,
}) => {
  await page.goto("/");
  await connect(page);
  await page
    .getByLabel("What would you like me to do?")
    .fill("Open Calculator");
  await page.getByRole("button", { name: "Let's do it" }).click();
  await expect(page.getByText("On it. You can take a breather.")).toBeVisible();
  await page.getByRole("button", { name: "Stop task", exact: true }).focus();
  await page.keyboard.press("Control+Alt+F8");
  await expect(page.getByText("Back in your hands")).toBeVisible();
});

test("completion shows a result and can start a new task", async ({ page }) => {
  await page.goto("/");
  await connect(page);
  await page.getByLabel("What would you like me to do?").fill("Draft a note");
  await page.getByRole("button", { name: "Let's do it" }).click();
  await expect(page.getByText("All taken care of")).toBeVisible({
    timeout: 12_000,
  });
  await page.getByRole("button", { name: "2 steps" }).click();
  await expect(page.getByText("Checked the result")).toBeVisible();
  await page.getByRole("button", { name: "New task" }).click();
  await expect(page.getByLabel("What would you like me to do?")).toBeEmpty();
  await expect(page.getByText("A few things to try")).toBeVisible();
});

test("questions release control and continue with a new countdown", async ({
  page,
}) => {
  await page.goto("/?scenario=ask");
  await connect(page);
  await page
    .getByLabel("What would you like me to do?")
    .fill("Create a document");
  await page.getByRole("button", { name: "Let's do it" }).click();
  await expect(page.getByLabel("Your answer")).toBeVisible({ timeout: 12_000 });
  await expect(page.getByLabel("Your answer")).toBeFocused();
  await expect(
    page.getByRole("button", { name: "Stop task", exact: true }),
  ).toBeHidden();
  await page.getByLabel("Your answer").fill("Use my Documents folder");
  await page.getByRole("button", { name: "Continue", exact: true }).click();
  await expect(page.getByText("Your desktop is next")).toBeVisible();
  await expect(page.getByText("All taken care of")).toBeVisible({
    timeout: 12_000,
  });
});

test("errors offer recovery and never discard the task", async ({ page }) => {
  await page.goto("/?scenario=error");
  await connect(page);
  await page
    .getByLabel("What would you like me to do?")
    .fill("Help with a task");
  await page.getByRole("button", { name: "Let's do it" }).click();
  await expect(page.getByText("Let’s get this sorted")).toBeVisible({
    timeout: 12_000,
  });
  await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
    "Help with a task",
  );
  await page.getByRole("button", { name: "Check connection" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
});

test("safe stop test works without a configured model", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open settings" }).click();
  await page.getByRole("tab", { name: "Preferences" }).click();
  await page.getByRole("button", { name: "Try a safe stop test" }).click();
  await expect(page.getByText("Your desktop is next")).toBeVisible();
  await page.keyboard.press("Control+Alt+F8");
  await expect(page.getByText("Back in your hands")).toBeVisible();
});

test("settings dialog traps focus and is accessible", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Open settings" }).click();
  for (let i = 0; i < 18; i++) {
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

test("light, dark, settings, and narrow layouts render without overflow", async ({
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
  await page.getByRole("tab", { name: "Preferences" }).click();
  await page.getByLabel("Appearance").selectOption("dark");
  await page.getByLabel("Reduce motion").click();
  await page.getByRole("button", { name: "Save settings" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(page.locator("html")).toHaveAttribute(
    "data-reduce-motion",
    "true",
  );
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
});
