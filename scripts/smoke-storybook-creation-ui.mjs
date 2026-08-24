#!/usr/bin/env node
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const FRONTEND_BASE = process.env.FRONTEND_BASE_URL || "http://127.0.0.1:5179";
const API_BASE = process.env.API_BASE_URL || "http://127.0.0.1:8082";
const CHROME_PATH = process.env.CHROME_EXECUTABLE_PATH || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const REQUESTED_CDP_PORT = process.env.CDP_PORT ? Number(process.env.CDP_PORT) : null;
const DB_CONTAINER = process.env.DB_CONTAINER || "kindleaf-postgres";
const DB_USER = process.env.DB_USER || "postgres";
const DB_NAME = process.env.DB_NAME || "";
const PNG_BASE64 = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4cGnfsxNbGCAUAEWMCcWN1afmAAAAAElFTkSuQmCC";

let chrome;
let userDataDir;
let cdp;
let cdpPort = REQUESTED_CDP_PORT || 9733;

const watchdog = setTimeout(() => {
  console.error("personalized UI smoke watchdog timeout");
  shutdown().finally(() => process.exit(2));
}, 240_000);

main()
  .then(async () => {
    clearTimeout(watchdog);
    await shutdown();
  })
  .catch(async (error) => {
    clearTimeout(watchdog);
    try {
      console.error(`[debug] url: ${await currentUrl()}`);
      console.error(`[debug] text: ${(await bodyText()).replace(/\s+/g, " ").slice(0, 1200)}`);
    } catch {}
    console.error(error);
    await shutdown();
    process.exit(1);
  });

