import assert from "node:assert/strict";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { availablePort, cleanup, waitDebuggingReady } from "./attach.ts";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { randomUUID } from "node:crypto";
import { test, type TestContext } from "node:test";

import { Driver } from "./driver.ts";
import { WindowsWebView2, matchesProfile, type WindowsCommand } from "./windows.ts";
import { Session } from "./webdriver.ts";

async function server(
  t: TestContext,
  handler: (request: IncomingMessage, response: ServerResponse) => void,
): Promise<string> {
  const http = createServer(handler);
  await new Promise<void>((resolve) => http.listen(0, "127.0.0.1", resolve));
  t.after(
    () =>
      new Promise<void>((resolve, reject) => {
        http.closeAllConnections();
        http.close((error) => {
          if (error) reject(error);
          else resolve();
        });
      }),
  );
  const address = http.address();
  assert.ok(address && typeof address !== "string");
  return `http://127.0.0.1:${String(address.port)}`;
}

function driver(t: TestContext, code = "setInterval(() => {}, 1000)"): Driver {
  const process = new Driver(globalThis.process.execPath, ["-e", code]);
  t.after(() => process.stop());
  return process;
}

const MOCK_BASE = "http://mock-driver";
const MOCK_TIMEOUT_MS = 10_000;

/** Exhaust the fixtures with a timeout at the deadline, without racing real HTTP or timers. */
function responsesThenTimeout(
  t: TestContext,
  responses: readonly { path: string; body: string; status?: number }[],
): () => void {
  t.mock.timers.enable({ apis: ["Date"], now: 0 });
  let attempts = 0;
  t.mock.method(globalThis, "fetch", (input: string | URL | Request) => {
    const response = responses[attempts++];
    if (response) {
      assert.equal(input, `${MOCK_BASE}${response.path}`);
      return Promise.resolve(new Response(response.body, { status: response.status ?? 200 }));
    }
    t.mock.timers.setTime(MOCK_TIMEOUT_MS);
    return Promise.reject(
      new DOMException("The operation was aborted due to timeout", "TimeoutError"),
    );
  });
  return () => {
    assert.equal(attempts, responses.length + 1);
  };
}

void test("readiness waits for a successful ready response", async (t) => {
  let attempts = 0;
  const base = await server(t, (_request, response) => {
    attempts++;
    if (attempts === 1) {
      response.writeHead(503).end(JSON.stringify({ value: { ready: true } }));
    } else if (attempts === 2) {
      response.end("not JSON");
    } else {
      response.end(JSON.stringify({ value: { ready: attempts > 3 } }));
    }
  });
  await driver(t).waitReady(base, { timeoutMs: 2_000, pollMs: 10 });
  assert.equal(attempts, 4);
});

for (const [name, status, body, message] of [
  ["HTTP error", 500, "native driver unavailable", /HTTP 500: native driver unavailable/],
  ["malformed response", 200, "not JSON", /invalid JSON \(HTTP 200\): not JSON/],
  ["not ready", 200, '{"value":{"ready":false}}', /driver not ready/],
] as const) {
  void test(`readiness retains ${name} after a later request timeout`, async (t) => {
    const consumed = responsesThenTimeout(t, [{ path: "/status", status, body }]);
    await assert.rejects(
      driver(t).waitReady(MOCK_BASE, { timeoutMs: MOCK_TIMEOUT_MS, pollMs: 0 }),
      (error: Error) => {
        assert.match(error.message, message);
        assert.match(error.message, /The operation was aborted due to timeout/);
        return true;
      },
    );
    consumed();
  });
}

void test("readiness retains the latest response diagnostic", async (t) => {
  const consumed = responsesThenTimeout(t, [
    { path: "/status", status: 500, body: "old failure" },
    { path: "/status", status: 503, body: "current failure" },
  ]);
  await assert.rejects(
    driver(t).waitReady(MOCK_BASE, { timeoutMs: MOCK_TIMEOUT_MS, pollMs: 0 }),
    (error: Error) => {
      assert.match(error.message, /HTTP 503: current failure/);
      assert.doesNotMatch(error.message, /old failure/);
      return true;
    },
  );
  consumed();
});

