import { createServer } from "node:net";
import { setTimeout as delay } from "node:timers/promises";
import { rm } from "node:fs/promises";
import { inspect } from "node:util";

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
          const url = `${base}/json/${path}`;
          let response: Response;
          let body: string;
          try {
            response = await fetch(url, {
              signal: AbortSignal.timeout(
                Math.min(requestTimeoutMs, Math.max(1, deadline - Date.now())),
              ),
            });
            body = await response.text();
          } catch (error) {
            throw new Error(`${url}: ${inspect(error, { depth: 5 })}`, { cause: error });
          }
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

interface CleanupOptions {
  stopProfileProcesses?: () => Promise<void>;
  restore?: () => void;
  record?: (event: string, data: unknown) => void;
  remove?: typeof rm;
}

/** Stop every owned process even if another fails; delete data only after all stop. */
export async function cleanup(
  session: Session | undefined,
  processes: (OwnedProcess | undefined)[],
  root: string | undefined,
  beforeRemove: () => void = () => undefined,
  {
    stopProfileProcesses = () => Promise.resolve(),
    restore = () => {},
    record = () => {},
    remove = rm,
  }: CleanupOptions = {},
): Promise<void> {
  const failures: unknown[] = [];
  await session?.end().catch((error: unknown) => {
    record("session-end-failed", inspect(error));
  });
  const results = await Promise.allSettled(
    processes.map(async (process) => {
      await process?.stop();
    }),
  );
  for (const result of results)
    if (result.status === "rejected") failures.push(result.reason as unknown);
  try {
    await stopProfileProcesses();
  } catch (error) {
    failures.push(error);
  }
  try {
    beforeRemove();
  } catch (error) {
    failures.push(error);
  }
  // Restoration must run even when process shutdown or crash-report copying failed.
  try {
    restore();
  } catch (error) {
    failures.push(error);
  }
  if (!failures.length && root) {
    try {
      await remove(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    } catch (error) {
      failures.push(error);
    }
  }
  record("cleanup", {
    root,
    retained: failures.length > 0,
    errors: failures.map((error) => inspect(error, { depth: 5 })),
  });
  if (failures.length)
    throw new AggregateError(
      failures,
      `Smoke cleanup failed; temporary test data was retained at ${root ?? "unknown location"}`,
    );
}