async function main() {
  console.log("== Kindleaf personalized storybook UI smoke ==");
  console.log(`FRONTEND_BASE=${FRONTEND_BASE}`);
  console.log(`API_BASE=${API_BASE}`);

  await assertApiHealth();
  await startChrome();
  await openTab(`${FRONTEND_BASE}/login`);

  console.log("1. login");
  await waitForText("登录绘本工作台");
  await evaluate("localStorage.clear()");
  await fillByLabel("邮箱或手机号", "lin@example.com");
  await fillByLabel("密码", "demo");
  await clickByText("登录");
  await waitForText("我的工作台");

  const token = await evaluate("localStorage.getItem('kindleaf_token')");
  const workspaces = await apiGet("/api/workspaces", token);
  const workspace = workspaces.data.find((item) => item.type === "school" && item.role === "school_admin") || workspaces.data[0];
  if (!workspace?.id) throw new Error("school workspace not found");

  console.log("2. create personalized draft");
  await navigate(`${FRONTEND_BASE}/app/${workspace.id}/storybooks/personalized/new`);
  await waitForText("创建专属绘本");
  await clickByText("我有一个想做给孩子的故事");
  await waitForText("想做一本怎样的专属绘本？");
  await fillByLabel("这本绘本送给谁", "乐乐");
  await fillByLabel("故事想法", "乐乐");
  await waitForText("还差 6 个字符");
  await fillByLabel("故事想法", "给乐乐做一本爸爸和小汽车一起学习轮流等待的专属绘本。");
  await waitForText("想法长度足够了");
  await clickByText("看看故事怎么讲");
  await waitForText("对象与素材");
  await waitForText("把真实照片转成绘本参考");

  console.log("3. upload photo through UI");
  await uploadFirstFileInput("ui-smoke-reference.png", "image/png", PNG_BASE64);
  await waitForText("人物照片 1");
  await waitForText("待确认用途");

  console.log("4. set usage and confirm visual reference");
  await fillByLabel("这是谁？", "爸爸");
  await clickByText("主角");
  await waitForText("确认参考");
  await clickByText("确认参考");
  await waitForText("照片素材已准备好");
  await waitForText("爸爸");
  await waitForText("已确认");

  console.log("5. refresh and restore session assets");
  await cdp.send("Page.reload", { ignoreCache: true });
  await waitUntil(async () => (await bodyText()).trim().length > 0, "page did not render after reload");
  await waitUntil(
    async () => (await pageHasText("继续上次创作")) || (await pageHasText("照片素材已准备好")),
    "restored draft prompt or session content did not appear",
  );
  if (await pageHasText("继续上次创作")) {
    await clickByText("继续上次创作");
  }
  await waitForText("照片素材已准备好");
  await waitForText("爸爸");
  await waitForText("主角 · 已确认");
  const personalizedContextBadges = await evaluate("[...document.querySelectorAll('.personalized-context .badge')].map((item) => item.innerText.trim())");
  const dadMentions = personalizedContextBadges.filter((item) => item === "爸爸").length;
  if (dadMentions !== 1) {
    throw new Error(`expected one personalized content badge for 爸爸, got ${dadMentions}: ${personalizedContextBadges.join(', ')}`);
  }
  await clickByText("移除");
  await waitForText("照片已从本次创作移除");
  await waitForText("重新预览");
  await waitForText("不用这张照片继续");
  await waitForText("取消本次制作");
  await clickByText("重新预览");
  await waitForText("这个故事想怎样讲？");
  await waitForElementCount(".direction-card", 3);
  const directOutlineVisibleBeforeDirection = await pageHasText("故事会这样展开") || (await evaluate("document.querySelectorAll('.outline-row').length")) > 0;
  if (directOutlineVisibleBeforeDirection) {
    throw new Error("direct creation outline should not appear before selecting a story direction");
  }
  await clickFirstElement(".direction-card");
  await waitForClickableText("按这个故事继续");
  await clickByText("按这个故事继续");
  await waitForText("故事会这样展开");
  await waitForElementCount(".outline-row", 6);
  await clickByText("换一个故事走向");
  await waitForText("这个故事想怎样讲？");
  await waitForElementCount(".direction-card", 3);
  const staleOutlineVisibleAfterDirectionRefresh = await pageHasText("故事会这样展开")
    || (await evaluate("document.querySelectorAll('.outline-row').length")) > 0;
  if (staleOutlineVisibleAfterDirectionRefresh) {
    throw new Error("direction refresh should replace the persisted outline before selecting a new direction");
  }
  await clickFirstElement(".direction-card");
  await waitForClickableText("按这个故事继续");
  await clickByText("按这个故事继续");
  await waitForText("故事会这样展开");
  await waitForElementCount(".outline-row", 6);

  console.log("6. verify direct creation evidence on review page");
  let directEvidenceBook = await apiPost(`/api/workspaces/${workspace.id}/storybooks`, token, {
    title: `UI Smoke 直接创作证据 ${Date.now()}`,
    age_group: "4-5 岁",
    use_scene: "家庭共读",
    teaching_goal: "验证直接创作产物证据展示",
    cover_tone: "温暖水彩",
  }).then((response) => response.data);
  const directPagesJob = await apiPost(`/api/workspaces/${workspace.id}/generation-jobs`, token, {
    job_type: "storybook_pages",
    storybook_id: directEvidenceBook.id,
    input_json: { page_count: 2 },
  }).then((response) => response.data);
  await waitForApiJob(workspace.id, directPagesJob.id, token);
  directEvidenceBook = await apiGet(`/api/workspaces/${workspace.id}/storybooks/${directEvidenceBook.id}`, token).then((response) => response.data);
  const directPageEvidence = (directEvidenceBook.pages || []).map((page) => ({
    page_number: page.page_number,
    title: page.title || `直接创作第 ${page.page_number} 页`,
    asset_reference_ids: ["ui-smoke-direct-asset-ref"],
    asset_references: [
      {
        asset_reference_id: "ui-smoke-direct-asset-ref",
        visual_reference_id: "ui-smoke-direct-visual-ref",
        display_name: "爸爸",
      },
    ],
  }));
  const directCreationPlan = {
    entry_type: "direct_create",
    creation_session_id: "ui-smoke-direct-session",
    generation_job_id: "ui-smoke-direct-job",
    selected_direction: { title: "爸爸和小汽车学会等待" },
    outline: {
      pages: directPageEvidence.map((item) => ({
        page_number: item.page_number,
        title: item.title,
      })),
    },
    asset_references: [
      {
        id: "ui-smoke-direct-asset-ref",
        display_name: "爸爸",
        usage: "main_character",
        kind: "person",
        visual_reference: {
          id: "ui-smoke-direct-visual-ref",
          status: "confirmed",
        },
      },
    ],
    page_evidence: directPageEvidence,
  };
  await psql(`update storybooks
set source = 'creation_session',
    status = 'exportable',
    teacher_review_status = 'confirmed',
    customization_plan = ${jsonbLiteral(directCreationPlan)},
    updated_at = now()
where id = '${directEvidenceBook.id}'`);
  await psql(`update storybook_pages set status = 'ready' where storybook_id = '${directEvidenceBook.id}'`);
  await navigate(`${FRONTEND_BASE}/app/${workspace.id}/storybooks/${directEvidenceBook.id}/review?result=direct-create`);
  await waitForText("本次创作证据");
  await waitForText("已关联直接创作冻结输入");
  await waitForText("直接创作");
  await waitForText("爸爸和小汽车学会等待");
  await waitForText("照片素材");
  await waitForText("爸爸");
  await waitForText("ui-smoke-direct-visual-ref");
  await waitForText("页级证据");

  console.log("7. cancel direct creation run through UI");
  const cancelSession = await apiPost(`/api/workspaces/${workspace.id}/storybook-creation-sessions`, token, {
    quick_idea: "给乐乐做一本可以取消制作的烟雾测试故事。",
    page_count: 6,
  }).then((response) => response.data);
  const cancelJob = await apiPost(`/api/workspaces/${workspace.id}/generation-jobs`, token, {
    job_type: "storybook_pages",
    storybook_id: directEvidenceBook.id,
    input_json: { page_count: 1, smoke: "direct-creation-cancel" },
  }).then((response) => response.data);
  await psql(`update generation_jobs
set status = 'running',
    locked_by = null,
    locked_at = null,
    finished_at = null
where id = '${cancelJob.id}'`);
  await psql(`update storybook_creation_sessions
set status = 'generating',
    last_job_id = '${cancelJob.id}',
    generation_summary_json = ${jsonbLiteral({
      text_generation_status: "succeeded",
      image_generation_status: "generating",
      quality_notice: null,
      recoverable_actions: ["cancel"],
    })},
    updated_at = now()
where id = '${cancelSession.id}'`);
  await navigate(`${FRONTEND_BASE}/app/${workspace.id}/storybooks/personalized/new`);
  await waitForText("继续上次创作");
  await clickByText("继续上次创作");
  await waitForText("正在把故事画出来");
  await waitForText("准备故事");
  await waitForText("完成文字");
  await waitForText("绘制画面");
  await waitForText("等待检查");
  await waitForText("可以先离开，稍后会按本次制作进度恢复。");
  await clickByText("取消本次制作");
  await waitForUrl(`/app/${workspace.id}/storybooks`);
  await waitUntil(async () => {
    const restored = await apiGet(`/api/workspaces/${workspace.id}/storybook-creation-sessions/${cancelSession.id}`, token).then((response) => response.data);
    return restored.status === "abandoned";
  }, "direct creation session was not abandoned after UI cancel");
  await waitUntil(async () => {
    const job = await apiGet(`/api/workspaces/${workspace.id}/generation-jobs/${cancelJob.id}`, token).then((response) => response.data);
    return job.status === "canceled";
  }, "direct creation generation job was not canceled after UI cancel");

  console.log("8. enforce photo upload limit in UI");
  const limitSession = await apiPost(`/api/workspaces/${workspace.id}/storybook-creation-sessions`, token, {
    quick_idea: "给乐乐做一本验证照片上限的烟雾测试故事。",
    page_count: 6,
  }).then((response) => response.data);
  for (let index = 0; index < 5; index += 1) {
    await apiUploadStorybookAsset(workspace.id, limitSession.id, token, {
      kind: "person",
      filename: `ui-smoke-limit-${index}.png`,
      contentType: "image/png",
      idempotencyKey: `ui-smoke-limit-${Date.now()}-${index}`,
    });
  }
  await navigate(`${FRONTEND_BASE}/app/${workspace.id}/storybooks/personalized/new`);
  await waitForText("继续上次创作");
  await clickByText("继续上次创作");
  await waitForText("照片已达到上限");
  await waitForText("最多添加 5 张使用中的真实照片");
  await waitForText("管理照片");
  const uploadDisabledAtLimit = await evaluate("(() => { const input = document.querySelector('input[type=\"file\"]'); const buttons = [...document.querySelectorAll('button')]; return Boolean(input?.disabled) && buttons.some((button) => button.innerText.includes('管理照片') && button.getAttribute('aria-disabled') === 'true'); })()");
  if (!uploadDisabledAtLimit) {
    throw new Error("photo upload controls should be disabled after reaching the 5-photo limit");
  }

  console.log("9. derive a custom storybook from existing source");
  let sourceBook = await apiPost(`/api/workspaces/${workspace.id}/storybooks`, token, {
    title: `UI Smoke 来源绘本 ${Date.now()}`,
    age_group: "4-5 岁",
    use_scene: "规则引导",
    teaching_goal: "验证基于已有绘本创作专属版本",
    cover_tone: "温暖水彩",
  }).then((response) => response.data);
  const rolesJob = await apiPost(`/api/workspaces/${workspace.id}/generation-jobs`, token, {
    job_type: "storybook_roles",
    storybook_id: sourceBook.id,
    input_json: { title: sourceBook.title, teacher_name: "UI Smoke 老师" },
  }).then((response) => response.data);
  await waitForApiJob(workspace.id, rolesJob.id, token);
  const pagesJob = await apiPost(`/api/workspaces/${workspace.id}/generation-jobs`, token, {
    job_type: "storybook_pages",
    storybook_id: sourceBook.id,
    input_json: { page_count: 6 },
  }).then((response) => response.data);
  await waitForApiJob(workspace.id, pagesJob.id, token);
  sourceBook = await apiGet(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}`, token).then((response) => response.data);
  const sourcePage = sourceBook.pages?.[0];
  const sourceRole = sourceBook.roles?.[0];
  if (!sourcePage?.id || !sourceRole?.id) throw new Error("generated source storybook missing page or role");
  for (const page of sourceBook.pages) {
    await apiPatch(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/pages/${page.id}`, token, { status: "ready" });
  }
  await apiPatch(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/pages/${sourcePage.id}`, token, {
    title: "学会等待",
    body: "孩子和老师一起练习轮流等待。",
    illustration_prompt: "明亮教室，老师和孩子围坐在地毯上练习等待",
    status: "ready",
  });
  await apiPatch(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/roles/${sourceRole.id}`, token, {
    name: "老师",
    role_type: "teacher",
    appearance: "温和、稳定、会蹲下来和孩子说话",
    story_function: "帮助孩子理解等待和轮流",
    needs_consistency: false,
  });
  sourceBook = await apiGet(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}`, token).then((response) => response.data);
  for (const role of sourceBook.roles) {
    await apiPatch(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/roles/${role.id}`, token, { needs_consistency: false });
  }
  sourceBook = await apiGet(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}`, token).then((response) => response.data);
  const rolesStillRequiringReference = sourceBook.roles?.filter((role) => role.needs_consistency) || [];
  if (rolesStillRequiringReference.length) {
    throw new Error(`source roles should not require reference: ${JSON.stringify(rolesStillRequiringReference)}`);
  }
  for (const page of sourceBook.pages) {
    await apiPatch(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/pages/${page.id}`, token, { status: "ready" });
  }
  await apiPatch(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}`, token, { teacher_review_status: "confirmed" });
  sourceBook = await apiPatch(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}`, token, { status: "exportable" }).then((response) => response.data);
  const children = await apiGet(`/api/workspaces/${workspace.id}/children?limit=20`, token);
  const child = children.data.find((item) => item.status !== "archived") || children.data[0];
  if (!child?.id) throw new Error("child profile not found");
  await navigate(`${FRONTEND_BASE}/app/${workspace.id}/storybooks/${sourceBook.id}/customize?childId=${child.id}`);
  await waitForUrl(`/storybooks/personalized/new`);
  const legacyCustomizeRedirectUrl = await currentUrl();
  if (!legacyCustomizeRedirectUrl.includes(`sourceStorybookId=${sourceBook.id}`) || !legacyCustomizeRedirectUrl.includes(`childId=${child.id}`)) {
    throw new Error(`legacy customize redirect should preserve source and child context: ${legacyCustomizeRedirectUrl}`);
  }
  await waitForText("基于已有绘本创作专属版本");
  await waitForText(sourceBook.title);
  await waitUntil(
    async () => evaluate(`([...document.querySelectorAll('.recipient-card.selected')].some((item) => item.innerText.includes(${JSON.stringify(child.nickname)})))`),
    "legacy customize childId should preselect the target child",
  );
  const batchChild = await apiPost(`/api/workspaces/${workspace.id}/children`, token, {
    nickname: `UI批量儿童${Date.now()}`,
    age_group: "4-5 岁",
    classroom: "小一班",
    interests: ["小汽车"],
    traits: ["愿意尝试"],
    focus: "轮流等待",
  }).then((response) => response.data);
  const runProbeChild = await apiPost(`/api/workspaces/${workspace.id}/children`, token, {
    nickname: `UI运行记录儿童${Date.now()}`,
    age_group: "4-5 岁",
    classroom: "小一班",
    interests: ["积木"],
    traits: ["认真观察"],
    focus: "表达需求",
  }).then((response) => response.data);
  await apiExpectError(
    "POST",
    `/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/derive-custom`,
    token,
    {
      child_id: child.id,
      intensity: "standard",
      customization_plan: sourceCustomizationPlan(sourceBook, [child.id], "single"),
    },
    400,
    "validation_error",
  );
  await apiExpectError(
    "POST",
    `/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/derive-custom`,
    token,
    {
      child_id: child.id,
      intensity: "standard",
      primary_material: "profile",
      customization_plan: {
        entry_type: "from_storybook",
        mode: "single",
        source_snapshot: {
          storybook_id: sourceBook.id,
          updated_at: "2000-01-01 00:00",
          page_count: sourceBook.pages.length,
          page_ids: sourceBook.pages.map((page) => page.id),
        },
        page_plan: [{ page_number: 1, decision: "keep" }],
      },
    },
    409,
    "source_revision_conflict",
  );
  await apiExpectError(
    "POST",
    `/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/derive-custom-batch`,
    token,
    {
      child_ids: [runProbeChild.id],
      intensity: "quick",
      customization_plan: sourceCustomizationPlan(sourceBook, [runProbeChild.id], "single"),
      material_choices: { [runProbeChild.id]: "name_only" },
    },
    409,
    "plan_mode_mismatch",
  );
  sourceBook = await apiPatch(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}`, token, { status: "editing" }).then((response) => response.data);
  await apiExpectError(
    "POST",
    `/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/derive-custom-batch`,
    token,
    {
      child_ids: [runProbeChild.id],
      intensity: "quick",
      customization_plan: sourceCustomizationPlan(sourceBook, [runProbeChild.id], "batch"),
      material_choices: { [runProbeChild.id]: "name_only" },
    },
    409,
    "state_conflict",
  );
  sourceBook = await apiPatch(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}`, token, { status: "exportable" }).then((response) => response.data);
  const runProbePlan = sourceCustomizationPlan(sourceBook, [runProbeChild.id], "batch");
  const runProbeResult = await apiPost(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/derive-custom-batch`, token, {
    child_ids: [runProbeChild.id],
    intensity: "quick",
    customization_plan: runProbePlan,
    material_choices: { [runProbeChild.id]: "name_only" },
  }).then((response) => response.data);
  const runProbeDuplicateResult = await apiPost(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/derive-custom-batch`, token, {
    child_ids: [runProbeChild.id],
    intensity: "quick",
    customization_plan: runProbePlan,
    material_choices: { [runProbeChild.id]: "name_only" },
  }).then((response) => response.data);
  if (
    runProbeDuplicateResult.run_id !== runProbeResult.run_id
    || runProbeDuplicateResult.storybooks?.[0]?.id !== runProbeResult.storybooks?.[0]?.id
  ) {
    throw new Error(`duplicate batch customization should reuse existing run and output: ${JSON.stringify({ first: runProbeResult, duplicate: runProbeDuplicateResult })}`);
  }
  if (!runProbeResult.run_id || !runProbeResult.items?.[0]?.run_item_id) {
    throw new Error(`batch customization run ids were not returned: ${JSON.stringify(runProbeResult)}`);
  }
  if (
    runProbeResult.storybooks?.[0]?.customization_run_id !== runProbeResult.run_id
    || runProbeResult.storybooks?.[0]?.customization_run_item_id !== runProbeResult.items[0].run_item_id
  ) {
    throw new Error(`custom output storybook did not carry run ids: ${JSON.stringify(runProbeResult.storybooks?.[0])}`);
  }
  const runProbeRun = await apiGet(`/api/workspaces/${workspace.id}/storybook-customization-runs/${runProbeResult.run_id}`, token).then((response) => response.data);
  if (
    runProbeRun.id !== runProbeResult.run_id
    || runProbeRun.mode !== "batch"
    || runProbeRun.status !== "succeeded"
    || runProbeRun.requested_count !== 1
    || runProbeRun.succeeded_count !== 1
    || runProbeRun.items?.[0]?.id !== runProbeResult.items[0].run_item_id
    || runProbeRun.items[0].target_child_id !== runProbeChild.id
    || runProbeRun.items[0].primary_material !== "name_only"
    || runProbeRun.items[0].generation_input_snapshot?.target_child_id !== runProbeChild.id
    || runProbeRun.items[0].generation_input_snapshot?.target_child_nickname !== runProbeChild.nickname
  ) {
    throw new Error(`batch customization run restore mismatch: ${JSON.stringify(runProbeRun)}`);
  }
  await psql(`update storybook_customization_run_items
set status = 'running',
    output_storybook_id = null,
    completed_at = null,
    updated_at = now()
where id = '${runProbeResult.items[0].run_item_id}'`);
  await psql(`update storybook_customization_runs
set status = 'running',
    succeeded_count = 0,
    failed_count = 0,
    failure_reason = null,
    completed_at = null,
    updated_at = now()
where id = '${runProbeResult.run_id}'`);
  const activeRunDuplicateResult = await apiPost(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/derive-custom-batch`, token, {
    child_ids: [runProbeChild.id],
    intensity: "quick",
    customization_plan: runProbePlan,
    material_choices: { [runProbeChild.id]: "name_only" },
  }).then((response) => response.data);
  if (
    activeRunDuplicateResult.run_id !== runProbeResult.run_id
    || activeRunDuplicateResult.created_count !== 0
    || activeRunDuplicateResult.storybooks?.length !== 0
    || activeRunDuplicateResult.items?.[0]?.run_item_id !== runProbeResult.items[0].run_item_id
    || activeRunDuplicateResult.items?.[0]?.status !== "running"
  ) {
    throw new Error(`active batch customization run should be restored instead of duplicated: ${JSON.stringify(activeRunDuplicateResult)}`);
  }
  await psql(`update storybook_customization_run_items
set status = 'succeeded',
    output_storybook_id = '${runProbeResult.storybooks[0].id}',
    failure_reason = null,
    completed_at = now(),
    updated_at = now()
where id = '${runProbeResult.items[0].run_item_id}'`);
  await psql(`update storybook_customization_runs
set status = 'succeeded',
    succeeded_count = 1,
    failed_count = 0,
    failure_reason = null,
    completed_at = now(),
    updated_at = now()
where id = '${runProbeResult.run_id}'`);
  const runProbeBook = await latestCustomBookForChild(workspace.id, runProbeChild.id, token);
  assertBatchCustomBook(runProbeBook, sourceBook, runProbeChild.id, "name_only");
  await psql(`update storybook_customization_run_items
set status = 'failed',
    failure_reason = '缺少主素材：smoke simulated run item failure',
    output_storybook_id = null,
    completed_at = now(),
    updated_at = now()
where id = '${runProbeResult.items[0].run_item_id}'`);
  await psql(`update storybook_customization_runs
set status = 'failed',
    succeeded_count = 0,
    failed_count = 1,
    failure_reason = 'smoke simulated batch partial failure',
    completed_at = now(),
    updated_at = now()
where id = '${runProbeResult.run_id}'`);
  await psql(`update storybooks set status = 'editing' where id = '${sourceBook.id}'`);
  await apiExpectError(
    "POST",
    `/api/workspaces/${workspace.id}/storybook-customization-runs/${runProbeResult.run_id}/items/${runProbeResult.items[0].run_item_id}/retry`,
    token,
    {},
    409,
    "state_conflict",
  );
  await psql(`update storybooks set status = 'exportable' where id = '${sourceBook.id}'`);
  await navigate(`${FRONTEND_BASE}/app/${workspace.id}/storybooks/personalized/new?sourceStorybookId=${sourceBook.id}&sourceRunId=${runProbeResult.run_id}`);
  await waitForText("批量结果");
  await waitForText("需补素材");
  await waitForText("缺少主素材");
  await waitForText("重试");
  await waitForText("放弃");
  const retriedRun = await apiPost(
    `/api/workspaces/${workspace.id}/storybook-customization-runs/${runProbeResult.run_id}/items/${runProbeResult.items[0].run_item_id}/retry`,
    token,
    {},
  ).then((response) => response.data);
  const retriedItem = retriedRun.items?.find((item) => item.id === runProbeResult.items[0].run_item_id);
  if (
    retriedRun.status !== "succeeded"
    || retriedRun.succeeded_count !== 1
    || retriedRun.failed_count !== 0
    || retriedItem?.status !== "succeeded"
    || !retriedItem?.output_storybook_id
  ) {
    throw new Error(`failed run item retry did not restore expected state: ${JSON.stringify(retriedRun)}`);
  }
  await psql(`update storybook_customization_run_items
set status = 'failed',
    failure_reason = 'smoke simulated run item failure after retry',
    output_storybook_id = null,
    completed_at = now(),
    updated_at = now()
where id = '${runProbeResult.items[0].run_item_id}'`);
  await psql(`update storybook_customization_runs
set status = 'failed',
    succeeded_count = 0,
    failed_count = 1,
    failure_reason = 'smoke simulated batch partial failure after retry',
    completed_at = now(),
    updated_at = now()
where id = '${runProbeResult.run_id}'`);
  const abandonedRun = await apiPost(
    `/api/workspaces/${workspace.id}/storybook-customization-runs/${runProbeResult.run_id}/items/${runProbeResult.items[0].run_item_id}/abandon`,
    token,
    {},
  ).then((response) => response.data);
  if (
    abandonedRun.status !== "canceled"
    || abandonedRun.failed_count !== 0
    || abandonedRun.items?.[0]?.status !== "canceled"
  ) {
    throw new Error(`failed run item abandon did not restore expected state: ${JSON.stringify(abandonedRun)}`);
  }
  await apiExpectError(
    "POST",
    `/api/workspaces/${workspace.id}/storybook-customization-runs/${runProbeResult.run_id}/items/${runProbeResult.items[0].run_item_id}/retry`,
    token,
    {},
    409,
    "run_item_not_retryable",
  );
  const rerunAfterAbandon = await apiPost(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/derive-custom-batch`, token, {
    child_ids: [runProbeChild.id],
    intensity: "quick",
    customization_plan: runProbePlan,
    material_choices: { [runProbeChild.id]: "name_only" },
  }).then((response) => response.data);
  if (
    !rerunAfterAbandon.run_id
    || rerunAfterAbandon.run_id === runProbeResult.run_id
    || !rerunAfterAbandon.storybooks?.[0]?.id
  ) {
    throw new Error(`abandoned customization run should not block a new run: ${JSON.stringify({ abandoned: runProbeResult.run_id, rerun: rerunAfterAbandon })}`);
  }
  const rerunAfterAbandonRun = await apiGet(`/api/workspaces/${workspace.id}/storybook-customization-runs/${rerunAfterAbandon.run_id}`, token).then((response) => response.data);
  if (
    rerunAfterAbandonRun.status !== "succeeded"
    || rerunAfterAbandonRun.items?.[0]?.status !== "succeeded"
    || rerunAfterAbandonRun.items?.[0]?.output_storybook_id !== rerunAfterAbandon.storybooks[0].id
  ) {
    throw new Error(`abandoned customization rerun did not finish as a new succeeded run: ${JSON.stringify(rerunAfterAbandonRun)}`);
  }
  await navigate(`${FRONTEND_BASE}/app/${workspace.id}/storybooks/personalized/new?sourceStorybookId=${sourceBook.id}`);
  await waitForText("基于已有绘本创作专属版本");
  await waitForText(sourceBook.title);
  await waitForText("可以补充照片素材");
  await waitUntil(
    async () => evaluate(`([...document.querySelectorAll('button')].some((item) => item.innerText.includes('添加照片') && item.getAttribute('aria-disabled') !== 'true' && !item.disabled))`),
    "source-storybook photo upload should be available after source asset session is prepared",
  );
  await uploadFirstFileInput("source-smoke-reference.png", "image/png", PNG_BASE64);
  await waitForText("人物照片 1");
  await waitForText("待确认用途");
  await fillByLabel("这是谁？", "彩虹书包");
  await clickByText("主角");
  await waitForText("确认参考");
  await clickByText("确认参考");
  await waitForText("照片素材已准备好");
  await waitForText("已确认");
  const latestDraftAfterSourceAssets = await apiGet(`/api/workspaces/${workspace.id}/storybook-creation-sessions/latest`, token);
  if (
    latestDraftAfterSourceAssets.data?.entry_type === "from_storybook_assets"
    || latestDraftAfterSourceAssets.data?.source_storybook_id === sourceBook.id
  ) {
    throw new Error(`source-storybook asset session should not pollute latest direct creation draft: ${JSON.stringify(latestDraftAfterSourceAssets.data)}`);
  }
  const sourceAssetSessionId = await psql(`select id from storybook_creation_sessions where workspace_id = '${workspace.id}' and source_storybook_id = '${sourceBook.id}' and entry_type = 'from_storybook_assets' order by updated_at desc limit 1`);
  if (!sourceAssetSessionId) {
    throw new Error("source-storybook asset session was not persisted");
  }
  const sourceAssetReferenceId = await psql(`select id from storybook_asset_references where workspace_id = '${workspace.id}' and creation_session_id = '${sourceAssetSessionId}' and display_name = '彩虹书包' and status = 'ready' order by updated_at desc limit 1`);
  if (!sourceAssetReferenceId) {
    throw new Error("source-storybook asset reference was not persisted as ready");
  }
  await apiExpectError(
    "POST",
    `/api/workspaces/${workspace.id}/storybook-creation-sessions/${sourceAssetSessionId}/directions:generate`,
    token,
    { direction_count: 3, refresh_reason: "initial" },
    409,
    "invalid_creation_session_entry_type",
  );
  await clickByText("为多人制作");
  await selectBatchRecipient(child.nickname);
  await selectBatchRecipient(batchChild.nickname);
  await setBatchMaterialChoice(child.nickname, "profile");
  await setBatchMaterialChoice(batchChild.nickname, "name_only");
  await clickByText("确认对象与素材");
  await waitForText("确认这些变化");
  await clickByText("开始批量制作");
  await waitForText("批量结果");
  await waitForText("已为 2/2 位对象创建定制绘本");
  await waitForText("检查第一本");
  const batchResultUrl = await currentUrl();
  if (!batchResultUrl.includes("sourceRunId=")) {
    throw new Error(`batch result URL should include sourceRunId for restore: ${batchResultUrl}`);
  }
  await cdp.send("Page.reload", { ignoreCache: true });
  await waitForText("批量结果");
  await waitForText("已为 2/2 位对象创建定制绘本");
  await waitForText("检查第一本");
  const batchChildOneBook = await latestCustomBookForChild(workspace.id, child.id, token);
  const batchChildTwoBook = await latestCustomBookForChild(workspace.id, batchChild.id, token);
  assertBatchCustomBook(batchChildOneBook, sourceBook, child.id, "profile", { requirePhotoReference: true });
  assertBatchCustomBook(batchChildTwoBook, sourceBook, batchChild.id, "name_only", { requirePhotoReference: true });
  await clickByText("检查第一本");
  await waitForUrl("/review?result=batch-custom");
  const reviewUrl = await currentUrl();
  if (!reviewUrl.includes("run_item_id=")) {
    throw new Error(`batch review URL should include run_item_id for evidence targeting: ${reviewUrl}`);
  }
  await waitForText("批量定制结果已展示");
  await waitForText("本次制作运行");
  await waitForText("已关联服务端运行记录");
  await waitForText("冻结输入");
  await waitForText("照片参考");
  await waitForText("彩虹书包");
  await waitForText("页级证据");
  await waitForText("批量定制");
  await waitForText("对象 ID");
  await apiDelete(`/api/workspaces/${workspace.id}/storybook-creation-sessions/${sourceAssetSessionId}/asset-references/${sourceAssetReferenceId}`, token);
  await apiExpectError(
    "POST",
    `/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/derive-custom-batch`,
    token,
    {
      child_ids: [child.id, batchChild.id],
      intensity: "quick",
      customization_plan: batchChildOneBook.customization_plan,
      material_choices: {
        [child.id]: "profile",
        [batchChild.id]: "name_only",
      },
    },
    409,
    "asset_revoked",
  );

  await navigate(`${FRONTEND_BASE}/app/${workspace.id}/storybooks/personalized/new?sourceStorybookId=${sourceBook.id}`);
  await waitForText("基于已有绘本创作专属版本");
  await waitForText(sourceBook.title);
  await clickByText(child.nickname);
  await waitForText("主素材");
  await fillByLabel("主素材", "profile");
  await clickByText("确认对象与素材");
  await waitForText("确认这些变化");
  await clickFirstKeepPageToggle();
  await waitForKeepPageBadge();
  await clickByText("开始制作定制绘本");
  await waitForUrl("/review?result=custom");
  await waitForText("修改与交付");
  await waitForText("定制结果已展示");
  await waitForText(child.nickname);
  await waitForText("本次定制计划");
  await waitForText("来源快照");
  await waitForText("冻结页数");
  await waitForText("尽量保持");
  await clickFirstStoryPageThumb();
  await waitForText("当前页检查");
  await clickByText("这页满意");
  await waitForText("已记录本页满意");
  await cdp.send("Page.reload", { ignoreCache: true });
  await waitForText("修改与交付");
  await waitForText("已满意");
  const customStorybookId = await evaluate("location.pathname.split('/').filter(Boolean).at(-2)");
  const customBook = await apiGet(`/api/workspaces/${workspace.id}/storybooks/${customStorybookId}`, token).then((response) => response.data);
  const duplicateCustomBook = await apiPost(`/api/workspaces/${workspace.id}/storybooks/${sourceBook.id}/derive-custom`, token, {
    child_id: child.id,
    intensity: "standard",
    primary_material: "profile",
    customization_plan: customBook.customization_plan,
  }).then((response) => response.data);
  if (duplicateCustomBook.id !== customBook.id) {
    throw new Error(`duplicate single customization should reuse existing output: ${JSON.stringify({ first: customBook.id, duplicate: duplicateCustomBook.id })}`);
  }
  const pagePlan = customBook.customization_plan?.page_plan || [];
  if (!pagePlan.some((item) => item.decision === "prefer_keep")) {
    throw new Error(`custom storybook did not persist prefer_keep plan: ${JSON.stringify(customBook.customization_plan)}`);
  }
  if (customBook.customization_plan?.target_child_nickname !== child.nickname) {
    throw new Error(`custom storybook did not freeze target child nickname: ${JSON.stringify(customBook.customization_plan)}`);
  }
  const customRun = await apiGet(`/api/workspaces/${workspace.id}/storybook-customization-runs/${customBook.customization_run_id}`, token).then((response) => response.data);
  const customRunItem = customRun.items?.find((item) => item.output_storybook_id === customBook.id) || customRun.items?.[0];
  if (customRunItem?.generation_input_snapshot?.target_child_nickname !== child.nickname) {
    throw new Error(`custom run item snapshot did not freeze target child nickname: ${JSON.stringify(customRun)}`);
  }
  const sourceSnapshot = customBook.customization_plan?.source_snapshot;
  if (
    sourceSnapshot?.storybook_id !== sourceBook.id
    || sourceSnapshot?.updated_at !== sourceBook.updated_at
    || sourceSnapshot?.page_count !== sourceBook.pages.length
    || !sourceSnapshot?.page_ids?.includes(sourcePage.id)
    || !sourceSnapshot?.preview_pages?.some((page) => page.id === sourcePage.id && page.status === "ready")
  ) {
    throw new Error(`custom storybook did not persist source snapshot: ${JSON.stringify(customBook.customization_plan)}`);
  }
  await psql(`update storybooks
set status = 'exportable',
    teacher_review_status = 'confirmed',
    teacher_reviewed_at = now(),
    updated_at = now()
where id = '${customBook.id}'`);
  await psql(`update storybook_pages set status = 'ready' where storybook_id = '${customBook.id}'`);
  await apiPost(`/api/workspaces/${workspace.id}/storybooks/${customBook.id}/exports`, token, {}).then((response) => {
    if (response.data?.status !== "queued" || !response.data?.id) {
      throw new Error(`custom export should be queued before evidence is broken: ${JSON.stringify(response)}`);
    }
  });
  const customShareToken = await apiPost(`/api/workspaces/${workspace.id}/storybooks/${customBook.id}/share-links`, token, {}).then((response) => {
    if (!response.data?.token || !response.data?.url) {
      throw new Error(`custom share link should be created before evidence is broken: ${JSON.stringify(response)}`);
    }
    return response.data.token;
  });
  await psql(`update storybook_customization_run_items
set generation_input_snapshot = generation_input_snapshot - 'source_snapshot',
    updated_at = now()
where id = '${customRunItem.id}'`);
  await cdp.send("Page.reload", { ignoreCache: true });
  await waitForText("修改与交付");
  await waitForText("先处理 1 个问题");
  await waitForText("本次制作运行");
  await waitForText("页级证据");
  await apiExpectError(
    "POST",
    `/api/workspaces/${workspace.id}/storybooks/${customBook.id}/exports`,
    token,
    {},
    409,
    "custom_evidence_missing",
  );
  await apiExpectError(
    "POST",
    `/api/workspaces/${workspace.id}/storybooks/${customBook.id}/share-links`,
    token,
    {},
    409,
    "custom_evidence_missing",
  );
  await apiExpectError(
    "POST",
    `/api/share-links/${customShareToken}/exports`,
    token,
    {},
    409,
    "custom_evidence_missing",
  );

  console.log("== personalized storybook UI smoke ok ==");
}

