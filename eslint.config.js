import js from "@eslint/js";
import svelte from "eslint-plugin-svelte";
import globals from "globals";
import ts from "typescript-eslint";

export default ts.config(
  {
    ignores: [
      "dist/",
      "target/",
      "src-tauri/",
      "e2e-results/",
      "ui/src/lib/api/generated/",
      "ui/e2e/fixtures/",
    ],
  },
  js.configs.recommended,
  ...ts.configs.strictTypeChecked,
  ...svelte.configs.recommended,
  ...svelte.configs.prettier,
  {
    languageOptions: {
      globals: globals.browser,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
        extraFileExtensions: [".svelte"],
      },
    },
  },
  {
    files: ["**/*.svelte", "**/*.svelte.ts"],
    languageOptions: { parserOptions: { parser: ts.parser } },
    rules: {
      // `let { x = $bindable() } = $props()` is a rune, not a default value.
      "@typescript-eslint/no-useless-default-assignment": "off",
    },
  },
  {
    files: ["eslint.config.js"],
    ...ts.configs.disableTypeChecked,
  },
);
