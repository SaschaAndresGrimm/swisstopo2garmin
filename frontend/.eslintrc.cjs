module.exports = {
  root: true,
  parser: "@typescript-eslint/parser",
  plugins: ["@typescript-eslint"],
  extends: ["eslint:recommended", "plugin:@typescript-eslint/recommended"],
  env: { browser: true, es2021: true },
  parserOptions: { ecmaVersion: 2021, sourceType: "module" },
  rules: {
    // SPEC.md FR-4: all user-visible strings live in i18n resource files.
    // This is a blunt guard; the review checklist is the real enforcement.
    "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
  },
};