async function assertApiHealth() {
  const response = await fetch(`${API_BASE}/api/health`).catch(() => null);
  if (!response?.ok) {
    throw new Error(`API health check failed: ${API_BASE}/api/health`);
  }
}

async function apiGet(path, token) {
  return apiJson("GET", path, token);
}

async function apiPost(path, token, payload) {
  return apiJson("POST", path, token, payload);
}

async function apiPatch(path, token, payload) {
  return apiJson("PATCH", path, token, payload);
}

async function apiDelete(path, token) {
  return apiJson("DELETE", path, token);
}

async function apiUploadStorybookAsset(workspaceId, sessionId, token, payload) {
  const bytes = Uint8Array.from(atob(PNG_BASE64), (char) => char.charCodeAt(0));
  const form = new FormData();
  form.append("file", new File([bytes], payload.filename, { type: payload.contentType }));
  form.append("kind", payload.kind);
  form.append("idempotency_key", payload.idempotencyKey);
  const response = await fetch(`${API_BASE}/api/workspaces/${workspaceId}/storybook-creation-sessions/${sessionId}/assets`, {
    method: "POST",
    headers: { Authorization: `Bearer ${token}` },
    body: form,
  });
  if (!response.ok) {
    throw new Error(`API upload storybook asset failed: ${response.status} ${await response.text()}`);
  }
  return response.json();
}