void test("readiness reports transport-only failures without a response diagnostic", async (t) => {
  const consumed = responsesThenTimeout(t, []);
  await assert.rejects(
    driver(t).waitReady(MOCK_BASE, { timeoutMs: MOCK_TIMEOUT_MS, pollMs: 0 }),
    (error: Error) => {
      assert.equal(
        error.message,
        `Driver readiness timed out after ${String(MOCK_TIMEOUT_MS)}ms: The operation was aborted due to timeout`,
      );
      return true;
    },
  );
  consumed();
});

void test("readiness bounds stalled HTTP requests", async (t) => {
  const base = await server(t, () => undefined);
  await assert.rejects(
    driver(t).waitReady(base, {
      timeoutMs: 150,
      requestTimeoutMs: 30,
      pollMs: 10,
    }),
    /readiness timed out/,
  );
});

void test("a missing driver fails with its spawn error", async (t) => {
  const process = new Driver(join(tmpdir(), `missing-driver-${randomUUID()}`), []);
  t.after(() => process.stop());
  await assert.rejects(process.waitReady("http://127.0.0.1:1"), /Could not start.*ENOENT/);
});

void test("early driver exit fails without waiting for readiness timeout", async (t) => {
  const base = await server(t, (_request, response) => response.end('{"value":{"ready":false}}'));
  await assert.rejects(driver(t, "process.exit(7)").waitReady(base), /exited \(code=7/);
});

void test("a driver exit also interrupts session creation", async (t) => {
  const base = await server(t, () => undefined);
  const process = driver(t, "setTimeout(() => process.exit(8), 50)");
  await assert.rejects(process.guard(Session.start(base, "app.exe", 150)), /exited \(code=8/);
});

void test("session creation preserves the native WebDriver error", async (t) => {
  const base = await server(t, (_request, response) => {
    response.writeHead(500).end(
      JSON.stringify({
        value: {
          error: "session not created",
          message: "DevToolsActivePort file doesn't exist",
        },
      }),
    );
  });
  await assert.rejects(Session.start(base, "app.exe"), /session not created.*DevToolsActivePort/);
});

void test("session creation has a bounded request timeout", async (t) => {
  const base = await server(t, () => undefined);
  await assert.rejects(Session.start(base, "app.exe", 50), /timed out after 50ms/);
});

void test("malformed session responses include HTTP and body context", async (t) => {
  const base = await server(t, (_request, response) =>
    response.writeHead(502).end("proxy unavailable"),
  );
  await assert.rejects(
    Session.start(base, "app.exe"),
    /invalid JSON \(HTTP 502\): proxy unavailable/,
  );
});

void test("session requests retain their capabilities and response handling", async (t) => {
  const base = await server(t, (request, response) => {
    if (request.url === "/session") {
      let body = "";
      request.on("data", (data: Buffer) => {
        body += data.toString();
      });
      request.on("end", () => {
        assert.deepEqual(JSON.parse(body), {
          capabilities: {
            alwaysMatch: {
              browserName: "wry",
              "tauri:options": { application: "app.exe" },
            },
          },
        });
        response.end('{"value":{"sessionId":"test-session"}}');
      });
    } else {
      response.end('{"value":"repository loaded"}');
    }
  });
  const session = await Session.start(base, "app.exe");
  assert.equal(session.id, "test-session");
  assert.equal(await session.execute("return document.title"), "repository loaded");
  await session.end();
});

void test("stop waits for the driver to close", async (t) => {
  const process = driver(t);
  await process.stop();
  assert.ok(process.child.exitCode !== null || process.child.signalCode !== null);
});

void test("attach sends only WebView2 attach capabilities", async (t) => {
  const base = await server(t, (request, response) => {
    let body = "";
    request.on("data", (data: Buffer) => {
      body += data.toString();
    });
    request.on("end", () => {
      assert.deepEqual(JSON.parse(body), {
        capabilities: {
          alwaysMatch: {
            browserName: "webview2",
            "ms:edgeChromium": true,
            "ms:edgeOptions": { debuggerAddress: "127.0.0.1:9222" },
          },
        },
      });
      response.end('{"value":{"sessionId":"attached"}}');
    });
  });
  assert.equal((await Session.attach(base, "127.0.0.1:9222")).id, "attached");
});

const version = JSON.stringify({
  Browser: "Edg/153",
  webSocketDebuggerUrl: "ws://127.0.0.1/browser",
});
const pages = JSON.stringify([
  { id: "page", type: "page", webSocketDebuggerUrl: "ws://127.0.0.1/page" },
]);

void test("debugging readiness waits for both version and a page target", async (t) => {
  let attempts = 0;
  const records: string[] = [];
  const base = await server(t, (request, response) => {
    if (request.url === "/json/version") {
      attempts++;
      response.end(attempts === 1 ? "not JSON" : version);
    } else response.end(attempts < 4 ? "[]" : pages);
  });
  await waitDebuggingReady(driver(t), base, { timeoutMs: 2_000, pollMs: 10 }, (path) => {
    records.push(path);
  });
  assert.equal(attempts, 4);
  assert.ok(records.includes("version") && records.includes("list"));
});

for (const [name, responses, expected] of [
  ["malformed version", [{ path: "/json/version", body: "not JSON" }], /version: malformed JSON/],
  ["incomplete version", [{ path: "/json/version", body: "{}" }], /version: missing Browser/],
  [
    "malformed targets",
    [
      { path: "/json/version", body: version },
      { path: "/json/list", body: "not JSON" },
    ],
    /list: malformed JSON/,
  ],
  [
    "missing page",
    [
      { path: "/json/version", body: version },
      { path: "/json/list", body: '[{"type":"service_worker"}]' },
    ],
    /no valid page target/,
  ],
  [
    "incomplete page",
    [
      { path: "/json/version", body: version },
      { path: "/json/list", body: '[{"type":"page"}]' },
    ],
    /no valid page target/,
  ],
] as const) {
  void test(`debugging readiness retains ${name} after a later request timeout`, async (t) => {
    const consumed = responsesThenTimeout(t, responses);
    await assert.rejects(
      waitDebuggingReady(driver(t), MOCK_BASE, { timeoutMs: MOCK_TIMEOUT_MS, pollMs: 0 }),
      (error: Error) => {
        assert.match(error.message, expected);
        assert.match(error.message, /The operation was aborted due to timeout/);
        return true;
      },
    );
    consumed();
  });
}

void test("debugging readiness bounds stalled requests", async (t) => {
  const base = await server(t, () => undefined);
  await assert.rejects(
    waitDebuggingReady(driver(t), base, { timeoutMs: 150, requestTimeoutMs: 30, pollMs: 10 }),
    /debugging endpoint readiness timed out/,
  );
});

void test("application exit interrupts debugging readiness immediately", async (t) => {
  const base = await server(t, () => undefined);
  await assert.rejects(
    waitDebuggingReady(driver(t, "process.exit(9)"), base, { timeoutMs: 2_000 }),
    /exited \(code=9/,
  );
});

void test("application exit interrupts attachment", async (t) => {
  const base = await server(t, () => undefined);
  await assert.rejects(
    driver(t, "setTimeout(() => process.exit(10), 50)").guard(
      Session.attach(base, "127.0.0.1:9222", 150),
    ),
    /exited \(code=10/,
  );
});

void test("failed attachment still closes both processes and removes the profile", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "attach-cleanup-"));
  t.after(() => {
    rmSync(root, { recursive: true, force: true });
  });
  const app = driver(t);
  const webdriver = driver(t);
  const base = await server(t, (_request, response) =>
    response.writeHead(500).end('{"value":{"error":"session not created"}}'),
  );
  await assert.rejects(
    app.guard(webdriver.guard(Session.attach(base, "127.0.0.1:9222"))),
    /session not created/,
  );
  await cleanup(undefined, [webdriver, app], root);
  assert.ok(app.child.exitCode !== null || app.child.signalCode !== null);
  assert.ok(webdriver.child.exitCode !== null || webdriver.child.signalCode !== null);
  assert.equal(existsSync(root), false);
});

void test("cleanup tries every process and retains data if termination fails", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "attach-retained-"));
  t.after(() => {
    rmSync(root, { recursive: true, force: true });
  });
  const app = driver(t);
  const webdriver = driver(t);
  const stop = app.stop.bind(app);
  t.mock.method(app, "stop", () => Promise.reject(new Error("termination failed")));
  await assert.rejects(
    cleanup(undefined, [app, webdriver], root),
    /temporary test data was retained/,
  );
  assert.ok(webdriver.child.exitCode !== null || webdriver.child.signalCode !== null);
  assert.equal(existsSync(root), true);
  t.mock.restoreAll();
  await stop();
});

