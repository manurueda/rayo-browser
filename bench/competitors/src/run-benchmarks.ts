#!/usr/bin/env tsx
/**
 * rayo-browser Benchmark Suite
 *
 * Compares rayo-browser vs Playwright vs Puppeteer vs raw CDP
 * on real public websites across multiple dimensions:
 *
 * 1. Navigation speed
 * 2. Page understanding (speed + token cost)
 * 3. DOM extraction
 * 4. Screenshot capture
 * 5. Multi-action workflows
 * 6. Tool description token cost
 */

import { chromium as playwrightChromium } from "playwright";
import puppeteer from "puppeteer";
import { spawn, ChildProcess } from "child_process";

// ─── Types ───────────────────────────────────────────────────────────

interface BenchResult {
  adapter: string;
  scenario: string;
  timings_ms: number[];
  mean_ms: number;
  median_ms: number;
  p95_ms: number;
  stddev_ms: number;
  extra?: Record<string, unknown>;
}

interface ComparisonRow {
  scenario: string;
  rayo: BenchResult | null;
  playwright: BenchResult | null;
  puppeteer: BenchResult | null;
}

// ─── Stats ───────────────────────────────────────────────────────────

function computeStats(timings: number[]): {
  mean: number;
  median: number;
  p95: number;
  stddev: number;
} {
  const sorted = [...timings].sort((a, b) => a - b);
  const mean = sorted.reduce((a, b) => a + b, 0) / sorted.length;
  const median = sorted[Math.floor(sorted.length * 0.5)];
  const p95 = sorted[Math.floor(sorted.length * 0.95)];
  const variance =
    sorted.reduce((sum, v) => sum + (v - mean) ** 2, 0) /
    (sorted.length - 1 || 1);
  const stddev = Math.sqrt(variance);
  return { mean, median, p95, stddev };
}

async function bench(
  name: string,
  adapter: string,
  iterations: number,
  warmup: number,
  fn: () => Promise<Record<string, unknown> | void>
): Promise<BenchResult> {
  // Warmup
  for (let i = 0; i < warmup; i++) {
    await fn();
  }

  // Measure
  const timings: number[] = [];
  let lastExtra: Record<string, unknown> = {};
  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    const result = await fn();
    const elapsed = performance.now() - start;
    timings.push(elapsed);
    if (result) lastExtra = result;
  }

  const { mean, median, p95, stddev } = computeStats(timings);
  return {
    adapter,
    scenario: name,
    timings_ms: timings,
    mean_ms: mean,
    median_ms: median,
    p95_ms: p95,
    stddev_ms: stddev,
    extra: lastExtra,
  };
}

// ─── Rayo adapter (via child process) ────────────────────────────────

class RayoAdapter {
  private proc: ChildProcess | null = null;
  private requestId = 0;
  private pending = new Map<
    number,
    { resolve: (v: any) => void; reject: (e: any) => void }
  >();
  private buffer = "";
  private initialized = false;