async function apiExpectError(method, path, token, payload, expectedStatus, expectedCode) {
  const response = await fetch(`${API_BASE}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(payload),
  });
  const body = await response.json().catch(() => ({}));
  if (response.status !== expectedStatus || body.error?.code !== expectedCode) {
    throw new Error(`expected API ${method} ${path} to fail with ${expectedStatus}/${expectedCode}, got ${response.status}/${JSON.stringify(body)}`);
  }
  return body;
}

async function latestCustomBookForChild(workspaceId, childId, token) {
  const response = await apiGet(`/api/workspaces/${workspaceId}/storybooks?type=custom&target_child_id=${childId}&limit=1&offset=0`, token);
  const book = response.data?.[0];
  if (!book?.id) throw new Error(`custom storybook not found for child: ${childId}`);
  return book;
}

function assertBatchCustomBook(book, sourceBook, childId, primaryMaterial, options = {}) {
  const plan = book.customization_plan || {};
  if (
    book.type !== "custom"
    || plan.mode !== "batch"
    || plan.target_child_id !== childId
    || plan.primary_material !== primaryMaterial
    || plan.source_storybook_id !== sourceBook.id
    || plan.source_snapshot?.storybook_id !== sourceBook.id
    || plan.source_snapshot?.page_count !== sourceBook.pages.length
  ) {
    throw new Error(`batch custom book did not persist plan: ${JSON.stringify({ bookId: book.id, plan })}`);
  }
  if (!book.customization_run_id || !book.customization_run_item_id) {
    throw new Error(`batch custom book did not expose run ids: ${JSON.stringify({ bookId: book.id, runId: book.customization_run_id, runItemId: book.customization_run_item_id })}`);
  }
  if (options.requirePhotoReference && !plan.confirmed_photo_references?.some((reference) => reference.visual_reference_id && reference.planned_pages?.length)) {
    throw new Error(`batch custom book did not freeze confirmed source photo references: ${JSON.stringify({ bookId: book.id, plan })}`);
  }
}

function sourceCustomizationPlan(sourceBook, childIds, mode) {
  return {
    entry_type: "from_storybook",
    mode,
    source_storybook_id: sourceBook.id,
    source_storybook_title: sourceBook.title,
    source_snapshot: {
      storybook_id: sourceBook.id,
      title: sourceBook.title,
      status: sourceBook.status,
      updated_at: sourceBook.updated_at,
      page_count: sourceBook.pages.length,
      page_ids: sourceBook.pages.map((page) => page.id),
      preview_pages: sourceBook.pages.slice(0, 3).map((page) => ({
        id: page.id,
        page_number: page.page_number,
        title: page.title,
        status: page.status,
      })),
    },
    target_child_ids: childIds,
    page_plan: sourceBook.pages.map((page) => ({
      page_id: page.id,
      page_number: page.page_number,
      decision: page.page_number === 1 ? "keep" : "child_version",
    })),
    optional_keep_page_ids: [sourceBook.pages[0]?.id].filter(Boolean),
    confirmed_photo_reference_ids: [],
  };
}

async function waitForApiJob(workspaceId, jobId, token) {
  await waitUntil(async () => {
    const job = await apiGet(`/api/workspaces/${workspaceId}/generation-jobs/${jobId}`, token).then((response) => response.data);
    if (job.status === "failed") throw new Error(`generation job failed: ${jobId}`);
    return job.status === "succeeded";
  }, `generation job did not finish: ${jobId}`, 90_000);
}

async function psql(sql) {
  if (!DB_NAME) throw new Error("DB_NAME is required for smoke database probes");
  return new Promise((resolve, reject) => {
    const child = spawn("docker", ["exec", "-i", DB_CONTAINER, "psql", "-U", DB_USER, "-d", DB_NAME, "-v", "ON_ERROR_STOP=1", "-tAc", sql], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolve(stdout.trim());
      } else {
        reject(new Error(`psql failed (${code}): ${stderr || stdout}`));
      }
    });
  });
}

function jsonbLiteral(value) {
  return `$$${JSON.stringify(value)}$$::jsonb`;
}

async function apiJson(method, path, token, payload) {
  const response = await fetch(`${API_BASE}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      ...(payload === undefined ? {} : { "Content-Type": "application/json" }),
    },
    body: payload === undefined ? undefined : JSON.stringify(payload),
  });
  if (!response.ok) {
    throw new Error(`API ${method} ${path} failed: ${response.status} ${await response.text()}`);
  }
  return response.json();
}

