#!/usr/bin/env node

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const rootDir = new URL("..", import.meta.url).pathname.replace(/\/$/, "");
const sourceDir = join(rootDir, "frontend", "src");
const strict = process.argv.includes("--strict");

const checks = [
  {
    label: "data/mock import",
    pattern: /from\s+["'][^"']*data\/mock["']/g,
    risk: "review",
    note: "确认该页面在 VITE_USE_API=true 时不会用 mock 解释业务数据。",
  },
  {
    label: "demo token",
    pattern: /demo-token/g,
    risk: "high",
    note: "API 模式下不能把 demo-token 当作真实公开链接或邀请。",
  },
  {
    label: "fixed mock route id",
    pattern: /\b(?:school-1|school-2|personal-1|storybook-1)\b/g,
    risk: "review",
    note: "确认仅用于非 API 原型分支、文案示例或 workspace alias。",
  },
  {
    label: "mock feedback copy",
    pattern: /mock\s*反馈|mock\s*状态|mock\s*结果|当前为 mock|模拟完成/g,
    risk: "low",
    note: "确认 API 模式下不会展示伪成功反馈。",
  },
];

function collectFiles(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      return collectFiles(path);
    }
    if (/\.(tsx?|jsx?)$/.test(name)) {
      return [path];
    }
    return [];
  });
}

function lineNumber(source, index) {
  return source.slice(0, index).split("\n").length;
}

function lineAt(source, number) {
  return source.split("\n")[number - 1]?.trim() || "";
}

function nearbyApiGuard(source, number) {
  const lines = source.split("\n");
  const start = Math.max(0, number - 16);
  const end = Math.min(lines.length, number + 4);
  const context = lines.slice(start, end).join("\n");
  if (/!\s*shouldUseApi|shouldUseApi\s*\?/.test(context)) {
    return "guarded";
  }
  if (/shouldUseApi/.test(context)) {
    return "mentions-api-mode";
  }
  return "unguarded";
}

const findings = [];

for (const file of collectFiles(sourceDir)) {
  const source = readFileSync(file, "utf8");
  for (const check of checks) {
    for (const match of source.matchAll(check.pattern)) {
      const line = lineNumber(source, match.index ?? 0);
      findings.push({
        ...check,
        file: relative(rootDir, file),
        line,
        text: match[0],
        guard: check.label === "demo token" || check.label === "fixed mock route id" || check.label === "mock feedback copy"
          ? nearbyApiGuard(source, line)
          : undefined,
        sourceLine: lineAt(source, line),
      });
    }
  }
}

const grouped = findings.reduce((acc, finding) => {
  const key = finding.label;
  acc[key] ||= [];
  acc[key].push(finding);
  return acc;
}, {});

console.log("# Frontend mock usage audit");
console.log(`scanned=${relative(rootDir, sourceDir)}`);
console.log(`findings=${findings.length}`);

for (const check of checks) {
  const items = grouped[check.label] || [];
  console.log(`\n## ${check.label} (${items.length})`);
  if (items.length === 0) {
    console.log("none");
    continue;
  }
  console.log(`risk=${check.risk}`);
  console.log(`note=${check.note}`);
  for (const item of items) {
    const guard = item.guard ? ` [${item.guard}]` : "";
    const source = item.sourceLine ? ` :: ${item.sourceLine}` : "";
    console.log(`- ${item.file}:${item.line}${guard} ${item.text}${source}`);
  }
}

const allowedFixedIdFiles = new Set([
  "frontend/src/data/mock.ts",
  "frontend/src/utils/workspace.ts",
]);

const strictFailures = findings.filter((finding) => {
  if (finding.label === "demo token") {
    return finding.guard === "unguarded";
  }
  if (finding.label === "fixed mock route id") {
    return finding.guard === "unguarded" && !allowedFixedIdFiles.has(finding.file);
  }
  if (finding.label === "mock feedback copy") {
    return finding.guard === "unguarded";
  }
  return false;
});

if (strict) {
  if (strictFailures.length > 0) {
    console.error(`\nstrict failed: ${strictFailures.length} unguarded demo token, fixed mock id, or mock feedback usage(s)`);
    for (const failure of strictFailures) {
      console.error(`- ${failure.file}:${failure.line} ${failure.text}`);
    }
    process.exit(1);
  }
  console.log("\nstrict ok: no unguarded demo-token, fixed mock id outside allowed prototype files, or mock feedback");
}
