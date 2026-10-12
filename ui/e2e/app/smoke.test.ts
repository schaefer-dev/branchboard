// Full-stack smoke test: the real app (Rust backend, WebView, git) driven via
// `tauri-driver`. Runs where tauri-driver is supported (Windows in CI).
//
//   cargo install tauri-driver --locked
//   pnpm tauri build --debug --no-bundle
//   pnpm smoke
//
// Environment: BRANCHBOARD_APP (app binary), TAURI_DRIVER (default
// `tauri-driver`), NATIVE_DRIVER (path to msedgedriver, optional).

import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import assert from "node:assert/strict";

import { Keys, Session } from "./webdriver.ts";

const DRIVER_URL = "http://127.0.0.1:4444";
const exe = process.platform === "win32" ? ".exe" : "";
const app =
  process.env.BRANCHBOARD_APP ?? join(process.cwd(), "target", "debug", `branchboard${exe}`);

let root: string;
let repo: string;
let driver: ChildProcess | undefined;
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

async function waitForDriver(): Promise<void> {
  for (let attempt = 0; attempt < 50; attempt++) {
    try {
      await fetch(`${DRIVER_URL}/status`);
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
  }
  throw new Error("tauri-driver did not start");
}

before(async () => {
  root = mkdtempSync(join(tmpdir(), "branchboard-smoke-"));
  repo = join(root, "smoke-repo");
  mkdirSync(repo);
  git(repo, "init", "--quiet", "--initial-branch=main");
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
  driver = spawn(process.env.TAURI_DRIVER ?? "tauri-driver", args, {
    stdio: "inherit",
    env: { ...process.env, BRANCHBOARD_WORKSPACE_FILE: workspace },
  });
  await waitForDriver();
  session = await Session.start(DRIVER_URL, app);
});

after(async () => {
  await session?.end().catch(() => undefined);
  driver?.kill();
  rmSync(root, { recursive: true, force: true });
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
