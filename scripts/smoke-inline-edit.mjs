#!/usr/bin/env node
/**
 * 就地编辑专项验证：新建普通绘本向导三步（方案 / 角色 / 分页）
 * 1) 点“手动修改”后，编辑表单必须替换原展示列表（同一个位置，不是追加在下方）
 * 2) 收起修改后，展示列表显示改后的内容
 * 3) 角色名 / 分页标题的修改必须真的持久化到后端（保存链路）
 * 运行：FRONTEND_BASE_URL=http://127.0.0.1:7100 node scripts/smoke-inline-edit.mjs
 */
import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const FRONTEND_BASE = process.env.FRONTEND_BASE_URL || "http://127.0.0.1:5173";
const API_BASE = process.env.API_BASE_URL || "http://127.0.0.1:8080";
const API_TOKEN = process.env.API_TOKEN || "dev-token";
const CHROME_PATH = process.env.CHROME_EXECUTABLE_PATH || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const stamp = Date.now();
const bookTitle = `就地编辑验证绘本 ${stamp}`;
const PLAN_MARK = "【就地改-方案】";
const ROLE_MARK = "·就地改";
const PAGE_MARK = "·就地改";

let chrome;
let userDataDir;
let cdp;
let cdpPort = 9333;
let workspaceId = "";
let bookId = "";

const watchdog = setTimeout(() => {
  console.error("inline-edit test watchdog timeout");
  shutdown().finally(() => process.exit(2));
}, 1_200_000);

main()
  .then(() => clearTimeout(watchdog))
  .catch(async (error) => {
    clearTimeout(watchdog);
    try {
      console.error(`[debug] current path: ${await evaluate("location.pathname + location.search")}`);
      const text = await evaluate("document.body.innerText");
      console.error(`[debug] visible text: ${String(text).replace(/\s+/g, " ").slice(0, 1000)}`);
    } catch {}
    console.error(error);
    await shutdown();
    process.exit(1);
  });

async function main() {
  console.log("== inline-edit wizard test ==");
  console.log(`FRONTEND_BASE=${FRONTEND_BASE}`);

  await assertApiHealth();
  await startChrome();
  await openTab(`${FRONTEND_BASE}/login`);
  await waitForText("登录绘本工作台");

  console.log("1. login");
  await evaluate("localStorage.clear()");
  await navigate(`${FRONTEND_BASE}/login`);
  await waitForText("登录绘本工作台");
  await fillByLabel("邮箱或手机号", "lin@example.com");
  await fillByLabel("密码", "demo");
  await clickByText("登录");
  await waitForUrl("/dashboard");
  workspaceId = await resolveWorkspaceId();
  console.log(`workspace=${workspaceId}`);

  console.log("2. step 需求：填写并生成绘本方案");
  await navigate(`${FRONTEND_BASE}/app/${workspaceId}/storybooks/new`);
  await waitForText("新建普通绘本");
  await fillInputAt(0, bookTitle);
  await fillInputAt(1, "验证就地编辑与保存链路");
  await clickByText("生成绘本方案");
  await waitForText("绘本方案已生成");
  await waitForText("故事概述");

  console.log("3. 方案步：手动修改必须原地替换展示列表");
  await evaluate("window.scrollTo(0, 0)");
  await sleep(300);
  const listTop = await rectTop(".review-block .review-list");
  if (listTop === null) throw new Error("生成后应展示 review-list");
  await clickByText("手动修改");
  await assertEditorReplacedList(".review-editor", listTop, "plan");
  await fillByLabel("故事概述", `这是就地修改后的故事概述。${PLAN_MARK}`);
  await clickByText("收起修改");
  await waitUntil(async () => {
    const hasList = await exists(".review-block .review-list");
    const hasEditor = await exists(".review-block .review-editor");
    return hasList && !hasEditor;
  }, "收起修改后应恢复展示列表");
  await waitForText(PLAN_MARK);
  console.log("   ok: 方案就地替换 + 收起后展示修改内容");

  console.log("4. 确认方案，创建绘本并进入角色步");
  await clickByText("确认方案，继续角色");
  await waitForText("普通绘本已创建");
  await waitForText("生成角色道具");

  console.log("5. 生成角色：编辑器自动在原位打开");
  await clickByText("生成角色道具");
  await waitForText("角色与道具已生成并写入绘本");
  // 生成完成后组件自动进入编辑态，同样必须是替换而不是追加
  await waitUntil(async () => exists(".review-block .review-editor.role-editor"), "角色编辑器未出现");
  if (await exists(".review-block .review-list")) {
    throw new Error("角色步：编辑态下 review-list 仍显示，说明是追加而不是替换");
  }
  const firstRoleName = await evaluate(`document.querySelector('.role-editor .editable-review-card label input')?.value || ""`);
  if (!firstRoleName) throw new Error("未读到第一个角色名");
  await setFirstControl(".role-editor .editable-review-card label input", `${firstRoleName}${ROLE_MARK}`);
  await clickByText("收起修改");
  await waitForText(`${firstRoleName}${ROLE_MARK}`);
  console.log(`   ok: 角色就地编辑，改名 ${firstRoleName} -> ${firstRoleName}${ROLE_MARK}`);

  console.log("6. 确认角色并生成分页（触发角色保存链路）");
  await clickByText("确认角色，生成分页");
  await waitForText("分页图文已生成并写入绘本");
  await waitUntil(async () => exists(".review-block .review-editor.page-editor"), "分页编辑器未出现");
  if (await exists(".review-block .review-list")) {
    throw new Error("分页步：编辑态下 review-list 仍显示，说明是追加而不是替换");
  }
  const firstPageTitle = await evaluate(`document.querySelector('.page-editor .editable-review-card label input')?.value || ""`);
  await setFirstControl(".page-editor .editable-review-card label input", `${firstPageTitle}${PAGE_MARK}`);
  await clickByText("收起修改");
  await waitForText(`${firstPageTitle}${PAGE_MARK}`);
  console.log(`   ok: 分页就地编辑，第 1 页标题 ${firstPageTitle} -> ${firstPageTitle}${PAGE_MARK}`);

  console.log("7. 确认分页进入详情（触发分页保存链路）");
  await clickByText("确认分页，进入预览");
  await waitForUrl("/storybooks/");
  await waitForText("普通绘本详情");
  bookId = await evaluate("location.pathname.split('/').filter(Boolean).at(-1)");
  console.log(`book=${bookId}`);

  console.log("8. API 校验：修改确实持久化到后端");
  const detail = (await apiGet(`/api/workspaces/${workspaceId}/storybooks/${bookId}`))?.data;
  const roleNames = (detail?.roles || []).map((role) => role.name);
  const pageTitles = (detail?.pages || []).map((page) => page.title);
  if (!roleNames.some((name) => name.includes(ROLE_MARK))) {
    throw new Error(`角色改名未持久化：${roleNames.join(" | ")}`);
  }
  if (!pageTitles.some((title) => title.includes(PAGE_MARK))) {
    throw new Error(`分页标题修改未持久化：${pageTitles.join(" | ")}`);
  }
  console.log(`   ok: 后端角色=${roleNames.join("、")}`);
  console.log(`   ok: 后端分页=${pageTitles.join("、")}`);

  console.log("== inline-edit wizard test ok ==");
  await shutdown();
}