async function startChrome() {
  const candidatePorts = REQUESTED_CDP_PORT ? [REQUESTED_CDP_PORT] : [9733, 9833, 9933];
  const failures = [];
  for (const port of candidatePorts) {
    try {
      await startChromeOnPort(port);
      cdpPort = port;
      return;
    } catch (err) {
      failures.push(`${port}: ${err instanceof Error ? err.message : String(err)}`);
      await stopChromeProcess();
    }
  }
  throw new Error(`Chrome remote debugging did not start. Tried ${failures.join("; ")}`);
}

async function startChromeOnPort(port) {
  const preexisting = await fetch(`http://127.0.0.1:${port}/json/version`).catch(() => null);
  if (preexisting?.ok) {
    throw new Error(`port ${port} is already served by another Chrome instance`);
  }
  userDataDir = mkdtempSync(join(tmpdir(), "kindleaf-personalized-ui-smoke-"));
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
  }, `Chrome remote debugging did not start on ${port}`);
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
  const result = await cdp.send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    const detail = result.exceptionDetails.exception?.description || result.exceptionDetails.text || "page evaluation failed";
    throw new Error(detail);
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

async function waitForClickableText(text) {
  await waitUntil(
    async () => evaluate(`(() => {
      const candidates = [...document.querySelectorAll('button, a')];
      return candidates.some((item) => !item.disabled && item.getAttribute('aria-disabled') !== 'true' && item.innerText.includes(${JSON.stringify(text)}));
    })()`),
    `clickable text not found: ${text}`,
  );
}