void test("driver exit interrupts attachment", async (t) => {
  const base = await server(t, () => undefined);
  const app = driver(t);
  const webdriver = driver(t, "setTimeout(() => process.exit(11), 50)");
  await assert.rejects(
    app.guard(webdriver.guard(Session.attach(base, "127.0.0.1:9222", 150))),
    /exited \(code=11/,
  );
});

void test("attachment has a bounded timeout", async (t) => {
  const base = await server(t, () => undefined);
  await assert.rejects(Session.attach(base, "127.0.0.1:9222", 50), /timed out after 50ms/);
});

void test("allocated debugging port can be bound on loopback", async (t) => {
  const port = await availablePort();
  const http = createServer();
  await new Promise<void>((resolve, reject) => {
    http.once("error", reject);
    http.listen(port, "127.0.0.1", resolve);
  });
  t.after(
    () =>
      new Promise<void>((resolve) => {
        http.close(() => {
          resolve();
        });
      }),
  );
});

const windowsProfile = String.raw`C:\Smoke Test\unique-profile`;
const browserArguments = "--remote-debugging-port=9222 --remote-debugging-address=127.0.0.1";

function windowsHarness(elevated = true, run?: WindowsCommand) {
  const calls: { action: string; payload: Record<string, unknown>; timeoutMs?: number }[] = [];
  const events: { event: string; data: unknown }[] = [];
  const snapshot = {
    elevated,
    values: [
      {
        path: "AdditionalBrowserArguments",
        exists: true,
        kind: "ExpandString",
        value: "original arguments",
      },
      { path: "UserDataFolder", exists: false, kind: null, value: null },
    ],
    createdKeys: ["UserDataFolder"],
  };
  const scope = new WindowsWebView2(
    String.raw`D:\app\branchboard.exe`,
    windowsProfile,
    browserArguments,
    (event, data) => {
      events.push({ event, data });
    },
    (action, payload, timeoutMs) => {
      calls.push({ action, payload, timeoutMs });
      if (run) return run(action, payload, timeoutMs);
      if (action === "snapshot") return snapshot;
      if (action === "processes") return [];
      return undefined;
    },
  );
  return { scope, calls, events, snapshot };
}

void test("elevated WebView2 uses scoped policies and restores original values and types", () => {
  const { scope, calls, snapshot } = windowsHarness();
  const env = {
    PATH: "original-path",
    WEBVIEW2_USER_DATA_FOLDER: "old-profile",
    webview2_additional_browser_arguments: "old-arguments",
  };
  assert.deepEqual(scope.prepare(env), { PATH: "original-path" });
  assert.equal(env.WEBVIEW2_USER_DATA_FOLDER, "old-profile");
  assert.deepEqual(
    calls.map((call) => call.action),
    ["snapshot", "configure"],
  );
  assert.deepEqual(calls[1]?.payload, {
    application: "branchboard.exe",
    profile: windowsProfile,
    browserArguments,
  });
  scope.restore();
  assert.deepEqual(calls[2]?.payload.snapshot, snapshot);
  scope.restore();
  assert.equal(calls.length, 3);
});

void test("non-elevated WebView2 keeps child-only environment configuration", () => {
  const { scope, calls } = windowsHarness(false);
  assert.deepEqual(scope.prepare({ PATH: "original" }), {
    PATH: "original",
    WEBVIEW2_USER_DATA_FOLDER: windowsProfile,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: browserArguments,
  });
  scope.restore();
  assert.deepEqual(
    calls.map((call) => call.action),
    ["snapshot"],
  );
});

void test("partial or denied policy setup rolls back before propagating the failure", () => {
  for (const message of ["second registry write failed", "access denied"]) {
    const { scope, calls } = windowsHarness(true, (action) => {
      if (action === "snapshot") return { elevated: true, values: [], createdKeys: [] };
      if (action === "configure") throw new Error(message);
    });
    assert.throws(() => scope.prepare({}), new RegExp(message));
    assert.deepEqual(
      calls.map((call) => call.action),
      ["snapshot", "configure", "restore"],
    );
  }
});

void test("failed rollback retains the snapshot for a later restoration attempt", () => {
  let attempts = 0;
  const { scope } = windowsHarness(true, (action) => {
    if (action === "snapshot") return { elevated: true, values: [], createdKeys: [] };
    if (action === "configure") throw new Error("write failed");
    if (action === "restore" && attempts++ === 0) throw new Error("restore failed");
  });
  assert.throws(() => scope.prepare({}), /setup and rollback failed/);
  scope.restore();
  assert.equal(attempts, 2);
});

for (const [commandLine, name, expected] of [
  [
    String.raw`msedgewebview2.exe --user-data-dir="C:\Smoke Test\unique-profile" --other`,
    "msedgewebview2.exe",
    true,
  ],
  [
    String.raw`msedgewebview2.exe --user-data-dir="c:/smoke test/UNIQUE-PROFILE/"`,
    "MSEDGEWEBVIEW2.EXE",
    true,
  ],
  [
    String.raw`msedgewebview2.exe --user-data-dir="C:\Smoke Test\unique-profile-other"`,
    "msedgewebview2.exe",
    false,
  ],
  [
    String.raw`msedgewebview2.exe --other="C:\Smoke Test\unique-profile"`,
    "msedgewebview2.exe",
    false,
  ],
  [String.raw`other.exe --user-data-dir="C:\Smoke Test\unique-profile"`, "other.exe", false],
  [null, "msedgewebview2.exe", false],
] as const) {
  void test(`profile matching ${String(commandLine)} (${name})`, () => {
    assert.equal(
      matchesProfile({ pid: 1, parentPid: 2, name, commandLine }, windowsProfile),
      expected,
    );
  });
}

void test("profile matching accepts an unquoted argument and rejects partial arguments", () => {
  const process = {
    pid: 1,
    parentPid: 2,
    name: "msedgewebview2.exe",
    commandLine: String.raw`msedgewebview2.exe --user-data-dir=C:\smoke\profile --other`,
  };
  assert.equal(matchesProfile(process, String.raw`C:\smoke\profile`), true);
  assert.equal(
    matchesProfile(
      { ...process, commandLine: process.commandLine.replace(" --user", " --fake-user") },
      String.raw`C:\smoke\profile`,
    ),
    false,
  );
});

void test("browser cleanup kills only the unique profile and verifies disappearance", async () => {
  const owned = {
    pid: 42,
    parentPid: 7,
    name: "msedgewebview2.exe",
    commandLine: `msedgewebview2.exe --user-data-dir="${windowsProfile}"`,
  };
  const unrelated = {
    ...owned,
    pid: 99,
    commandLine: "msedgewebview2.exe --user-data-dir=C:\\other",
  };
  let killed = false;
  const { scope, calls, events } = windowsHarness(true, (action) => {
    if (action === "processes") return killed ? [unrelated] : [owned, unrelated];
    if (action === "kill") killed = true;
  });
  await scope.stopProfileProcesses();
  assert.deepEqual(
    calls.filter((call) => call.action === "kill").map((call) => call.payload.pid),
    [42],
  );
  assert.ok(events.some((entry) => entry.event === "profile-processes-stopped"));
  assert.ok(calls.every((call) => call.timeoutMs !== undefined && call.timeoutMs <= 5_000));
});

void test("browser cleanup has a bounded deadline when a profile process survives", async (t) => {
  t.mock.timers.enable({ apis: ["Date"], now: 0 });
  const { scope } = windowsHarness(true, (action) => {
    if (action === "processes")
      return [
        {
          pid: 42,
          parentPid: 7,
          name: "msedgewebview2.exe",
          commandLine: `msedgewebview2.exe --user-data-dir="${windowsProfile}"`,
        },
      ];
    if (action === "kill") t.mock.timers.setTime(5_000);
  });
  await assert.rejects(scope.stopProfileProcesses(), /within 5000ms.*retained/);
});

void test("cleanup restores policy and preserves data after browser shutdown failure", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "browser-cleanup-"));
  t.after(() => {
    rmSync(root, { recursive: true, force: true });
  });
  const order: string[] = [];
  await assert.rejects(
    cleanup(
      undefined,
      [],
      root,
      () => {
        order.push("crashes");
      },
      {
        stopProfileProcesses: () => {
          order.push("browsers");
          return Promise.reject(new Error("browser survived"));
        },
        restore: () => {
          order.push("restore");
        },
      },
    ),
    /temporary test data was retained/,
  );
  assert.deepEqual(order, ["browsers", "crashes", "restore"]);
  assert.equal(existsSync(root), true);
});