async function rectTop(selector) {
  return evaluate(`(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    return el ? Math.round(el.getBoundingClientRect().top + window.scrollY) : null;
  })()`);
}

async function assertEditorReplacedList(editorSelector, listTop, stepName) {
  await waitUntil(async () => exists(editorSelector), `${stepName}: 编辑器未出现`);
  if (await exists(".review-block .review-list")) {
    throw new Error(`${stepName}: 编辑态下 review-list 仍显示，修改发生在新位置而不是原展示位置`);
  }
  const editorTop = await rectTop(editorSelector);
  if (editorTop === null || Math.abs(editorTop - listTop) > 120) {
    throw new Error(`${stepName}: 编辑器位置(${editorTop})偏离原展示位置(${listTop})过多`);
  }
}

async function exists(selector) {
  return evaluate(`Boolean(document.querySelector(${JSON.stringify(selector)}))`);
}

async function setFirstControl(selector, value) {
  await evaluate(`(${setControlValue.toString()})(document.querySelector(${JSON.stringify(selector)}), ${JSON.stringify(value)})`);
}

async function assertApiHealth() {
  const payload = await apiGet("/api/health", false);
  if (payload.data?.status !== "ok") throw new Error("API health check failed");
}

async function resolveWorkspaceId() {
  const payload = await apiGet("/api/auth/me");
  const workspaces = payload.data?.workspaces || [];
  const personal = workspaces.find((item) => item.type === "personal");
  const target = personal || workspaces[0];
  if (!target?.id) throw new Error("no workspace found");
  return target.id;
}

async function apiGet(path, auth = true) {
  const response = await fetch(`${API_BASE}${path}`, {
    headers: auth ? { Authorization: `Bearer ${API_TOKEN}` } : {},
  });
  if (!response.ok) throw new Error(`${path} failed with ${response.status}`);
  return response.json();
}

