import assert from "node:assert/strict";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { randomUUID } from "node:crypto";
import { test, type TestContext } from "node:test";

import { Driver } from "./driver.ts";
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
  ["malformed response", 200, "not JSON", /JSON/],
  ["not ready", 200, '{"value":{"ready":false}}', /driver not ready/],
] as const) {
  void test(`readiness rejects ${name} with diagnostic context`, async (t) => {
    const base = await server(t, (_request, response) => response.writeHead(status).end(body));
    await assert.rejects(driver(t).waitReady(base, { timeoutMs: 150, pollMs: 10 }), message);
  });
}

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
