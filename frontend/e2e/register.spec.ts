import { expect, test } from "@playwright/test";

test("a new account can verify email and sign in", async ({ page, request }) => {
  const email = `e2e-${Date.now()}@example.com`;
  const username = `e2e-${Date.now()}`;
  const password = "correct-horse-battery";

  await page.goto("/sign-up");
  await page.getByLabel("Username").fill(username);
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password", { exact: true }).fill(password);
  await page.getByLabel("Confirm password").fill(password);
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page.getByRole("heading", { name: "Verify your email" })).toBeVisible();

  const mailbox = await request.get(`http://127.0.0.1:8080/api/v1/dev/mailbox?email=${encodeURIComponent(email)}`);
  expect(mailbox.ok()).toBeTruthy();
  const body = (await mailbox.json()) as { messages: { template: string; text: string }[] };
  const text = body.messages.find((message) => message.template === "verify-email")?.text ?? "";
  const token = text.split("token=")[1]?.split(/\s/)[0]?.replace(/[^A-Za-z0-9_-].*$/, "");
  expect(token).toBeTruthy();

  await page.goto(`/verify-email?token=${token}`);
  await expect(page.getByText("Email verified. You can sign in.")).toBeVisible();

  await page.goto("/sign-in");
  await page.getByLabel("Email or username").fill(username);
  await page.getByLabel("Password", { exact: true }).fill(password);
  await page.getByRole("button", { name: "Continue" }).click();
  await expect(page.getByRole("heading", { name: "Account" })).toBeVisible();
  await expect(page.getByRole("main").getByText(email, { exact: true })).toBeVisible();

  await page.reload();
  await expect(page.getByRole("heading", { name: "Account" })).toBeVisible();
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Account" })).toBeVisible();
  await page.goto("/sign-in");
  await expect(page.getByRole("heading", { name: "Account" })).toBeVisible();
});
