// A minimal W3C WebDriver client over `fetch`, enough to drive the real app
// through `tauri-driver` without pulling in a WebDriver framework.

const ELEMENT = "element-6066-11e4-a52f-4a5d6b6b8b4d";

/** WebDriver key codes, see https://www.w3.org/TR/webdriver2/#keyboard-actions */
export const Keys = { space: "", enter: "", arrowDown: "" } as const;

interface WebDriverResponse<T> {
  value: T;
}

export class Session {
  private readonly base: string;
  readonly id: string;

  private constructor(base: string, id: string) {
    this.base = base;
    this.id = id;
  }

  static async start(base: string, application: string, timeoutMs = 90_000): Promise<Session> {
    const response = await request<{ sessionId: string }>(
      `${base}/session`,
      "POST",
      {
        capabilities: {
          alwaysMatch: { browserName: "wry", "tauri:options": { application } },
        },
      },
      timeoutMs,
    );
    return new Session(base, response.sessionId);
  }

  static async attach(base: string, debuggerAddress: string, timeoutMs = 90_000): Promise<Session> {
    const response = await request<{ sessionId: string }>(
      `${base}/session`,
      "POST",
      {
        capabilities: {
          alwaysMatch: {
            browserName: "webview2",
            "ms:edgeChromium": true,
            "ms:edgeOptions": { debuggerAddress },
          },
        },
      },
      timeoutMs,
    );
    return new Session(base, response.sessionId);
  }

  async end(): Promise<void> {
    await request(`${this.base}/session/${this.id}`, "DELETE");
  }

  /** Waits until `script` returns a truthy value and returns it. */
  async waitFor<T>(description: string, script: string, timeoutMs = 20_000): Promise<T> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const value = await this.execute<T | null>(script);
      if (value) return value;
      if (Date.now() > deadline) throw new Error(`Timed out waiting for ${description}`);
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
  }

  execute<T>(script: string): Promise<T> {
    return request<T>(`${this.base}/session/${this.id}/execute/sync`, "POST", {
      script,
      args: [],
    });
  }

  async find(css: string): Promise<string> {
    const element = await request<Record<string, string>>(
      `${this.base}/session/${this.id}/element`,
      "POST",
      { using: "css selector", value: css },
    );
    const id = element[ELEMENT];
    if (!id) throw new Error(`No element id for ${css}`);
    return id;
  }

  async click(css: string): Promise<void> {
    const element = await this.find(css);
    await request(`${this.base}/session/${this.id}/element/${element}/click`, "POST", {});
  }

  /** Types into whatever has focus, key by key (W3C key actions). */
  async keys(text: string): Promise<void> {
    const characters = Array.from(new Intl.Segmenter().segment(text), (part) => part.segment);
    const actions = characters.flatMap((value) => [
      { type: "keyDown", value },
      { type: "keyUp", value },
    ]);
    await request(`${this.base}/session/${this.id}/actions`, "POST", {
      actions: [{ type: "key", id: "keyboard", actions }],
    });
  }

  async type(css: string, text: string): Promise<void> {
    const element = await this.find(css);
    await request(`${this.base}/session/${this.id}/element/${element}/value`, "POST", { text });
  }
}

async function request<T>(
  url: string,
  method: string,
  body?: unknown,
  timeoutMs = 10_000,
): Promise<T> {
  const signal = AbortSignal.timeout(timeoutMs);
  try {
    const response = await fetch(url, {
      method,
      headers: { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal,
    });
    const text = await response.text();
    let json: WebDriverResponse<T>;
    try {
      json = JSON.parse(text) as WebDriverResponse<T>;
    } catch {
      throw new Error(
        `${method} ${url} returned invalid JSON (HTTP ${String(response.status)}): ${text}`,
      );
    }
    if (!response.ok) {
      throw new Error(`${method} ${url} failed: ${JSON.stringify(json.value)}`);
    }
    return json.value;
  } catch (error) {
    if (signal.aborted)
      throw new Error(`${method} ${url} timed out after ${String(timeoutMs)}ms`, { cause: error });
    throw error;
  }
}