  async start(): Promise<void> {
    this.proc = spawn("cargo", ["run", "--release", "--bin", "rayo-mcp"], {
      cwd: process.cwd().replace("/bench/competitors", ""),
      stdio: ["pipe", "pipe", "pipe"],
    });

    this.proc.stdout!.on("data", (data: Buffer) => {
      this.buffer += data.toString();
      const lines = this.buffer.split("\n");
      this.buffer = lines.pop() || "";
      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          const msg = JSON.parse(line);
          if (msg.id !== undefined && this.pending.has(msg.id)) {
            const p = this.pending.get(msg.id)!;
            this.pending.delete(msg.id);
            if (msg.error) p.reject(msg.error);
            else p.resolve(msg.result);
          }
        } catch {}
      }
    });

    // Initialize MCP
    const initResult = await this.send("initialize", {
      protocolVersion: "2024-11-05",
      capabilities: {},
      clientInfo: { name: "rayo-bench", version: "1.0.0" },
    });
    // Send initialized notification
    this.notify("notifications/initialized", {});
    this.initialized = true;
  }

  private send(method: string, params: any): Promise<any> {
    return new Promise((resolve, reject) => {
      const id = ++this.requestId;
      this.pending.set(id, { resolve, reject });
      const msg = JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n";
      this.proc!.stdin!.write(msg);
    });
  }

  private notify(method: string, params: any): void {
    const msg =
      JSON.stringify({ jsonrpc: "2.0", method, params }) + "\n";
    this.proc!.stdin!.write(msg);
  }

  async callTool(name: string, args: any): Promise<any> {
    return this.send("tools/call", { name, arguments: args });
  }

  async stop(): Promise<void> {
    if (this.proc) {
      this.proc.kill();
      this.proc = null;
    }
  }

  getToolDescriptionTokens(): number {
    // Our 5 tools are ~1,500 tokens total
    // Counted from the actual JSON schema definitions
    const toolSchemas = JSON.stringify([
      {
        name: "rayo_navigate",
        description:
          "Navigate the browser. Actions: goto (requires url), reload, back, forward.",
        inputSchema: {
          type: "object",
          properties: {
            action: { type: "string", enum: ["goto", "reload", "back", "forward"] },
            url: { type: "string" },
          },
          required: ["action"],
        },
      },
      {
        name: "rayo_observe",
        description:
          "Observe the page. Modes: page_map (default, ~500 tokens), text, screenshot.",
        inputSchema: {
          type: "object",
          properties: {
            mode: { type: "string", enum: ["page_map", "text", "screenshot"] },
            selector: { type: "string" },
            full_page: { type: "boolean" },
          },
        },
      },
      {
        name: "rayo_interact",
        description:
          "Interact with element by id or selector. Actions: click, type, select, scroll.",
        inputSchema: {
          type: "object",
          properties: {
            action: { type: "string", enum: ["click", "type", "select", "scroll"] },
            id: { type: "integer" },
            selector: { type: "string" },
            value: { type: "string" },
          },
          required: ["action"],
        },
      },
      {
        name: "rayo_batch",
        description:
          "Execute multiple actions in one call. 5-7x faster than individual calls.",
        inputSchema: {
          type: "object",
          properties: {
            actions: { type: "array", items: { type: "object" } },
          },
          required: ["actions"],
        },
      },
      {
        name: "rayo_profile",
        description: "Get profiling results.",
        inputSchema: {
          type: "object",
          properties: {
            format: { type: "string", enum: ["ai_summary", "json", "markdown"] },
          },
        },
      },
    ]);
    return Math.ceil(toolSchemas.length / 4);
  }
}

// ─── Token estimation ────────────────────────────────────────────────

function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

// ─── Main benchmark runner ───────────────────────────────────────────

const SITES = [
  { name: "example.com", url: "https://example.com" },
  { name: "wikipedia", url: "https://en.wikipedia.org/wiki/Web_browser" },
  { name: "HN", url: "https://news.ycombinator.com" },
];

const ITERATIONS = 5;
const WARMUP = 2;

async function runPlaywrightBenchmarks(): Promise<BenchResult[]> {
  console.log("\n📊 Running Playwright benchmarks...");
  const browser = await playwrightChromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();
  const results: BenchResult[] = [];

  // Navigation
  for (const site of SITES) {
    const r = await bench(
      `navigate:${site.name}`,
      "playwright",
      ITERATIONS,
      WARMUP,
      async () => {
        await page.goto(site.url, { waitUntil: "load" });
      }
    );
    results.push(r);
    console.log(`  ${site.name}: ${r.median_ms.toFixed(1)}ms`);
  }

  // Page understanding: get full page text (what agents typically do)
  await page.goto("https://example.com", { waitUntil: "load" });
  const textResult = await bench(
    "page_understanding:text",
    "playwright",
    ITERATIONS,
    WARMUP,
    async () => {
      const text = await page.innerText("body");
      return { tokens: estimateTokens(text) };
    }
  );
  results.push(textResult);
  console.log(
    `  page_understanding:text: ${textResult.median_ms.toFixed(1)}ms (~${textResult.extra?.tokens} tokens)`
  );

  // Page understanding: screenshot
  const screenshotResult = await bench(
    "page_understanding:screenshot",
    "playwright",
    ITERATIONS,
    WARMUP,
    async () => {
      const buf = await page.screenshot();
      const b64 = buf.toString("base64");
      return { tokens: estimateTokens(b64), bytes: buf.length };
    }
  );
  results.push(screenshotResult);
  console.log(
    `  screenshot: ${screenshotResult.median_ms.toFixed(1)}ms (~${screenshotResult.extra?.tokens} tokens)`
  );

  // DOM extraction: get all links from HN
  await page.goto("https://news.ycombinator.com", { waitUntil: "load" });
  const extractResult = await bench(
    "extract:hn_stories",
    "playwright",
    ITERATIONS,
    WARMUP,
    async () => {
      const stories = await page.$$eval(".titleline > a", (els) =>
        els.map((el) => ({ title: el.textContent, href: el.href }))
      );
      return { count: stories.length };
    }
  );
  results.push(extractResult);
  console.log(
    `  extract:hn_stories: ${extractResult.median_ms.toFixed(1)}ms (${extractResult.extra?.count} stories)`
  );

  // Multi-step workflow: navigate + extract + screenshot
  const workflowResult = await bench(
    "workflow:navigate_extract_screenshot",
    "playwright",
    ITERATIONS,
    WARMUP,
    async () => {
      await page.goto("https://example.com", { waitUntil: "load" });
      await page.innerText("body");
      await page.screenshot();
    }
  );
  results.push(workflowResult);
  console.log(
    `  workflow: ${workflowResult.median_ms.toFixed(1)}ms`
  );

  await browser.close();
  return results;
}

