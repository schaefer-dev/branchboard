import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";

/** Owns a child process and preserves failures during startup and session creation. */
export class OwnedProcess {
  readonly child: ChildProcess;
  private readonly failure: Promise<never>;
  private readonly closed: Promise<void>;
  protected problem: Error | undefined;
  private stopped = false;

  constructor(
    program: string,
    args: string[],
    env: NodeJS.ProcessEnv = process.env,
    log: (message: string) => void = () => undefined,
  ) {
    this.child = spawn(program, args, { stdio: ["ignore", "pipe", "pipe"], env });
    this.failure = new Promise<never>((_, reject) => {
      this.child.once("error", (error) => {
        this.problem = new Error(`Could not start ${program}: ${error.message}`, { cause: error });
        log(`${this.problem.message}\n`);
        reject(this.problem);
      });
      this.child.once("exit", (code, signal) => {
        const message = `${program} exited (code=${String(code)}, signal=${String(signal)})`;
        log(`${message}\n`);
        if (!this.stopped) {
          this.problem = new Error(message);
          reject(this.problem);
        }
      });
    });
    // A process may fail before the next operation starts racing it.
    void this.failure.catch(() => undefined);
    this.closed = new Promise((resolve) =>
      this.child.once("close", () => {
        resolve();
      }),
    );
    this.child.stdout?.on("data", (data: Buffer) => {
      log(`[stdout] ${data.toString()}`);
    });
    this.child.stderr?.on("data", (data: Buffer) => {
      log(`[stderr] ${data.toString()}`);
    });
  }

  async guard<T>(operation: Promise<T>): Promise<T> {
    return Promise.race([operation, this.problem ? Promise.reject(this.problem) : this.failure]);
  }

  async stop(): Promise<void> {
    this.stopped = true;
    this.problem ??= new Error("Owned process stopped");
    if (
      process.platform === "win32" &&
      this.child.pid &&
      this.child.exitCode === null &&
      this.child.signalCode === null
    ) {
      const result = spawnSync("taskkill", ["/PID", String(this.child.pid), "/T", "/F"], {
        encoding: "utf8",
        timeout: 5_000,
      });
      if (result.error || result.status !== 0) {
        throw new Error(
          `Process tree termination failed: ${result.error?.message ?? result.stderr}; temporary test data was retained`,
        );
      }
    } else if (this.child.exitCode === null && this.child.signalCode === null) this.child.kill();
    const wait = async () => {
      const controller = new AbortController();
      try {
        return await Promise.race([
          this.closed.then(() => true),
          delay(5_000, false, { signal: controller.signal }),
        ]);
      } finally {
        controller.abort();
      }
    };
    if (await wait()) return;
    this.child.kill("SIGKILL");
    if (!(await wait()))
      throw new Error("Process did not terminate; temporary test data was retained");
  }
}

export class Driver extends OwnedProcess {
  async waitReady(
    base: string,
    { timeoutMs = 10_000, requestTimeoutMs = 1_000, pollMs = 200 } = {},
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
        if (this.problem) throw this.problem;
        try {
          const response = await fetch(`${base}/status`, {
            signal: AbortSignal.timeout(
              Math.min(requestTimeoutMs, Math.max(1, deadline - Date.now())),
            ),
          });
          const text = await response.text();
          if (!response.ok) invalid(`HTTP ${String(response.status)}: ${text}`);
          let body: { value?: { ready?: boolean } } | null = null;
          try {
            body = JSON.parse(text) as { value?: { ready?: boolean } } | null;
          } catch {
            invalid(`invalid JSON (HTTP ${String(response.status)}): ${text}`);
          }
          if (body?.value?.ready === true) return;
          responseError = lastError = `driver not ready: ${text}`;
        } catch (error) {
          lastError = error instanceof Error ? error.message : String(error);
        }
        await delay(Math.min(pollMs, Math.max(0, deadline - Date.now())));
      }
      throw new Error(
        `Driver readiness timed out after ${String(timeoutMs)}ms: ${lastError}${responseError && responseError !== lastError ? `; last invalid response: ${responseError}` : ""}`,
      );
    };
    await this.guard(poll());
  }
}
