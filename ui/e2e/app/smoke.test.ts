// Full-stack smoke test: the real app (Rust backend, WebView, git) driven via
// `tauri-driver`. Runs where tauri-driver is supported (Windows in CI).
//
//   cargo install tauri-driver --locked
//   pnpm tauri build --debug --no-bundle
//   pnpm smoke
//
// Environment: BRANCHBOARD_APP (app binary), TAURI_DRIVER (default
// `tauri-driver`), NATIVE_DRIVER (path to msedgedriver, optional).

import { spawnSync } from "node:child_process";
import { appendFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import assert from "node:assert/strict";

import { Keys, Session } from "./webdriver.ts";
import { Driver } from "./driver.ts";

const DRIVER_URL = "http://127.0.0.1:4444";
const exe = process.platform === "win32" ? ".exe" : "";
const app =
  process.env.BRANCHBOARD_APP ?? join(process.cwd(), "target", "debug", `branchboard${exe}`);

let root: string | undefined;
let repo: string;
let driver: Driver | undefined;
let session: Session | undefined;

function git(cwd: string, ...args: string[]): string {
  const result = spawnSync(
    "git",
    [
      "-c",
      "user.name=Smoke",
      "-c",
      "user.email=smoke@branchboard.invalid",
      "-c",
      "commit.gpgsign=false",
      ...args,
    ],
    { cwd, encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  return result.stdout;
}

before(async () => {
  const diagnostics = join(process.cwd(), "e2e-results", "smoke");
  mkdirSync(diagnostics, { recursive: true });
  writeFileSync(join(diagnostics, "driver.log"), "");
  const log = (message: string) => {
    appendFileSync(join(diagnostics, "driver.log"), message);
    process.stdout.write(message);
  };
  writeFileSync(
    join(diagnostics, "startup.json"),
    JSON.stringify(
      {
        application: app,
        driver: process.env.TAURI_DRIVER ?? "tauri-driver",
        nativeDriver: process.env.NATIVE_DRIVER ?? null,
        platform: process.platform,
        node: process.version,
      },
      null,
      2,
    ),
  );
  root = mkdtempSync(join(tmpdir(), "branchboard-smoke-"));
  repo = join(root, "smoke-repo");
  mkdirSync(repo);
  git(repo, "init", "--quiet", "--initial-branch=main");
  git(repo, "config", "--local", "core.autocrlf", "false");
  git(repo, "config", "--local", "core.eol", "lf");
  writeFileSync(join(repo, "README.md"), "# smoke\n");
  git(repo, "add", ".");
  git(repo, "commit", "--quiet", "-m", "init");
  writeFileSync(join(repo, "hello.txt"), "hello\n");

  const workspace = join(root, "workspace.json");
  writeFileSync(
    workspace,
    JSON.stringify({
      version: 1,
      repos: [{ id: "smoke", path: repo }],
      groups: [],
      activeRepoId: "smoke",
    }),
  );

  const args = process.env.NATIVE_DRIVER ? ["--native-driver", process.env.NATIVE_DRIVER] : [];
  driver = new Driver(
    process.env.TAURI_DRIVER ?? "tauri-driver",
    args,
    { ...process.env, BRANCHBOARD_WORKSPACE_FILE: workspace },
    log,
  );
  try {
    await driver.waitReady(DRIVER_URL);
    session = await driver.guard(Session.start(DRIVER_URL, app));
  } catch (error) {
    log(
      `Smoke-test startup failed: ${error instanceof Error ? (error.stack ?? error.message) : String(error)}\n`,
    );
    throw error;
  }
});

after(async () => {
  await session?.end().catch(() => undefined);
  await driver?.stop();
  if (root) rmSync(root, { recursive: true, force: true });
});

void test("shows the repository with its branch and changes", async () => {
  assert.ok(session);
  const summary = await session.waitFor<string>(
    "the repository summary",
    `const row = document.querySelector('[data-repo-id="smoke"]');
     return row && row.textContent.includes("main") && row.querySelector('[title="Uncommitted changes"]') ? row.textContent : null;`,
  );
  assert.match(summary, /smoke-repo/);
  await session.waitFor(
    "the untracked file",
    `return document.querySelector('[data-key="unstaged:hello.txt"]') !== null;`,
  );
});

void test("stages a file in the real repository", async () => {
  assert.ok(session);
  await session.click('[data-key="unstaged:hello.txt"]');
  await session.type('[role="listbox"]', Keys.space);
  await session.waitFor(
    "the staged file",
    `return document.querySelector('[data-key="staged:hello.txt"]') !== null;`,
  );
  assert.equal(git(repo, "status", "--porcelain").trim(), "A  hello.txt");
});

void test("picks up changes made outside the app", async () => {
  assert.ok(session);
  writeFileSync(join(repo, "outside.txt"), "made in the terminal\n");
  await session.waitFor(
    "the external change",
    `return document.querySelector('[data-key="unstaged:outside.txt"]') !== null;`,
  );
});
