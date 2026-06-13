import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  retries: 0,
  reporter: process.env.PLAYWRIGHT_VIDEO === "1"
    ? [["list"], ["json", { outputFile: "test-results/report.json" }]]
    : "list",
  use: {
    launchOptions: { args: ["--use-gl=swiftshader"] },
    headless: true,
    baseURL: "http://127.0.0.1:4173",
    video: process.env.PLAYWRIGHT_VIDEO === "1" ? "on" : "off",
  },
  webServer: [
    {
      command: "npm run preview",
      url: "http://127.0.0.1:4173",
      reuseExistingServer: !process.env.CI,
    },
    {
      // Copy to /tmp to prevent POST mutations from polluting the source-controlled file.
      // Use --release: xtask ci pre-builds release binary (`cargo test -p engawa-api --release static_assets`),
      // so launch is instant and avoids debug cold-cache build exceeding the 120s timeout (Issue #145).
      command:
        "bash -c 'cp examples/simple_box.engawa /tmp/engawa-test-server.engawa && cargo run -p engawa-api --release -- /tmp/engawa-test-server.engawa'",
      url: "http://127.0.0.1:7878/api/v0/mesh",
      // Always launch a fresh engawa-api: a pre-existing server bound to 7878 may
      // be serving a different .engawa file, which would silently break tests
      // that depend on /tmp/engawa-test-server.engawa content (Issue #145).
      reuseExistingServer: false,
      timeout: 120_000,
      cwd: "..",
    },
  ],
});