async function waitForElementCount(selector, expectedCount) {
  await waitUntil(
    async () => (await evaluate(`document.querySelectorAll(${JSON.stringify(selector)}).length`)) === expectedCount,
    `selector ${selector} did not have ${expectedCount} elements`,
  );
}

async function pageHasText(text) {
  return (await bodyText()).includes(text);
}

async function waitForUrl(fragment) {
  await waitUntil(async () => (await currentUrl()).includes(fragment), `url did not include: ${fragment}`);
}

async function fillByLabel(label, value) {
  await evaluate(`(${setControlValue.toString()})((() => {
    const label = [...document.querySelectorAll('label')].find((item) => item.innerText.includes(${JSON.stringify(label)}));
    if (!label) return null;
    const nested = label.querySelector('input, textarea, select');
    if (nested) return nested;
    const id = label.getAttribute('for');
    return id ? document.getElementById(id) : null;
  })(), ${JSON.stringify(value)})`);
}

async function clickByText(text) {
  await evaluate(`(() => {
    const candidates = [...document.querySelectorAll('button, a')].filter((item) => !item.disabled && item.getAttribute('aria-disabled') !== 'true');
    const el = candidates.find((item) => item.innerText.trim() === ${JSON.stringify(text)})
      || candidates.find((item) => item.innerText.includes(${JSON.stringify(text)}));
    if (!el) throw new Error('click target not found: ${escapeForError(text)}');
    el.click();
  })()`);
}