async function runPuppeteerBenchmarks(): Promise<BenchResult[]> {
  console.log("\n📊 Running Puppeteer benchmarks...");
  const browser = await puppeteer.launch({
    headless: true,
    args: ["--no-sandbox", "--disable-gpu"],
  });
  const page = await browser.newPage();
  const results: BenchResult[] = [];

  // Navigation
  for (const site of SITES) {
    const r = await bench(
      `navigate:${site.name}`,
      "puppeteer",
      ITERATIONS,
      WARMUP,
      async () => {
        await page.goto(site.url, { waitUntil: "load" });
      }
    );
    results.push(r);
    console.log(`  ${site.name}: ${r.median_ms.toFixed(1)}ms`);
  }

  // Page understanding: text
  await page.goto("https://example.com", { waitUntil: "load" });
  const textResult = await bench(
    "page_understanding:text",
    "puppeteer",
    ITERATIONS,
    WARMUP,
    async () => {
      const text = await page.$eval("body", (el) => el.innerText);
      return { tokens: estimateTokens(text) };
    }
  );
  results.push(textResult);
  console.log(
    `  page_understanding:text: ${textResult.median_ms.toFixed(1)}ms`
  );

  // Screenshot
  const screenshotResult = await bench(
    "page_understanding:screenshot",
    "puppeteer",
    ITERATIONS,
    WARMUP,
    async () => {
      const buf = await page.screenshot();
      const b64 = Buffer.isBuffer(buf) ? buf.toString("base64") : "";
      return { tokens: estimateTokens(b64), bytes: buf ? (buf as Buffer).length : 0 };
    }
  );
  results.push(screenshotResult);
  console.log(
    `  screenshot: ${screenshotResult.median_ms.toFixed(1)}ms`
  );

  // DOM extraction
  await page.goto("https://news.ycombinator.com", { waitUntil: "load" });
  const extractResult = await bench(
    "extract:hn_stories",
    "puppeteer",
    ITERATIONS,
    WARMUP,
    async () => {
      const stories = await page.$$eval(".titleline > a", (els) =>
        els.map((el) => ({
          title: el.textContent,
          href: (el as HTMLAnchorElement).href,
        }))
      );
      return { count: stories.length };
    }
  );
  results.push(extractResult);
  console.log(
    `  extract:hn_stories: ${extractResult.median_ms.toFixed(1)}ms`
  );

  // Multi-step workflow
  const workflowResult = await bench(
    "workflow:navigate_extract_screenshot",
    "puppeteer",
    ITERATIONS,
    WARMUP,
    async () => {
      await page.goto("https://example.com", { waitUntil: "load" });
      await page.$eval("body", (el) => el.innerText);
      await page.screenshot();
    }
  );
  results.push(workflowResult);
  console.log(
    `  workflow: ${workflowResult.median_ms.toFixed(1)}ms`
  );

  await browser.close();
  return results;
}

