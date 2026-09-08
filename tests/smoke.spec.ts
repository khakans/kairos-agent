import { test, expect, type APIRequestContext } from "@playwright/test";
import { randomUUID } from "node:crypto";
const token = "smoke-only-owner-token-not-for-production";
const headers = { "x-kairos-command": "1" };
async function login(request: APIRequestContext) {
  expect(
    (
      await request.post("/api/v1/auth/login", { headers, data: { token } })
    ).ok(),
  ).toBeTruthy();
}
async function command(
  request: APIRequestContext,
  action: Record<string, unknown>,
) {
  return request.post("/api/v1/commands", {
    headers,
    data: { request_id: randomUUID(), ...action },
  });
}

test("authentication, CSRF, malformed command and live guards", async ({
  request,
}) => {
  expect((await request.get("/api/v1/runtime")).status()).toBe(401);
  expect((await request.get("/health/live")).ok()).toBeTruthy();
  await login(request);
  expect(
    (
      await request.post("/api/v1/commands", {
        data: { request_id: randomUUID(), action: "kill" },
      })
    ).status(),
  ).toBe(403);
  expect(
    (
      await request.post("/api/v1/commands", {
        headers: { ...headers, Origin: "https://untrusted.example" },
        data: { request_id: randomUUID(), action: "kill" },
      })
    ).status(),
  ).toBe(403);
  expect(
    (
      await command(request, {
        action: "set_mode",
        mode: "DRY_RUN",
        enable_signing: true,
      })
    ).status(),
  ).toBe(422);
  expect(
    (
      await command(request, { action: "sign_transaction", bytes: "arbitrary" })
    ).status(),
  ).toBe(422);
  expect(
    (await command(request, { action: "set_mode", mode: "LIVE" })).ok(),
  ).toBeTruthy();
  expect(
    (
      await command(request, {
        action: "activate_live",
        confirmation: "ACTIVATE LIVE",
      })
    ).status(),
  ).toBe(422);
  const state = await (await request.get("/api/v1/runtime")).json();
  expect(state.capabilities.signing_enabled).toBe(false);
  expect(Number(state.portfolio.equity)).toBe(0);
  await command(request, { action: "set_mode", mode: "DRY_RUN" });
});

test("desktop workflow: create, fill, close, reset, reload, switch modes and kill", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await page.getByLabel("Owner token").fill(token);
  await page.getByRole("button", { name: "Open command center" }).click();
  await expect(
    page.getByRole("heading", { name: "Clarity before execution." }),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/overview-desktop.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: "New dry run" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByLabel("Session name").fill("Browser smoke session");
  await page
    .getByRole("button", { name: "Create session", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page
    .getByRole("button", { name: "Start session", exact: true })
    .click();
  await page.getByRole("button", { name: "Run cycle", exact: true }).click();
  for (const name of [
    "Orchestrator",
    "Market Analyst",
    "On-chain Analyst",
    "Strategy & Evaluation",
  ]) {
    await expect(
      page.getByRole("button", { name: new RegExp(name.replace("&", "\\&")) }),
    ).toBeVisible();
  }
  await expect(page.getByText("4 AGENTS", { exact: true })).toBeVisible();
  await page
    .getByRole("button", { name: "Approve simulation", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Close position", exact: true }),
  ).toBeVisible();
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({
    path: "test-results/overview-with-position.png",
    fullPage: true,
  });
  // The next real HTTP model request must contain the prior journaled fill.
  await page.getByRole("button", { name: "Run cycle", exact: true }).click();
  await expect(page.getByText(/SIMULATED CLOSE POSITION/)).toBeVisible();
  const contexts = await (
    await page.request.get("http://127.0.0.1:18081/contexts")
  ).json();
  const latest = contexts.at(-1);
  expect(
    contexts
      .slice(-4)
      .map((context: { agent_role: string }) => context.agent_role)
      .sort(),
  ).toEqual([
    "market_analyst",
    "onchain_analyst",
    "orchestrator",
    "strategy_evaluation",
  ]);
  expect(Object.keys(latest.agent_reports)).toHaveLength(3);
  expect(latest.onchain.mints).toHaveLength(2);
  expect(
    latest.trade_history.some(
      (record: { event: { kind: string } }) =>
        record.event.kind === "execution.virtual_filled",
    ),
  ).toBeTruthy();
  expect(latest.open_positions).toHaveLength(1);
  await page
    .getByRole("button", { name: "Approve simulation", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Close position", exact: true }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "Orders", exact: true }).click();
  await expect(page.getByText("SIMULATED · NO SIGNATURE")).toHaveCount(2);
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await page.getByRole("button", { name: "Stop session", exact: true }).click();
  await page.getByRole("button", { name: "Reset as new session" }).click();
  await page.reload();
  await expect(
    page.getByRole("heading", { name: "Clarity before execution." }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Live", exact: true }).click();
  await expect(
    page.getByText("LIVE · REAL FUNDS · OWNER AUTHORIZATION REQUIRED"),
  ).toBeVisible();
  await expect(page.getByText("Jupiter live market").first()).toBeVisible();
  await page.getByRole("button", { name: "Wallets", exact: true }).click();
  await expect(
    page.getByText("Give your runtime a dedicated wallet."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Dry Run", exact: true }).click();
  await page.getByRole("button", { name: "Activate kill switch" }).click();
  await expect(
    page.getByRole("button", { name: "Activate kill switch" }),
  ).toBeDisabled();
  expect(errors).toEqual([]);
});

test("mobile navigation, pages, keyboard dialog and responsive layout", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.getByLabel("Owner token").fill(token);
  await page.getByRole("button", { name: "Open command center" }).click();
  await expect(
    page.getByRole("heading", { name: "Clarity before execution." }),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/overview-mobile.png",
    fullPage: true,
  });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBeTruthy();
  for (const name of [
    "Agent Field",
    "Markets",
    "Signals",
    "Portfolio",
    "Positions",
    "Orders",
    "Risk",
    "Adapters",
    "Wallets",
    "Notifications",
    "Audit Log",
    "Settings",
  ]) {
    await page.getByRole("button", { name: "Open menu", exact: true }).click();
    await page.getByRole("button", { name, exact: true }).click();
    await expect(
      page.getByRole("heading", { name, exact: true, level: 1 }),
    ).toBeVisible();
  }
  await page.getByRole("button", { name: "Open menu", exact: true }).click();
  await page.getByRole("button", { name: "Wallets", exact: true }).click();
  await page.getByRole("button", { name: "Configure live wallet" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
});
