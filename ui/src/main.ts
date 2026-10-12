import { mount } from "svelte";

import App from "./App.svelte";
import "./lib/ui/tokens.css";
import "./lib/ui/base.css";

// End-to-end tests run the UI in a plain browser against a fake backend.
// The condition is static, so this import never reaches production builds.
if (import.meta.env.MODE === "e2e") {
  const { installFakeBackend } = await import("../e2e/fake/backend");
  installFakeBackend();
}

const target = document.getElementById("app");
if (!target) {
  throw new Error("Missing #app mount point");
}

mount(App, { target });
