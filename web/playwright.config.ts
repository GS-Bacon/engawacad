import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  retries: 0,
  use: {
    launchOptions: { args: ["--use-gl=swiftshader"] },
    headless: true,
    baseURL: "http://127.0.0.1:4173",
  },
  webServer: [
    {
      command: "npm run preview",
      url: "http://127.0.0.1:4173",
      reuseExistingServer: !process.env.CI,
    },
    {
      // cwd: repo root so "examples/simple_box.mycad" resolves correctly
      command: "cargo run -p mycad-api -- examples/simple_box.mycad",
      url: "http://127.0.0.1:7878/api/v0/mesh",
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
      cwd: "..",
    },
  ],
});