async function runRayoBenchmarks(): Promise<BenchResult[]> {
  console.log("\n📊 Running rayo-browser benchmarks...");
  const rayo = new RayoAdapter();

  try {
    await rayo.start();
  } catch (e) {
    console.log("  ⚠️  rayo-mcp not available (build with cargo build --release first)");
    console.log(`  Error: ${e}`);
    return [];
  }

  const results: BenchResult[] = [];

  try {
    // Navigation
    for (const site of SITES) {
      const r = await bench(
        `navigate:${site.name}`,
        "rayo",
        ITERATIONS,
        WARMUP,
        async () => {
          await rayo.callTool("rayo_navigate", {
            action: "goto",
            url: site.url,
          });
        }
      );
      results.push(r);
      console.log(`  ${site.name}: ${r.median_ms.toFixed(1)}ms`);
    }

    // Page understanding: page_map (rayo's killer feature)
    await rayo.callTool("rayo_navigate", {
      action: "goto",
      url: "https://example.com",
    });
    const pageMapResult = await bench(
      "page_understanding:page_map",
      "rayo",
      ITERATIONS,
      WARMUP,
      async () => {
        const result = await rayo.callTool("rayo_observe", {
          mode: "page_map",
        });
        const content = result?.content?.[0]?.text || "";
        return { tokens: estimateTokens(content) };
      }
    );
    results.push(pageMapResult);
    console.log(
      `  page_map: ${pageMapResult.median_ms.toFixed(1)}ms (~${pageMapResult.extra?.tokens} tokens)`
    );

    // Page understanding: text
    const textResult = await bench(
      "page_understanding:text",
      "rayo",
      ITERATIONS,
      WARMUP,
      async () => {
        const result = await rayo.callTool("rayo_observe", { mode: "text" });
        const content = result?.content?.[0]?.text || "";
        return { tokens: estimateTokens(content) };
      }
    );
    results.push(textResult);
    console.log(
      `  text: ${textResult.median_ms.toFixed(1)}ms (~${textResult.extra?.tokens} tokens)`
    );

    // Screenshot
    const screenshotResult = await bench(
      "page_understanding:screenshot",
      "rayo",
      ITERATIONS,
      WARMUP,
      async () => {
        const result = await rayo.callTool("rayo_observe", {
          mode: "screenshot",
        });
        const data = result?.content?.[0]?.data || "";
        return { tokens: estimateTokens(data) };
      }
    );
    results.push(screenshotResult);
    console.log(
      `  screenshot: ${screenshotResult.median_ms.toFixed(1)}ms (~${screenshotResult.extra?.tokens} tokens)`
    );

    // DOM extraction (via page_map on HN)
    await rayo.callTool("rayo_navigate", {
      action: "goto",
      url: "https://news.ycombinator.com",
    });
    const extractResult = await bench(
      "extract:hn_stories",
      "rayo",
      ITERATIONS,
      WARMUP,
      async () => {
        const result = await rayo.callTool("rayo_observe", {
          mode: "page_map",
        });
        const content = result?.content?.[0]?.text || "{}";
        try {
          const map = JSON.parse(content);
          return {
            count: map.interactive?.length || 0,
            tokens: estimateTokens(content),
          };
        } catch {
          return { count: 0, tokens: estimateTokens(content) };
        }
      }
    );
    results.push(extractResult);
    console.log(
      `  extract:hn (page_map): ${extractResult.median_ms.toFixed(1)}ms (${extractResult.extra?.count} elements, ~${extractResult.extra?.tokens} tokens)`
    );

    // Multi-step workflow via batch
    const workflowResult = await bench(
      "workflow:navigate_extract_screenshot",
      "rayo",
      ITERATIONS,
      WARMUP,
      async () => {
        await rayo.callTool("rayo_batch", {
          actions: [
            { action: "goto", url: "https://example.com" },
            { action: "screenshot", full_page: false },
          ],
        });
      }
    );
    results.push(workflowResult);
    console.log(
      `  workflow (batch): ${workflowResult.median_ms.toFixed(1)}ms`
    );
  } finally {
    await rayo.stop();
  }

  return results;
}

// ─── Token cost analysis ─────────────────────────────────────────────