void test("cleanup restores policies when crash preservation fails", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "crash-copy-"));
  t.after(() => {
    rmSync(root, { recursive: true, force: true });
  });
  let restored = false;
  await assert.rejects(
    cleanup(
      undefined,
      [],
      root,
      () => {
        throw new Error("copy failed");
      },
      {
        restore: () => {
          restored = true;
        },
      },
    ),
    /temporary test data was retained/,
  );
  assert.equal(restored, true);
  assert.equal(existsSync(root), true);
});

void test("cleanup retains data and records policy restoration failures", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "restore-failure-"));
  t.after(() => {
    rmSync(root, { recursive: true, force: true });
  });
  const records: unknown[] = [];
  await assert.rejects(
    cleanup(undefined, [], root, undefined, {
      restore: () => {
        throw new Error("policy restore denied");
      },
      record: (_event, data) => {
        records.push(data);
      },
    }),
    /temporary test data was retained/,
  );
  assert.match(JSON.stringify(records), /policy restore denied/);
  assert.equal(existsSync(root), true);
});

void test("debugging failures include the endpoint and nested connection error", async (t) => {
  t.mock.timers.enable({ apis: ["Date"], now: 0 });
  t.mock.method(globalThis, "fetch", () => {
    t.mock.timers.setTime(10_000);
    return Promise.reject(
      new TypeError("fetch failed", {
        cause: Object.assign(new Error("connection refused"), { code: "ECONNREFUSED" }),
      }),
    );
  });
  await assert.rejects(
    waitDebuggingReady(driver(t), MOCK_BASE, { timeoutMs: 10_000, pollMs: 0 }),
    /http:\/\/mock-driver\/json\/version:[\s\S]*ECONNREFUSED/,
  );
});