async function clickFirstElement(selector) {
  await evaluate(`(() => {
    const el = [...document.querySelectorAll(${JSON.stringify(selector)})].find((item) => !item.disabled);
    if (!el) throw new Error('click target not found: ${escapeForError(selector)}');
    el.click();
  })()`);
}

async function selectBatchRecipient(nickname) {
  await evaluate(`(() => {
    const rows = [...document.querySelectorAll('.batch-recipient-row')];
    const row = rows.find((item) => item.innerText.includes(${JSON.stringify(nickname)}));
    const checkbox = row?.querySelector('input[type="checkbox"]');
    if (!checkbox) throw new Error('batch recipient not found: ${escapeForError(nickname)}');
    if (!checkbox.checked) checkbox.click();
  })()`);
}

async function setBatchMaterialChoice(nickname, value) {
  await waitUntil(async () => evaluate(`Boolean((() => {
      const rows = [...document.querySelectorAll('.batch-recipient-row')];
      const row = rows.find((item) => item.innerText.includes(${JSON.stringify(nickname)}));
      return row?.querySelector('select') || null;
    })())`), `batch material selector not found: ${nickname}`);
  await evaluate(`(${setControlValue.toString()})((() => {
      const rows = [...document.querySelectorAll('.batch-recipient-row')];
      const row = rows.find((item) => item.innerText.includes(${JSON.stringify(nickname)}));
      return row?.querySelector('select') || null;
    })(), ${JSON.stringify(value)})`);
}

