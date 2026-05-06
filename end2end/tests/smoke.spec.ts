import { test, expect } from "@playwright/test";
import * as fs from "fs";
import * as path from "path";
import * as yaml from "js-yaml";

// Path to the generated YAML file, accessible on disk because docker-compose
// bind-mounts ./dev/config into the container and the same directory is
// readable from the host (local) or the CI runner (which has checked out the
// repo alongside the running stack).
const YAML_PATH = path.join(
  __dirname,
  "../../dev/config/irrigation_unlimited.yaml",
);

interface IuZone {
  zone_id: string;
  name: string;
  entity_id: string;
}

interface IuSchedule {
  name: string;
  time: string;
  weekday?: string[];
}

interface IuSequence {
  name: string;
  sequence_id: string;
  schedules: IuSchedule[];
  zones: { zone_id: string; duration: string }[];
}

interface IuController {
  name: string;
  zones: IuZone[];
  sequences: IuSequence[];
}

interface IuConfig {
  controllers: IuController[];
}

test.describe("Config page smoke test", () => {
  const MORNING_TIME = "06:30";
  const AFTERNOON_TIME = "14:00";
  // zone_1 morning duration in MM:SS
  const ZONE1_MORNING_DURATION = "05:00";

  test("save config, verify UI round-trip and generated YAML", async ({
    page,
  }) => {
    // ── 1. Select schedule days so sequences are exported to YAML ────────
    await page.goto("/");

    // Wait for the day matrix — confirms SSR + WASM hydration is complete
    const matrix = page.locator(".zone-day-matrix");
    await expect(matrix).toBeVisible({ timeout: 30_000 });

    // Set a known state: only Monday and Wednesday ticked, all others unticked
    const ENABLED_DAYS = ["Monday", "Wednesday"];
    const ALL_DAYS = [
      "Monday",
      "Tuesday",
      "Wednesday",
      "Thursday",
      "Friday",
      "Saturday",
      "Sunday",
    ];
    for (const day of ALL_DAYS) {
      const headerCheckbox = page
        .locator(`label[title="Toggle all zones for ${day}"]`)
        .locator('input[type="checkbox"]');
      if (ENABLED_DAYS.includes(day)) {
        if (!(await headerCheckbox.isChecked())) {
          await headerCheckbox.check();
        }
      } else {
        if (await headerCheckbox.isChecked()) {
          await headerCheckbox.uncheck();
        }
      }
    }

    await page.click(".form-actions button.btn--primary");
    const scheduleSuccess = page.locator("p.success");
    await expect(scheduleSuccess).toBeVisible({ timeout: 15_000 });

    // ── 2. Load the config page ───────────────────────────────────────────
    await page.goto("/config");

    // Wait for the config form — confirms SSR + WASM hydration is complete
    const form = page.locator(".config-form");
    await expect(form).toBeVisible({ timeout: 30_000 });

    // ── 3. Fill in session times ──────────────────────────────────────────
    await page.fill("#morning-time", MORNING_TIME);
    await page.fill("#afternoon-time", AFTERNOON_TIME);

    // ── 4. Set zone_1 morning duration ────────────────────────────────────
    // First zone-table row; first duration input within it is the morning one.
    const firstRow = page.locator(".zone-table__row").first();
    const morningDurationInput = firstRow
      .locator('input[placeholder="MM:SS"]')
      .first();
    await morningDurationInput.fill(ZONE1_MORNING_DURATION);

    // ── 5. Save ───────────────────────────────────────────────────────────
    await page.click("button.btn--primary");

    // Wait for the success message to appear
    const successMsg = page.locator("p.success");
    await expect(successMsg).toBeVisible({ timeout: 15_000 });
    await expect(successMsg).toContainText("Configuration saved");

    // ── 6. UI round-trip: reload and assert values persisted ──────────────
    await page.reload();
    await expect(page.locator(".config-form")).toBeVisible({ timeout: 30_000 });

    await expect(page.locator("#morning-time")).toHaveValue(MORNING_TIME);
    await expect(page.locator("#afternoon-time")).toHaveValue(AFTERNOON_TIME);

    const reloadedFirstRow = page.locator(".zone-table__row").first();
    await expect(
      reloadedFirstRow.locator('input[placeholder="MM:SS"]').first(),
    ).toHaveValue(ZONE1_MORNING_DURATION);

    // ── 7. File check: assert generated irrigation_unlimited.yaml ─────────
    // The YAML is written synchronously by the server on save, so by the time
    // the reload above has finished the file is guaranteed to be up to date.
    const raw = fs.readFileSync(YAML_PATH, "utf-8");
    const config = yaml.load(raw) as IuConfig;

    // Top-level structure
    expect(config).toBeTruthy();
    expect(Array.isArray(config.controllers)).toBe(true);
    expect(config.controllers.length).toBeGreaterThan(0);

    const controller = config.controllers[0];

    if (controller === undefined) {
      throw new Error("Could not find controller element in config file.");
    }

    // All 8 zones are present in the zone definitions
    expect(controller.zones).toHaveLength(8);

    // At least one sequence schedule carries the morning time we just saved
    const allScheduleTimes = controller.sequences
      .flatMap((seq) => seq.schedules ?? [])
      .map((s) => s.time);

    expect(allScheduleTimes).toContain(MORNING_TIME);
  });
});