async function startChrome() {
  const failures = [];
  for (const port of [9333, 9444, 9555, 9666]) {
    try {
      const preexisting = await fetch(`http://127.0.0.1:${port}/json/version`).catch(() => null);
      if (preexisting?.ok) throw new Error(`port ${port} is already served by another Chrome instance`);
      userDataDir = mkdtempSync(join(tmpdir(), "kindleaf-inline-edit-"));
      chrome = spawn(CHROME_PATH, [
        "--headless=new",
        `--remote-debugging-port=${port}`,
        `--user-data-dir=${userDataDir}`,
        "--disable-gpu",
        "--no-first-run",
        "about:blank",
      ], { stdio: "ignore" });
      await waitUntil(async () => {
        const response = await fetch(`http://127.0.0.1:${port}/json/version`).catch(() => null);
        return Boolean(response?.ok);
      }, `Chrome remote debugging did not start on ${port}`, 30_000);
      cdpPort = port;
      return;
    } catch (err) {
      failures.push(`${port}: ${err instanceof Error ? err.message : String(err)}`);
      await stopChromeProcess();
    }
  }
  throw new Error(`Chrome did not start. Tried ${failures.join("; ")}`);
}

async function openTab(url) {
  const response = await fetch(`http://127.0.0.1:${cdpPort}/json/new?${encodeURIComponent(url)}`, { method: "PUT" });
  if (!response.ok) throw new Error(`failed to create Chrome tab: ${response.status}`);
  const target = await response.json();
  cdp = new CdpClient(target.webSocketDebuggerUrl);
  await cdp.connect();
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");
  await waitForUrl("/login");
}

async function navigate(url) {
  await cdp.send("Page.navigate", { url });
  await waitUntil(async () => (await bodyText()).trim().length > 0, `page did not render: ${url}`);
}

async function evaluate(expression) {
  const result = await cdp.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) {
    throw new Error(result.exceptionDetails.exception?.description || result.exceptionDetails.text || "page evaluation failed");
  }
  return result.result?.value;
}

async function bodyText() {
  return evaluate("document.body?.innerText || ''");
}

async function currentUrl() {
  return evaluate("location.href");
}

async function waitForText(text) {
  await waitUntil(async () => (await bodyText()).includes(text), `text not found: ${text}`);
}

async function waitForUrl(fragment) {
  await waitUntil(async () => (await currentUrl()).includes(fragment), `url did not include: ${fragment}`);
}

async function fillByLabel(label, value) {
  await evaluate(`(${setControlValue.toString()})((() => {
    const label = [...document.querySelectorAll('label')].find((item) => item.innerText.includes(${JSON.stringify(label)}));
    return label?.querySelector('input, textarea, select');
  })(), ${JSON.stringify(value)})`);
}

async function fillInputAt(index, value) {
  await evaluate(`(${setControlValue.toString()})([...document.querySelectorAll('input')][${index}], ${JSON.stringify(value)})`);
}

async function clickByText(text) {
  await evaluate(`(() => {
    const candidates = [...document.querySelectorAll('button, a')].filter((item) => !item.disabled);
    const el = candidates.find((item) => item.innerText.trim() === ${JSON.stringify(text)})
      || candidates.find((item) => item.innerText.includes(${JSON.stringify(text)}));
    if (!el) throw new Error('click target not found: ${text}');
    el.click();
  })()`);
}

function setControlValue(control, value) {
  if (!control) throw new Error("form control not found");
  const descriptor = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(control), "value");
  descriptor?.set?.call(control, value);
  control.dispatchEvent(new Event("input", { bubbles: true }));
  control.dispatchEvent(new Event("change", { bubbles: true }));
}

async function waitUntil(check, message, timeoutMs = 240_000) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    if (await check()) return;
    await sleep(150);
  }
  throw new Error(message);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function shutdown() {
  if (cdp) {
    cdp.close();
    cdp = null;
  }
  await stopChromeProcess();
  process.exit(0);
}

async function stopChromeProcess() {
  if (chrome && !chrome.killed) chrome.kill("SIGTERM");
  chrome = null;
  if (userDataDir) {
    rmSync(userDataDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
    userDataDir = null;
  }
}

class CdpClient {
  constructor(url) {
    this.url = url;
    this.nextId = 1;
    this.pending = new Map();
  }

  connect() {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(this.url);
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
      this.ws.addEventListener("message", (event) => {
        const payload = JSON.parse(event.data);
        if (!payload.id) return;
        const callbacks = this.pending.get(payload.id);
        if (!callbacks) return;
        this.pending.delete(payload.id);
        if (payload.error) {
          callbacks.reject(new Error(payload.error.message));
        } else {
          callbacks.resolve(payload.result);
        }
      });
    });
  }

  send(method, params = {}) {
    const id = this.nextId++;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });
  }

  close() {
    this.ws?.close();
  }
}
