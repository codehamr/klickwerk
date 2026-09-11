import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

test("startup download is accessible and skip gives control back immediately", async ({
  page,
}) => {
  const external: string[] = [];
  page.on("request", (request) => {
    if (
      request.url().startsWith("http") &&
      !request.url().startsWith("http://127.0.0.1:1420")
    )
      external.push(request.url());
  });
  await page.goto("/?update=downloading");
  await expect(
    page.getByRole("heading", { name: "Getting klickwerk ready" }),
  ).toBeVisible();
  await expect(
    page.getByRole("progressbar", { name: "Update download" }),
  ).toHaveAttribute("value", "50");
  await expect(page.getByLabel("What would you like me to do?")).toHaveCount(0);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await page.getByRole("button", { name: "Skip this time" }).click();
  await page
    .getByLabel("What would you like me to do?")
    .fill("Keep my unsent task");
  await expect(page.getByText("Update skipped", { exact: true })).toBeVisible();
  await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
    "Keep my unsent task",
  );
  expect(external).toEqual([]);
});

test("completed startup returns to the composer and explains the update", async ({
  page,
}) => {
  await page.goto("/?update=success");
  await expect(
    page.getByText("klickwerk is updated. You’re ready to go."),
  ).toBeVisible();
  await expect(page.getByLabel("What would you like me to do?")).toBeVisible();
  await expect(page.getByText("Up to date", { exact: true })).toBeVisible();
});

test("skipping during a check prevents a later simulated restart", async ({
  page,
}) => {
  await page.goto("/?update=success");
  await page.getByRole("button", { name: "Skip this time" }).click();
  await page
    .getByLabel("What would you like me to do?")
    .fill("Keep this draft");
  await page.waitForTimeout(1600);
  await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
    "Keep this draft",
  );
  await expect(
    page.getByText("klickwerk is updated. You’re ready to go."),
  ).toBeHidden();
});

test("offline startup stays usable and a later check preserves unsent text", async ({
  page,
}) => {
  await page.goto("/?update=error");
  await expect(
    page.getByText(
      "Updates could not be checked. You can keep working and try again later.",
    ),
  ).toBeVisible();
  await page
    .getByLabel("What would you like me to do?")
    .fill("My unsent prompt");
  await page.getByRole("button", { name: "Check for updates" }).click();
  await expect(
    page.getByText(
      "An update is available. It will install the next time you open klickwerk.",
    ),
  ).toBeVisible();
  await expect(page.getByLabel("What would you like me to do?")).toHaveValue(
    "My unsent prompt",
  );
});

test("installation cannot be dismissed midway", async ({ page }) => {
  await page.goto("/?update=installing");
  await expect(page.getByText("Restarting with your update…")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Skip this time" }),
  ).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(page.getByLabel("What would you like me to do?")).toHaveCount(0);
});

test("German update screen works in a narrow dark window", async ({
  browser,
}) => {
  const context = await browser.newContext({
    locale: "de-DE",
    colorScheme: "dark",
    viewport: { width: 390, height: 740 },
  });
  const page = await context.newPage();
  await page.goto("/?update=downloading");
  await expect(
    page.getByRole("heading", { name: "klickwerk macht sich bereit" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Diesmal überspringen" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await context.close();
});