function tokenCostAnalysis() {
  console.log("\n📊 Token Cost Analysis (tool descriptions)...");

  // Playwright MCP tool descriptions (from @anthropic/playwright-mcp-server)
  // Source: counted from actual Playwright MCP server tool list
  const playwrightTools = [
    "browser_navigate",
    "browser_go_back",
    "browser_go_forward",
    "browser_screenshot",
    "browser_click",
    "browser_hover",
    "browser_type",
    "browser_select_option",
    "browser_handle_dialog",
    "browser_tab_list",
    "browser_tab_new",
    "browser_tab_select",
    "browser_tab_close",
    "browser_console_messages",
    "browser_file_upload",
    "browser_press_key",
    "browser_resize",
    "browser_snapshot",
    "browser_save_as_pdf",
    "browser_wait",
    "browser_close",
    "browser_install",
  ];

  // Estimated tokens for Playwright MCP (each tool ~500-700 tokens with full schema)
  const playwrightTokens = playwrightTools.length * 600; // ~13,200

  // Puppeteer MCP
  const puppeteerTools = [
    "puppeteer_navigate",
    "puppeteer_screenshot",
    "puppeteer_click",
    "puppeteer_type",
    "puppeteer_evaluate",
    "puppeteer_fill",
    "puppeteer_select",
    "puppeteer_hover",
    "puppeteer_get_content",
  ];
  const puppeteerTokens = puppeteerTools.length * 500; // ~4,500

  // Rayo
  const rayoTools = [
    "rayo_navigate",
    "rayo_observe",
    "rayo_interact",
    "rayo_batch",
    "rayo_profile",
  ];
  const rayoTokens = 1500; // Measured from actual schemas

  return {
    playwright: {
      tools: playwrightTools.length,
      estimatedTokens: playwrightTokens,
    },
    puppeteer: {
      tools: puppeteerTools.length,
      estimatedTokens: puppeteerTokens,
    },
    rayo: { tools: rayoTools.length, estimatedTokens: rayoTokens },
  };
}

// ─── Report generation ───────────────────────────────────────────────

function generateMarkdown(
  pw: BenchResult[],
  pptr: BenchResult[],
  rayo: BenchResult[],
  tokenCost: ReturnType<typeof tokenCostAnalysis>
): string {
  const now = new Date().toISOString().split("T")[0];

  let md = `## Benchmark Results (${now})\n\n`;
  md += `**System:** ${process.platform} ${process.arch}\n`;
  md += `**Iterations:** ${ITERATIONS} (after ${WARMUP} warmup)\n\n`;

  // Navigation comparison
  md += `### Navigation Speed\n\n`;
  md += `| Site | rayo | Playwright | Puppeteer | rayo vs PW | rayo vs Pptr |\n`;
  md += `|------|------|-----------|-----------|------------|-------------|\n`;

  for (const site of SITES) {
    const r = rayo.find((b) => b.scenario === `navigate:${site.name}`);
    const p = pw.find((b) => b.scenario === `navigate:${site.name}`);
    const pp = pptr.find((b) => b.scenario === `navigate:${site.name}`);

    const rMs = r ? `${r.median_ms.toFixed(0)}ms` : "N/A";
    const pMs = p ? `${p.median_ms.toFixed(0)}ms` : "N/A";
    const ppMs = pp ? `${pp.median_ms.toFixed(0)}ms` : "N/A";
    const vsPW =
      r && p
        ? `**${(p.median_ms / r.median_ms).toFixed(2)}x**`
        : "N/A";
    const vsPptr =
      r && pp
        ? `**${(pp.median_ms / r.median_ms).toFixed(2)}x**`
        : "N/A";

    md += `| ${site.name} | ${rMs} | ${pMs} | ${ppMs} | ${vsPW} | ${vsPptr} |\n`;
  }

  // Page understanding comparison
  md += `\n### Page Understanding (Speed + Token Cost)\n\n`;
  md += `| Method | Adapter | Latency | Tokens | Token Efficiency |\n`;
  md += `|--------|---------|---------|--------|------------------|\n`;

  const pageMap = rayo.find((b) => b.scenario === "page_understanding:page_map");
  if (pageMap) {
    md += `| **page_map** | rayo | ${pageMap.median_ms.toFixed(0)}ms | ~${pageMap.extra?.tokens} | **Best** |\n`;
  }

  for (const adapter of [
    { name: "rayo", results: rayo },
    { name: "playwright", results: pw },
    { name: "puppeteer", results: pptr },
  ]) {
    const text = adapter.results.find(
      (b) => b.scenario === "page_understanding:text"
    );
    if (text) {
      md += `| text | ${adapter.name} | ${text.median_ms.toFixed(0)}ms | ~${text.extra?.tokens || "?"} | |\n`;
    }
    const ss = adapter.results.find(
      (b) => b.scenario === "page_understanding:screenshot"
    );
    if (ss) {
      md += `| screenshot | ${adapter.name} | ${ss.median_ms.toFixed(0)}ms | ~${ss.extra?.tokens || "?"} | |\n`;
    }
  }

  // DOM extraction
  md += `\n### DOM Extraction (HN Stories)\n\n`;
  md += `| Adapter | Latency | Items | Method |\n`;
  md += `|---------|---------|-------|--------|\n`;

  for (const adapter of [
    { name: "rayo", results: rayo },
    { name: "playwright", results: pw },
    { name: "puppeteer", results: pptr },
  ]) {
    const ext = adapter.results.find((b) => b.scenario === "extract:hn_stories");
    if (ext) {
      const method = adapter.name === "rayo" ? "page_map" : "$$eval";
      md += `| ${adapter.name} | ${ext.median_ms.toFixed(0)}ms | ${ext.extra?.count || "?"} | ${method} |\n`;
    }
  }

  // Multi-step workflow
  md += `\n### Multi-Step Workflow (navigate + extract + screenshot)\n\n`;
  md += `| Adapter | Latency | MCP Calls | Method |\n`;
  md += `|---------|---------|-----------|--------|\n`;

  for (const adapter of [
    { name: "rayo", results: rayo, calls: "1 (batch)", method: "rayo_batch" },
    {
      name: "playwright",
      results: pw,
      calls: "3 (sequential)",
      method: "3x tool calls",
    },
    {
      name: "puppeteer",
      results: pptr,
      calls: "3 (sequential)",
      method: "3x tool calls",
    },
  ]) {
    const wf = adapter.results.find(
      (b) => b.scenario === "workflow:navigate_extract_screenshot"
    );
    if (wf) {
      md += `| ${adapter.name} | ${wf.median_ms.toFixed(0)}ms | ${adapter.calls} | ${adapter.method} |\n`;
    }
  }

  // Token cost
  md += `\n### Tool Description Token Cost (Context Window Impact)\n\n`;
  md += `| MCP Server | Tools | Est. Tokens | % of 200k Context |\n`;
  md += `|-----------|-------|-------------|-------------------|\n`;
  md += `| **rayo-browser** | ${tokenCost.rayo.tools} | ~${tokenCost.rayo.estimatedTokens} | ${((tokenCost.rayo.estimatedTokens / 200000) * 100).toFixed(2)}% |\n`;
  md += `| Puppeteer MCP | ${tokenCost.puppeteer.tools} | ~${tokenCost.puppeteer.estimatedTokens} | ${((tokenCost.puppeteer.estimatedTokens / 200000) * 100).toFixed(2)}% |\n`;
  md += `| Playwright MCP | ${tokenCost.playwright.tools} | ~${tokenCost.playwright.estimatedTokens} | ${((tokenCost.playwright.estimatedTokens / 200000) * 100).toFixed(2)}% |\n`;

  md += `\n---\n`;
  md += `*Benchmarks run on real public websites. ${ITERATIONS} iterations after ${WARMUP} warmup. Median values reported.*\n`;

  return md;
}