async function uploadFirstFileInput(filename, contentType, base64) {
  await evaluate(`(() => {
    const input = document.querySelector('input[type="file"]');
    if (!input) throw new Error('file input not found');
    const bytes = Uint8Array.from(atob(${JSON.stringify(base64)}), (char) => char.charCodeAt(0));
    const file = new File([bytes], ${JSON.stringify(filename)}, { type: ${JSON.stringify(contentType)} });
    const transfer = new DataTransfer();
    transfer.items.add(file);
    input.files = transfer.files;
    input.dispatchEvent(new Event('change', { bubbles: true }));
  })()`);
}

async function clickFirstKeepPageToggle() {
  await evaluate(`(() => {
    const labels = [...document.querySelectorAll('.change-preview-card .toggle-row')];
    const label = labels.find((item) => item.innerText.includes('尽量保持这一页'));
    const input = label?.querySelector('input[type="checkbox"]');
    if (!input) throw new Error('keep page checkbox not found');
    input.click();
  })()`);
}

async function waitForKeepPageBadge() {
  await waitUntil(
    async () => evaluate("[...document.querySelectorAll('.change-preview-card .badge')].some((item) => item.innerText.trim() === '尽量保持')"),
    "keep page badge did not update",
  );
}

async function clickFirstStoryPageThumb() {
  await evaluate(`(() => {
    const thumb = document.querySelector('.page-thumb:not(.cover-thumb)');
    if (!thumb) throw new Error('story page thumb not found');
    thumb.click();
  })()`);
}

function setControlValue(control, value) {
  if (!control) throw new Error("form control not found");
  const descriptor = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(control), "value");
  descriptor?.set?.call(control, value);
  control.dispatchEvent(new Event("input", { bubbles: true }));
  control.dispatchEvent(new Event("change", { bubbles: true }));
}

async function waitUntil(check, message, timeoutMs = 60_000) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    if (await check()) return;
    await sleep(100);
  }
  throw new Error(message);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function escapeForError(value) {
  return String(value).replaceAll("'", "\\'");
}

async function shutdown() {
  if (cdp) {
    cdp.close();
    cdp = null;
  }
  await stopChromeProcess();
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
