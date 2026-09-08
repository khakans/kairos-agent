import { test, expect, type APIRequestContext } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
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

test("Rust schedules repeated cycles without a page and stops during inference", async ({
  page,
}) => {
  test.setTimeout(120000);
  await page.goto("/");
  await page.getByLabel("Owner token").fill(token);
  await page.getByRole("button", { name: "Open command center" }).click();
  const api = page.request;
  let initial = await (await api.get("/api/v1/runtime")).json();
  await command(api, { action: "set_mode", mode: "DRY_RUN" });
  if (initial.session) {
    await command(api, { action: "stop_session" });
    await command(api, { action: "reset_session" });
  } else {
    await command(api, {
      action: "create_session",
      name: "Automatic cycle smoke",
      starting_balance: "1000",
      fee_bps: 10,
      slippage_bps: 20,
    });
  }
  await command(api, { action: "start_session" });
  await page.reload();
  await page.getByLabel("Interval unit", { exact: true }).selectOption("3600");
  await page.getByLabel("Cycle interval", { exact: true }).fill("2");
  await page.getByRole("button", { name: "Save interval" }).click();
  await expect
    .poll(
      async () =>
        (await (await api.get("/api/v1/runtime")).json()).cycle_schedule
          .interval_seconds,
    )
    .toBe(7200);
  await page.reload();
  await expect(page.getByLabel("Interval unit", { exact: true })).toHaveValue(
    "3600",
  );
  await expect(page.getByLabel("Cycle interval", { exact: true })).toHaveValue(
    "2",
  );
  await page.getByLabel("Interval unit", { exact: true }).selectOption("60");
  await page.getByLabel("Cycle interval", { exact: true }).fill("1");
  await page.getByRole("button", { name: "Save interval" }).click();
  await expect
    .poll(
      async () =>
        (await (await api.get("/api/v1/runtime")).json()).cycle_schedule
          .interval_seconds,
    )
    .toBe(60);
  initial = await (await api.get("/api/v1/runtime")).json();
  await page.getByRole("button", { name: "Start cycles", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Stop cycles", exact: true }),
  ).toBeVisible();
  await expect
    .poll(
      async () => (await (await api.get("/api/v1/runtime")).json()).cycle_count,
      { timeout: 12000 },
    )
    .toBe(initial.cycle_count + 1);
  await expect(
    page.getByText("Waiting for next cycle", { exact: true }),
  ).toBeVisible();
  await page.screenshot({
    path: "test-results/automatic-cycles-waiting.png",
    fullPage: true,
  });
  // No application JavaScript is loaded while the Rust timer starts cycle two.
  await page.goto("about:blank");
  await expect
    .poll(
      async () =>
        (await (await api.get("http://127.0.0.1:18080/api/v1/runtime")).json())
          .cycle_count,
      { timeout: 75000, intervals: [2000] },
    )
    .toBe(initial.cycle_count + 2);
  await page.goto("/");
  await page.getByRole("button", { name: "Stop cycles", exact: true }).click();
  await expect(
    page.getByText("Automatic cycles stopped", { exact: true }),
  ).toBeVisible();
  let stopped = await (await api.get("/api/v1/runtime")).json();
  expect(stopped.cycle_schedule.next_run_at).toBeNull();
  await api.post("http://127.0.0.1:18081/control", {
    data: { delay_ms: 3000 },
  });
  try {
    await page
      .getByRole("button", { name: "Start cycles", exact: true })
      .click();
    await expect(
      page.locator('.agent-node[data-status="Running"]'),
    ).toHaveCount(1, { timeout: 12000 });
    await page
      .getByRole("button", { name: "Stop cycles", exact: true })
      .click();
    await expect(
      page.getByText("Automatic cycles stopped", { exact: true }),
    ).toBeVisible({ timeout: 10000 });
    stopped = await (await api.get("/api/v1/runtime")).json();
    expect(stopped.cycle_schedule.next_run_at).toBeNull();
    expect(stopped.cycle_count).toBe(initial.cycle_count + 2);
    expect(
      stopped.agents.some((a: { status: string }) => a.status === "Running"),
    ).toBe(false);
  } finally {
    await api.post("http://127.0.0.1:18081/control", { data: { delay_ms: 0 } });
    await command(api, { action: "stop_session" });
  }
});

test("Pump.fun discovery uses Rust filters, reconnects and supplies four agents and daily logs", async ({
  page,
}) => {
  test.setTimeout(120000);
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await page.getByLabel("Owner token").fill(token);
  await page.getByRole("button", { name: "Open command center" }).click();
  const api = page.request;
  const state = async () => (await api.get("/api/v1/runtime")).json();
  const stats = async () =>
    (await api.get("http://127.0.0.1:18081/pump-stats")).json();
  await page.getByRole("button", { name: "Markets", exact: true }).click();
  await page.getByLabel("Maximum pair age (hours)").fill("6");
  await page.getByLabel("Minimum 1h volume (USD)").fill("20000");
  await page.getByRole("button", { name: "Save discovery filters" }).click();
  await expect
    .poll(async () => (await state()).pumpfun_config.max_pair_age_hours)
    .toBe(6);
  await page.getByRole("button", { name: "Enable discovery" }).click();
  try {
    await expect
      .poll(async () => (await state()).pumpfun.matching, { timeout: 25000 })
      .toBe(1);
    await expect(
      page.getByText("MATCHES FILTERS", { exact: true }),
    ).toHaveCount(1);
    await expect(page.getByText(/Mayhem-mode token excluded/)).toBeVisible();
    expect((await state()).pumpfun.tracked).toBe(2);
    expect(new Set((await stats()).subscriptions)).toEqual(
      new Set(["subscribeNewToken", "subscribeMigration"]),
    );
    const connected = (await stats()).connections;
    await page.reload();
    await page.getByRole("button", { name: "Markets", exact: true }).click();
    await expect(page.getByLabel("Maximum pair age (hours)")).toHaveValue("6");
    await expect(page.getByLabel("Minimum 1h volume (USD)")).toHaveValue(
      "20000",
    );
    expect((await stats()).connections).toBe(connected);

    await api.post("http://127.0.0.1:18081/pump-control", {
      data: { disconnect: true },
    });
    await expect
      .poll(async () => (await stats()).connections, { timeout: 15000 })
      .toBeGreaterThan(connected);
    await expect
      .poll(async () => (await state()).pumpfun.matching, { timeout: 20000 })
      .toBe(1);
    await page.screenshot({
      path: "test-results/pumpfun-desktop.png",
      fullPage: true,
    });
    await page.setViewportSize({ width: 390, height: 844 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBeTruthy();
    await page.screenshot({
      path: "test-results/pumpfun-mobile.png",
      fullPage: true,
    });

    let current = await state();
    expect(
      (await command(api, { action: "set_mode", mode: "DRY_RUN" })).ok(),
    ).toBeTruthy();
    if (current.session) {
      await command(api, { action: "stop_session" });
      await command(api, { action: "reset_session" });
    } else {
      await command(api, {
        action: "create_session",
        name: "Pump discovery smoke",
        starting_balance: "1000",
        fee_bps: 10,
        slippage_bps: 20,
      });
    }
    expect((await command(api, { action: "start_session" })).ok()).toBeTruthy();
    expect((await command(api, { action: "run_cycle" })).ok()).toBeTruthy();
    const contexts = await (
      await api.get("http://127.0.0.1:18081/contexts")
    ).json();
    for (const context of contexts.slice(-4)) {
      expect(context.pumpfun.candidates).toHaveLength(1);
      expect(context.pumpfun.candidates[0].symbol).toBe("SMOKEPUMP");
      expect(context.pumpfun.execution_supported).toBe(false);
      expect(context.pumpfun.excluded_candidates).toHaveLength(1);
      expect(context.pumpfun.excluded_candidates[0].matches_filters).toBe(
        false,
      );
    }
    current = await state();
    const root = current.readiness
      .find(
        (row: { capability: string }) =>
          row.capability === "Daily trade journal",
      )
      .detail.split(" / YYYY")[0];
    const paths = (await readdir(root, { recursive: true })).filter((path) =>
      path.endsWith(".json"),
    );
    const records = await Promise.all(
      paths.map(async (path) =>
        JSON.parse(await readFile(join(root, path), "utf8")),
      ),
    );
    expect(
      records.some(
        (record) =>
          record.event.kind === "agent.cycle.completed" &&
          record.decision?.pumpfun?.candidates?.[0]?.symbol === "SMOKEPUMP",
      ),
    ).toBeTruthy();
    expect(current.orders).toHaveLength(0);
    expect(
      (await command(api, { action: "set_mode", mode: "LIVE" })).ok(),
    ).toBeTruthy();
    current = await state();
    expect(current.pumpfun.matching).toBe(1);
    expect(current.capabilities.signing_enabled).toBe(false);
    await command(api, { action: "set_mode", mode: "DRY_RUN" });

    await api.post("http://127.0.0.1:18081/pump-control", {
      data: { metrics: "missing" },
    });
    await expect
      .poll(async () => (await state()).pumpfun.matching, { timeout: 25000 })
      .toBe(0);
    expect((await state()).pumpfun.candidates[0].pair.liquidity_usd).toBeNull();
    await api.post("http://127.0.0.1:18081/pump-control", {
      data: { metrics: "offline" },
    });
    await expect
      .poll(async () => (await state()).pumpfun.error, { timeout: 25000 })
      .toContain("DEX Screener");
    expect((await state()).pumpfun.matching).toBe(0);
    expect(errors).toEqual([]);
  } finally {
    const current = await state();
    await command(api, {
      action: "configure_pumpfun",
      config: { ...current.pumpfun_config, enabled: false },
    });
    await command(api, { action: "set_mode", mode: "DRY_RUN" });
    await command(api, { action: "stop_session" });
    await api.post("http://127.0.0.1:18081/pump-control", {
      data: { metrics: "healthy" },
    });
    await expect.poll(async () => (await stats()).active).toBe(0);
  }
});

test("desktop workflow: create, fill, close, reset, reload, switch modes and kill", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await page.getByLabel("Owner token").fill(token);
  await page.screenshot({
    path: "test-results/kairos-welcome.png",
    fullPage: true,
  });
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
  await page.getByRole("button", { name: "Run once", exact: true }).click();
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
  await page.getByRole("button", { name: "Run once", exact: true }).click();
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

test("lifecycle streams active stages, timer, reload recovery and failure", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByLabel("Owner token").fill(token);
  await page.getByRole("button", { name: "Open command center" }).click();
  const initial = await (await page.request.get("/api/v1/runtime")).json();
  await command(page.request, { action: "set_mode", mode: "DRY_RUN" });
  if (initial.session) {
    await command(page.request, { action: "stop_session" });
    await command(page.request, { action: "reset_session" });
  } else {
    await command(page.request, {
      action: "create_session",
      name: "Lifecycle smoke",
      starting_balance: "1000",
      fee_bps: 10,
      slippage_bps: 20,
    });
  }
  await command(page.request, { action: "start_session" });
  await page.reload();
  await page.request.post("http://127.0.0.1:18081/control", {
    data: { delay_ms: 2500 },
  });
  try {
    const progress = page.getByRole("region", { name: "Cycle progress" });
    await page.getByRole("button", { name: "Run once", exact: true }).click();
    await expect(
      page.getByRole("button", { name: /Running cycle/ }),
    ).toBeDisabled();
    await expect(
      page
        .locator('.agent-node[data-status="Running"]')
        .filter({ hasText: "Orchestrator" }),
    ).toBeVisible();
    await expect(progress.getByRole("status")).toHaveText("Planning the cycle");
    await expect(progress.getByLabel("Cycle elapsed time")).not.toHaveText(
      "0:00",
    );
    const started = Date.now();
    const state = await (await page.request.get("/api/v1/runtime")).json();
    expect(Date.now() - started).toBeLessThan(1000);
    expect(
      state.agents.some((a: { status: string }) => a.status === "Running"),
    ).toBeTruthy();
    await expect(
      page.locator('.agent-node[data-status="Running"]'),
    ).toHaveCount(2);
    await expect(progress.getByRole("status")).toHaveText(
      "Analyzing market & on-chain evidence",
    );
    await page.screenshot({
      path: "test-results/lifecycle-running-desktop.png",
      fullPage: true,
    });
    await page.reload();
    await expect(progress).toHaveClass(/is-active/);
    await expect(
      page.getByRole("button", { name: /Running cycle/ }),
    ).toBeDisabled();
    await page.emulateMedia({ reducedMotion: "reduce" });
    expect(
      await progress
        .locator(".spin")
        .first()
        .evaluate((element) => getComputedStyle(element).animationName),
    ).toBe("none");
    await expect(progress.getByRole("status")).toHaveText("Cycle complete", {
      timeout: 15000,
    });
    await expect(progress.getByRole("progressbar")).toHaveAttribute(
      "value",
      "4",
    );
    await expect(
      page.locator('.agent-node[data-status="Running"]'),
    ).toHaveCount(0);
    await page.setViewportSize({ width: 390, height: 844 });
    await page.request.post("http://127.0.0.1:18081/control", {
      data: { delay_ms: 1500, failed_role: "market_analyst" },
    });
    await page.getByRole("button", { name: "Run once", exact: true }).click();
    await expect(progress).toHaveClass(/is-active/);
    await page.screenshot({
      path: "test-results/lifecycle-running-mobile.png",
      fullPage: true,
    });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBeTruthy();
    await expect(progress.getByRole("status")).toHaveText(/Cycle stopped/, {
      timeout: 12000,
    });
    await expect(page.locator('.agent-node[data-status="Failed"]')).toHaveCount(
      1,
    );
    await expect(progress.locator(".spin")).toHaveCount(0);
  } finally {
    await page.request.post("http://127.0.0.1:18081/control", {
      data: { delay_ms: 0 },
    });
  }
});
