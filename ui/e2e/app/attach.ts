import { createServer } from "node:net";
import { setTimeout as delay } from "node:timers/promises";
import { rmSync } from "node:fs";

import { OwnedProcess } from "./driver.ts";
import { Session } from "./webdriver.ts";

export async function availablePort(): Promise<number> {
  const server = createServer();
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("No loopback port allocated");
  await new Promise<void>((resolve, reject) =>
    server.close((error) => {
      if (error) reject(error);
      else resolve();
    }),
  );
  return address.port;
}

export async function waitDebuggingReady(
  app: OwnedProcess,
  base: string,
  { timeoutMs = 30_000, requestTimeoutMs = 1_000, pollMs = 200 } = {},
  record: (path: string, body: string) => void = () => undefined,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  let lastError = "no response";
  let responseError = "";
  const invalid = (message: string): never => {
    responseError = message;
    throw new Error(message);
  };
  const poll = async () => {
    while (Date.now() < deadline) {
      await app.guard(Promise.resolve());
      try {
        const read = async (path: string): Promise<unknown> => {
          const response = await fetch(`${base}/json/${path}`, {
            signal: AbortSignal.timeout(
              Math.min(requestTimeoutMs, Math.max(1, deadline - Date.now())),
            ),
          });
          const body = await response.text();
          record(path, body);
          if (!response.ok) invalid(`${path}: HTTP ${String(response.status)}: ${body}`);
          try {
            return JSON.parse(body) as unknown;
          } catch {
            invalid(`${path}: malformed JSON: ${body}`);
          }
        };
        const version = await read("version");
        if (
          !version ||
          typeof version !== "object" ||
          !("Browser" in version) ||
          typeof version.Browser !== "string" ||
          !("webSocketDebuggerUrl" in version) ||
          typeof version.webSocketDebuggerUrl !== "string" ||
          !version.webSocketDebuggerUrl.startsWith("ws://")
        ) {
          invalid("version: missing Browser or webSocketDebuggerUrl");
        }
        const targets = await read("list");
        if (
          !Array.isArray(targets) ||
          !targets.some(
            (target: unknown) =>
              target &&
              typeof target === "object" &&
              "type" in target &&
              target.type === "page" &&
              "id" in target &&
              typeof target.id === "string" &&
              "webSocketDebuggerUrl" in target &&
              typeof target.webSocketDebuggerUrl === "string" &&
              target.webSocketDebuggerUrl.startsWith("ws://"),
          )
        ) {
          invalid("list: no valid page target");
        }
        return;
      } catch (error) {
        lastError = error instanceof Error ? error.message : String(error);
      }
      await delay(Math.min(pollMs, Math.max(0, deadline - Date.now())));
    }
    throw new Error(
      `WebView2 debugging endpoint readiness timed out after ${String(timeoutMs)}ms: ${lastError}${responseError && responseError !== lastError ? `; last invalid response: ${responseError}` : ""}`,
    );
  };
  await app.guard(poll());
}

/** Stop every owned process even if another fails; delete data only after all stop. */
export async function cleanup(
  session: Session | undefined,
  processes: (OwnedProcess | undefined)[],
  root: string | undefined,
  beforeRemove: () => void = () => undefined,
): Promise<void> {
  await session?.end().catch(() => undefined);
  const results = await Promise.allSettled(
    processes.map(async (process) => {
      await process?.stop();
    }),
  );
  const failures = results.filter((result) => result.status === "rejected");
  if (failures.length)
    throw new AggregateError(
      failures.map((result) => result.reason as unknown),
      "Process cleanup failed; temporary test data was retained",
    );
  beforeRemove();
  if (root) rmSync(root, { recursive: true, force: true });
}