void test("browser cleanup kills root trees rather than each child separately", async () => {
  const browser = {
    pid: 42,
    parentPid: 7,
    name: "msedgewebview2.exe",
    commandLine: `msedgewebview2.exe --user-data-dir="${windowsProfile}"`,
  };
  let killed = false;
  const { scope, calls } = windowsHarness(true, (action) => {
    if (action === "processes")
      return killed ? [] : [browser, { ...browser, pid: 43, parentPid: 42 }];
    if (action === "kill") killed = true;
  });
  await scope.stopProfileProcesses();
  assert.deepEqual(
    calls.filter((call) => call.action === "kill").map((call) => call.payload.pid),
    [42],
  );
});

void test("cleanup uses bounded removal retries and restores policy before deletion", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "cleanup-remove-"));
  t.after(() => {
    rmSync(root, { recursive: true, force: true });
  });
  let restored = false;
  const records: unknown[] = [];
  await assert.rejects(
    cleanup(undefined, [], root, undefined, {
      restore: () => {
        restored = true;
      },
      remove: (path, options) => {
        assert.equal(restored, true);
        assert.equal(path, root);
        assert.deepEqual(options, {
          recursive: true,
          force: true,
          maxRetries: 10,
          retryDelay: 100,
        });
        return Promise.reject(Object.assign(new Error("profile locked"), { code: "EPERM" }));
      },
      record: (_event, data) => {
        records.push(data);
      },
    }),
    new RegExp(root),
  );
  assert.equal(existsSync(root), true);
  assert.match(JSON.stringify(records), /EPERM/);
});