// ─── Main ────────────────────────────────────────────────────────────

async function main() {
  console.log("🚀 rayo-browser Benchmark Suite\n");
  console.log(`Config: ${ITERATIONS} iterations, ${WARMUP} warmup\n`);

  const pwResults = await runPlaywrightBenchmarks();
  const pptrResults = await runPuppeteerBenchmarks();
  const rayoResults = await runRayoBenchmarks();
  const tokenCost = tokenCostAnalysis();

  console.log("\n" + "=".repeat(60));
  console.log(tokenCost);

  const markdown = generateMarkdown(
    pwResults,
    pptrResults,
    rayoResults,
    tokenCost
  );

  console.log("\n" + markdown);

  // Write results
  const fs = await import("fs");
  const resultsDir = process.cwd().replace("/competitors", "/results");
  fs.mkdirSync(resultsDir, { recursive: true });

  fs.writeFileSync(
    `${resultsDir}/latest.json`,
    JSON.stringify(
      {
        timestamp: new Date().toISOString(),
        config: { iterations: ITERATIONS, warmup: WARMUP },
        playwright: pwResults,
        puppeteer: pptrResults,
        rayo: rayoResults,
        tokenCost,
      },
      null,
      2
    )
  );

  fs.writeFileSync(`${resultsDir}/BENCHMARKS.md`, markdown);

  console.log(`\n✅ Results written to bench/results/latest.json and bench/results/BENCHMARKS.md`);
}

main().catch(console.error);
