#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const FRONTEND_BASE = process.env.FRONTEND_BASE_URL || "http://127.0.0.1:5173";
const API_BASE = process.env.API_BASE_URL || "http://127.0.0.1:8080";
const DB_CONTAINER = process.env.DB_CONTAINER || "kindleaf-postgres";
const DB_NAME = process.env.DB_NAME || "kindleaf_development";
const API_TOKEN = process.env.API_TOKEN || "dev-token";
const CHROME_PATH = process.env.CHROME_EXECUTABLE_PATH || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const CDP_PORT = Number(process.env.CDP_PORT || 9333);

const stamp = Date.now();
const plainTitle = `UI Smoke 普通绘本 ${stamp}`;
const childName = `UI Smoke 儿童 ${stamp}`;
const archiveChildName = `UI Smoke 归档儿童 ${stamp}`;
const parentChildName = `UI Smoke 家长提交 ${stamp}`;
const className = `UI Smoke 班级 ${stamp}`;
const registeredName = `UI Smoke 注册老师 ${stamp}`;
const registeredEmail = `ui-smoke-${stamp}@example.com`;
const invitedTeacherName = `UI Smoke 邀请老师 ${stamp}`;
const invitedTeacherEmail = `ui-smoke-invite-${stamp}@example.com`;
const revokedTeacherName = `UI Smoke 撤回老师 ${stamp}`;
const revokedTeacherEmail = `ui-smoke-revoke-${stamp}@example.com`;

let chrome;
let userDataDir;
let cdp;
let schoolWorkspaceId = "school-1";
let teacherWorkspaceId = "school-2";
let personalWorkspaceId = "personal-1";

main().catch(async (error) => {
  console.error(error);
  await cleanup();
  process.exit(1);
});

async function main() {
  console.log("== Kindleaf UI smoke ==");
  console.log(`FRONTEND_BASE=${FRONTEND_BASE}`);
  console.log(`API_BASE=${API_BASE}`);

  await assertApiHealth();
  await cleanup();

  await startChrome();
  await openTab(`${FRONTEND_BASE}/login`);
  await waitForText("登录绘本工作台");

  console.log("1. register personal workspace");
  await evaluate("localStorage.clear()");
  await navigate(`${FRONTEND_BASE}/register`);
  await waitForText("注册账号");
  await fillByLabel("显示名称", registeredName);
  await fillByLabel("邮箱", registeredEmail);
  await fillByLabel("密码", "password123");
  await clickByText("注册并进入个人空间");
  await waitForUrl("/dashboard");
  await waitForText("个人工作台");
  await waitForText("当前空间");

  console.log("1b. invalid session returns to login");
  await evaluate("localStorage.setItem('kindleaf_token', 'invalid-ui-smoke-token')");
  await navigate(`${FRONTEND_BASE}/app`);
  await waitForUrl("/login");
  await waitForText("登录绘本工作台");
  await evaluate("localStorage.clear()");
  await navigate(`${FRONTEND_BASE}/app`);
  await waitForUrl("/login");
  await waitForText("登录绘本工作台");

  console.log("2. login and dashboard");
  await evaluate("localStorage.clear()");
  await navigate(`${FRONTEND_BASE}/login`);
  await waitForText("登录绘本工作台");
  await fillByLabel("邮箱或手机号", "lin@example.com");
  await fillByLabel("密码", "demo");
  await clickByText("登录并进入个人空间");
  await waitForUrl("/dashboard");
  await waitForText("我的工作台");
  schoolWorkspaceId = await resolveSchoolWorkspaceId();
  teacherWorkspaceId = await resolveTeacherWorkspaceId();
  personalWorkspaceId = await resolvePersonalWorkspaceId();
  const platformWorkspaceId = await resolvePlatformWorkspaceId();
  await expectNoSelectOption("Kindleaf 平台运营");
  await navigate(`${FRONTEND_BASE}/app/${platformWorkspaceId}/dashboard`);
  await waitForUrl("/operator/submissions");
  await waitForText("平台投稿审核");
  await navigate(`${FRONTEND_BASE}/operator/marketplace`);
  await waitForText("部署前总检查");
  await waitForText("当前环境还不适合对外试点");
  await waitForText("数据库连接");
  await waitForText("数据库结构");
  await waitForText("外部访问域名");
  await waitForText("登录令牌有效期");
  await waitForText("生成 provider 密钥");
  await waitForText("生成 provider 配置");
  await waitForText("真实生成能力");
  await waitForText("生成预算上限");
  await waitForText("演示数据开关");
  await waitForText("当前生成 provider 状态");
  await waitForText("图片组件");
  await waitForText("seedream");
  await waitForText("缺少 SEEDREAM_API_KEY 或 ARK_API_KEY");
  await waitForText("/api/v3/images/generations");
  await waitForText("PDF 与插图存储边界");
  await waitForText("PDF 目录");
  await waitForText("tmp/exports");
  await waitForText("插图目录");
  await waitForText("tmp/generated-images");
  await waitForText("存储后端");
  await waitForText("本地文件系统");
  await waitForText("下载策略");
  await waitForText("权限 API 下载");
  await waitForText("PDF 上限");
  await waitForText("50 MB");
  await waitForText("文件名校验");
  await waitForText("已启用");
  await waitForText("公共直链");
  await waitForText("已关闭");

  console.log("2b. teacher cannot access school admin pages");
  await navigate(`${FRONTEND_BASE}/app/${teacherWorkspaceId}/admin`);
  await waitForText("需要园所管理员权限");
  await waitForText("当前空间角色不能访问园所管理");
  await navigate(`${FRONTEND_BASE}/app/${teacherWorkspaceId}/children`);
  await waitForText("儿童档案");
  await waitForText("安安");
  await expectNoText("家长资料链接");
  await expectNoText("待老师确认的儿童资料");

  console.log("3. API detail pages do not fall back to mock data");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/storybooks/00000000-0000-0000-0000-00000000ffff`);
  await waitForText("绘本详情加载失败");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/children/00000000-0000-0000-0000-00000000ffff`);
  await waitForText("儿童资料加载失败");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/marketplace/00000000-0000-0000-0000-00000000ffff`);
  await waitForText("模板不存在");
  await navigate(`${FRONTEND_BASE}/invite/demo-token`);
  await waitForText("邀请不可用");

  console.log("4. invite and accept school teacher");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/admin/members`);
  await waitForText("成员管理");
  await clickByText("邀请老师");
  await waitForText("老师姓名");
  await fillByLabel("老师姓名", invitedTeacherName);
  await fillByLabel("老师邮箱", invitedTeacherEmail);
  await clickByText("发送邀请");
  await waitForText("邀请已发送");
  await clickByText("复制邀请链接");
  await waitForText("邀请链接已准备复制");
  await waitForText(invitedTeacherName);
  await clickByText("复制成员邀请链接");
  await waitForText("邀请链接已准备复制");
  await clickByText("邀请老师");
  await waitForText("老师姓名");
  await fillByLabel("老师姓名", revokedTeacherName);
  await fillByLabel("老师邮箱", revokedTeacherEmail);
  await clickByText("发送邀请");
  await waitForText("邀请已发送");
  await clickRowButton(revokedTeacherEmail, "撤回邀请");
  await waitForText("邀请已撤回");
  await waitForText("已撤回");
  const invitedMembers = await apiGet(`/api/workspaces/${schoolWorkspaceId}/members`);
  const invitedMember = invitedMembers.data?.find((item) => item.email === invitedTeacherEmail);
  let invitePath = invitedMember?.invitation_url || (invitedMember?.invitation_token ? `/invite/${invitedMember.invitation_token}` : "");
  if (!invitePath) {
    throw new Error(`No invitation path returned for ${invitedTeacherEmail}`);
  }
  await navigate(`${FRONTEND_BASE}${invitePath}`);
  await waitForText("老师邀请");
  await waitForText("加入");
  await waitForText("邀请状态");
  await clickByText("接受邀请并进入园所空间");
  await waitForUrl("/dashboard");
  await waitForText("园所工作台");

  console.log("5. create classroom");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/admin/classes`);
  await waitForText("班级管理");
  await clickByText("创建班级");
  await waitForText("班级名称");
  await fillByLabel("班级名称", className);
  await selectOptionByText("4-5 岁");
  await clickByText("确认创建");
  await waitForText("班级已创建");
  await clickRowButton(className, "归档班级");
  await waitForText("班级已归档");

  console.log("6. create child profile");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/children`);
  await waitForText("儿童档案");
  await clickByText("新增儿童档案");
  await waitForText("孩子称呼");
  await fillByLabel("孩子称呼", childName);
  await selectOptionByText("4-5 岁");
  await fillByLabel("兴趣或喜欢的活动", "积木、唱歌");
  await fillByLabel("性格特点", "认真、愿意尝试");
  await fillByLabel("关注点", "轮流等待");
  await clickByText("确认新增");
  await waitForText("资料已提交");
  await waitForText("定制准备");
  await waitForText(childName);
  await clickCardContaining("小雨");
  await waitForText("儿童档案");
  await waitForText("入园适应和午睡");
  await clickByText("编辑资料");
  await waitForText("编辑 小雨 的资料");
  await fillByLabel("关注点", "轮流等待和主动表达");
  await fillByLabel("兴趣标签", "积木、唱歌、小汽车");
  await fillByLabel("性格特点", "认真、愿意尝试、喜欢被鼓励");
  await clickByText("保存资料");
  await waitForText("儿童资料已保存");
  const selectedChildId = await currentChildId();
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/children`);
  await waitForText("儿童档案");
  await clickByText("新增儿童档案");
  await waitForText("孩子称呼");
  await fillByLabel("孩子称呼", archiveChildName);
  await selectOptionByText("4-5 岁");
  await fillByLabel("兴趣或喜欢的活动", "拼图、画画");
  await fillByLabel("性格特点", "安静、细心");
  await fillByLabel("关注点", "离园资料归档验证");
  await clickByText("确认新增");
  await waitForText("资料已提交");
  await waitForText(archiveChildName);
  await clickCardContaining(archiveChildName);
  await waitForText("直接进入定制绘本");
  await clickByText("归档资料");
  await waitForText("儿童资料已归档");
  await clickByText("恢复资料");
  await waitForText("儿童资料已恢复");

  console.log("7. submit parent intake and confirm");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/children`);
  await waitForText("家长资料链接");
  await selectOptionByLabelInCard("收集家长补充资料", "班级范围", "小一班");
  await selectOptionByLabelInCard("收集家长补充资料", "链接状态", "可填写");
  await clickByText("生成资料链接");
  await waitForText("家长资料链接已生成");
  const intakePath = await extractNoticeIntakePath();
  const intakeLink = await parentIntakeLinkByUrl(intakePath);
  await navigate(`${FRONTEND_BASE}${intakePath}`);
  await waitForText("填写孩子资料");
  await waitForText("提交目标空间：星星幼儿园");
  await waitForText("提交目标班级：小一班");
  await fillByLabel("孩子称呼", parentChildName);
  await selectOptionByText("4-5 岁");
  await fillByLabel("兴趣或喜欢的活动", "画画、积木车");
  await clickByText("提交给老师确认");
  await waitForText("资料已提交");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/children`);
  await waitForText("家长资料链接");
  await waitForText("可填写");
  await apiPost(`/api/workspaces/${schoolWorkspaceId}/parent-intake-links/${intakeLink.id}/revoke`, {});
  await selectOptionByText("已撤回");
  await waitForText("已显示");
  await waitForText("已停止收集");
  await navigate(`${FRONTEND_BASE}${intakePath}`);
  await waitForText("家长资料链接已撤回");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/children`);
  await waitForText("家长资料链接");
  await selectOptionByLabelInCard("收集家长补充资料", "班级范围", "小一班");
  await selectOptionByLabelInCard("收集家长补充资料", "链接状态", "可填写");
  await clickByText("生成资料链接");
  await waitForText("家长资料链接已生成");
  await waitForEnabledButton("停用小一班可填写链接");
  await clickByText("停用小一班可填写链接");
  await waitForText("已停用");
  await waitForText("还没有资料链接");
  await selectOptionByText("已撤回");
  await waitForText("已停止收集");

  console.log("8. create plain storybook");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/storybooks/new`);
  await waitForText("生成状态");
  await waitForText("图片组件");
  await waitForText("seedream");
  await waitForText("缺少 SEEDREAM_API_KEY 或 ARK_API_KEY");
  await fillInputAt(0, plainTitle);
  await fillInputAt(1, "验证 UI smoke 普通绘本流程");
  await clickByText("生成绘本方案");
  await waitForText("绘本方案已生成");
  await waitForText("进入场景");
  await clickByText("确认方案，继续角色");
  await waitForText("米米");
  await clickByText("确认角色，继续分页");
  await waitForText("分页图文");
  await clickByText("确认分页，进入预览");
  await waitForText("分页图文已生成并写入绘本");
  await waitForText("已准备好");
  await clickByText("进入绘本详情");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/storybooks`);
  await waitForText("园所绘本");
  await waitForText("先创建普通绘本");
  await waitForText(plainTitle);
  await clickByText(plainTitle);
  await waitForUrl("/storybooks/");
  await waitForText("普通绘本详情");
  await waitForText(plainTitle);
  const sharedBookTitle = await currentStorybookTitle();
  const plainBookId = await currentStorybookId();
  console.log(`plain=${plainBookId}`);
  await clickByText("复制副本");
  await waitForText(`${plainTitle} 副本`);
  await waitForText("普通绘本详情");
  await waitForText(`复制自《${plainTitle}》`);
  await expectButtonDisabled("导出 PDF");
  await expectButtonDisabled("分享");
  const duplicateTitle = `${plainTitle} 副本改名`;
  await clickByText("编辑信息");
  await waitForText("编辑绘本信息");
  await fillByLabel("绘本标题", duplicateTitle);
  await selectOptionByText("5-6 岁");
  await fillByLabel("使用场景", "情绪引导");
  await fillByLabel("教学目标", "练习说出情绪并请求帮助");
  await fillByLabel("封面风格", "明亮、有安全感");
  await clickByText("保存信息");
  await waitForText("绘本信息已保存");
  await waitForText(duplicateTitle);
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/storybooks`);
  await waitForText(duplicateTitle);
  await waitForText(`复制自《${plainTitle}》`);
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/storybooks/${plainBookId}`);
  await waitForText(plainTitle);

  console.log("9. edit page, generate image, and share inside workspace");
  const editedPageTitle = `UI Smoke 第 1 页 ${stamp}`;
  await fillByLabel("页面标题", editedPageTitle);
  await fillByLabel("正文", "孩子们在老师引导下练习轮流等待。");
  await fillByLabel("插图描述", "明亮教室里，老师和孩子围坐在地毯上，一起看小汽车。");
  await clickByText("保存本页");
  await waitForText("当前页已保存");
  await clickByText("生成插图");
  await waitForText("插图任务已完成");
  await selectOptionByText("园所/空间内共享");
  await clickByText("保存共享设置");
  await waitForText("共享设置已保存");

  console.log("10. export and share plain storybook");
  await clickByText("导出 PDF");
  await clickByText("分享");
  await waitForText("管理分享链接");
  await waitForText("7 天有效");
  if (await pageHasText("创建新的分享链接")) {
    await clickByText("创建新的分享链接");
    try {
      await waitForText("分享链接已创建");
      await waitForText("有效期至");
      await waitForText("分享链接 1");
      await waitForText("打开最新分享页");
      await clickByText("复制链接");
      await waitForText("分享链接已准备复制");
      const sharePath = await extractSharePath();
      await navigate(`${FRONTEND_BASE}${sharePath}`);
    } catch {
      const shareBookId = await currentStorybookId();
      const shareJson = await apiPost(`/api/workspaces/${schoolWorkspaceId}/storybooks/${shareBookId}/share-links`, {});
      const sharePath = shareJson?.data?.url || `/link/share/${shareJson?.data?.token}`;
      await navigate(`${FRONTEND_BASE}${sharePath}`);
    }
  } else {
    const sharePath = await extractSharePath();
    await navigate(`${FRONTEND_BASE}${sharePath}`);
  }
  await waitForText("家庭分享版");
  await waitForText(sharedBookTitle);
  await clickByText("下载 PDF");
  await waitForText("PDF 已准备下载");

  console.log("11. derive custom storybook");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/storybooks/${plainBookId}/customize?childId=${selectedChildId}`);
  await waitForText("生成定制绘本");
  await waitForText("生成状态");
  await waitForText("图片组件");
  await waitForText("seedream");
  await waitForText("缺少 SEEDREAM_API_KEY 或 ARK_API_KEY");
  await waitForText("当前儿童");
  await clickCardContaining("小雨");
  await clickByText("确认孩子");
  await waitForText("档案检查");
  await clickByText("确认档案");
  await waitForText("定制强度");
  await clickByText("生成定制方案");
  await waitForText("定制方案已生成");
  await clickByText("生成定制副本");
  await waitForText("定制副本已生成");
  await clickByText("查看生成结果");
  await waitForUrl("/storybooks/");
  await waitForText("编辑当前页");
  await clickByText("标记可交付");
  await waitForText("绘本已标记可交付");
  console.log(`custom=${await currentStorybookId()}`);

  console.log("12. submit, approve, and copy from marketplace");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/admin/submissions`);
  await waitForText("市场投稿");
  await clickByText("新建投稿");
  await waitForText("新建市场投稿");
  await selectOptionByText(sharedBookTitle);
  await clickByText("创建投稿草稿");
  await waitForText("投稿草稿已创建");
  await selectOptionByText("隐私待确认");
  await waitForText(sharedBookTitle);
  await clickRowButton(sharedBookTitle, "确认隐私");
  await waitForText("投稿隐私确认");
  await clickByText("确认无隐私风险");
  await waitForTextOrDump("隐私确认已保存", "plain submission privacy confirmation");
  await selectOptionByText("审核中");
  await waitForText(sharedBookTitle);

  const riskyBook = await apiPost(`/api/workspaces/${schoolWorkspaceId}/storybooks`, {
    title: `UI Smoke 隐私风险绘本 ${stamp}`,
    age_group: "4-5 岁",
    use_scene: "市场投稿隐私验证",
    teaching_goal: "验证投稿隐私风险提示",
  });
  const riskyBookId = riskyBook?.data?.id;
  const riskyPageId = riskyBook?.data?.pages?.[0]?.id;
  if (!riskyBookId || !riskyPageId) {
    throw new Error("risky storybook was not created");
  }
  await apiPatch(`/api/workspaces/${schoolWorkspaceId}/storybooks/${riskyBookId}/pages/${riskyPageId}`, {
    body: "老师电话是 138 0013 8000，这段内容应阻断市场投稿。",
  });
  const riskySubmission = await apiPost(`/api/workspaces/${schoolWorkspaceId}/submissions`, {
    storybook_id: riskyBookId,
  });
  const riskySubmissionId = riskySubmission?.data?.id;
  if (!riskySubmissionId) {
    throw new Error("risky submission was not created");
  }
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/admin/submissions`);
  await waitForText("市场投稿");
  await selectOptionByText("隐私待确认");
  await waitForText("UI Smoke 隐私风险绘本");
  await clickRowButton("UI Smoke 隐私风险绘本", "确认隐私");
  await waitForText("投稿隐私确认");
  await clickByText("确认无隐私风险");
  await waitForText("确认失败：发现隐私风险");
  await waitForText("请回到绘本详情修改对应正文");

  await navigate(`${FRONTEND_BASE}/operator/submissions`);
  await waitForText(sharedBookTitle);
  await selectOptionByText("待审核");
  await waitForText(sharedBookTitle);
  await clickByText("审核");
  await waitForText("通过并上架市场");

  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/admin`);
  await waitForText("待处理事项");
  await waitForText("状态说明");
  console.log("12b. recover generation queue from school admin");
  const schoolRecoverJobId = randomUUID();
  const foreignRecoverJobId = randomUUID();
  const privacyAuditJobId = randomUUID();
  const cancelProbeJobId = randomUUID();
  executeSql(`
insert into generation_jobs
  (id, workspace_id, storybook_id, job_type, status, input_json, output_json, attempt_count, last_error, locked_by, locked_at, created_at, finished_at)
values
  ('${schoolRecoverJobId}', '${schoolWorkspaceId}', null, 'storybook_plan', 'running', '{"theme":"UI Smoke 园所恢复"}'::jsonb, null, 1, null, 'ui-smoke-stale-worker', now() - interval '30 minutes', now(), null),
  ('${foreignRecoverJobId}', '${personalWorkspaceId}', null, 'storybook_plan', 'running', '{"theme":"UI Smoke 跨空间不应恢复"}'::jsonb, null, 1, null, 'ui-smoke-stale-worker', now() - interval '30 minutes', now(), null),
  ('${cancelProbeJobId}', '${schoolWorkspaceId}', null, 'ui_cancel_probe', 'queued', '{"theme":"UI Smoke 取消生成任务"}'::jsonb, null, 0, null, null, null, now() + interval '2 seconds', null),
  ('${privacyAuditJobId}', '${schoolWorkspaceId}', null, 'privacy_audit_probe', 'succeeded', '{"theme":"UI Smoke 脱敏审计"}'::jsonb, '{"schema_version":"generation.provider.v1","provider":"deepseek","mode":"privacy_audit_probe","message":"UI smoke 脱敏审计展示","privacy_audit":{"redacted":true,"labels":["sensitive_field","phone"]}}'::jsonb, 1, null, null, null, now() + interval '1 second', now() + interval '1 second');
`);
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/admin`);
  await waitForText("生成队列");
  await waitForText("ui_cancel_probe");
  await clickCompactRowContaining("ui_cancel_probe");
  await waitForText("取消任务");
  await clickByText("取消任务");
  await waitForText("已取消生成任务");
  await waitForText("已取消");
  await waitForText("privacy_audit_probe");
  await clickCompactRowContaining("privacy_audit_probe");
  await waitForText("脱敏审计");
  await waitForText("已脱敏：儿童/家长资料、手机号");
  await clickByText("恢复生成队列");
  await waitForText("生成队列已恢复");
  await waitForText("已处理");
  const foreignStatus = sqlValue(`select status || ':' || coalesce(locked_by, '') from generation_jobs where id = '${foreignRecoverJobId}'`);
  if (foreignStatus !== "running:ui-smoke-stale-worker") {
    throw new Error(`foreign recover job was unexpectedly changed: ${foreignStatus}`);
  }
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/admin/audit-logs`);
  await waitForText("最近操作记录");
  await waitForText("取消生成任务");

  const templateId = await marketplaceTemplateIdByTitle("一起玩小汽车");
  await navigate(`${FRONTEND_BASE}/app/${schoolWorkspaceId}/marketplace/${templateId}`);
  await waitForText("模板详情");
  await waitForText("复制后会成为当前空间的普通绘本");
  await clickByText("复制到当前空间");
  await waitForText("确认复制并打开副本");
  await clickByText("确认复制并打开副本");
  await waitForUrl("/storybooks/");
  await waitForText("普通绘本详情");
  await waitForText("绘本详情");

  console.log("== ui smoke ok ==");
  await cleanup();
  await shutdown();
}

async function assertApiHealth() {
  const payload = await apiGet("/api/health", false);
  if (payload.data?.status !== "ok") {
    throw new Error("API health check failed");
  }
}

async function apiGet(path, auth = true) {
  const response = await fetch(`${API_BASE}${path}`, {
    headers: auth ? { Authorization: `Bearer ${API_TOKEN}` } : {},
  });
  if (!response.ok) {
    throw new Error(`${path} failed with ${response.status}`);
  }
  return response.json();
}

async function apiPost(path, body) {
  const response = await fetch(`${API_BASE}${path}`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${API_TOKEN}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw new Error(`${path} failed with ${response.status}: ${await response.text()}`);
  }
  return response.json();
}

async function apiPatch(path, body) {
  const response = await fetch(`${API_BASE}${path}`, {
    method: "PATCH",
    headers: {
      Authorization: `Bearer ${API_TOKEN}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });
  if (!response.ok) {
    throw new Error(`${path} failed with ${response.status}: ${await response.text()}`);
  }
  return response.json();
}

async function marketplaceTemplateIdByTitle(title) {
  const payload = await apiGet(`/api/marketplace/templates?q=${encodeURIComponent(title)}`);
  const template = payload.data?.find((item) => item.title === title);
  if (!template) {
    throw new Error(`marketplace template not found: ${title}`);
  }
  return template.id;
}

async function parentIntakeLinkByUrl(url) {
  const payload = await apiGet(`/api/workspaces/${schoolWorkspaceId}/parent-intake-links?status=active&classroom=${encodeURIComponent("小一班")}&limit=50&offset=0`);
  const link = payload.data?.find((item) => item.url === url);
  if (!link) {
    throw new Error(`parent intake link not found: ${url}`);
  }
  return link;
}

async function resolveSchoolWorkspaceId() {
  const payload = await apiGet("/api/auth/me");
  const school = payload.data?.workspaces?.find((item) => item.type === "school" && item.role === "school_admin")
    || payload.data?.workspaces?.find((item) => item.type === "school")
    || payload.data?.workspaces?.[0];
  if (!school?.id) {
    throw new Error("school workspace not found");
  }
  return school.id;
}

async function resolveTeacherWorkspaceId() {
  const payload = await apiGet("/api/auth/me");
  const school = payload.data?.workspaces?.find((item) => item.type === "school" && item.role === "school_teacher");
  if (!school?.id) {
    throw new Error("teacher workspace not found");
  }
  return school.id;
}

async function resolvePersonalWorkspaceId() {
  const payload = await apiGet("/api/auth/me");
  const personal = payload.data?.workspaces?.find((item) => item.type === "personal");
  if (!personal?.id) {
    throw new Error("personal workspace not found");
  }
  return personal.id;
}

async function resolvePlatformWorkspaceId() {
  const payload = await apiGet("/api/auth/me");
  const platform = payload.data?.workspaces?.find((item) => item.type === "platform" && item.role === "platform_operator");
  if (!platform?.id) {
    throw new Error("platform workspace not found");
  }
  return platform.id;
}

function executeSql(sql) {
  const result = spawnSync("docker", ["exec", "-i", DB_CONTAINER, "psql", "-U", "postgres", "-d", DB_NAME, "-v", "ON_ERROR_STOP=1"], {
    input: sql,
    encoding: "utf8",
    stdio: ["pipe", "pipe", "pipe"],
  });
  if (result.status !== 0) {
    throw new Error(`sql failed: ${result.stderr || result.stdout}`);
  }
}

function sqlValue(sql) {
  const result = spawnSync("docker", ["exec", "-i", DB_CONTAINER, "psql", "-U", "postgres", "-d", DB_NAME, "-Atc", sql], {
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`sql failed: ${result.stderr || result.stdout}`);
  }
  return result.stdout.trim();
}

async function startChrome() {
  userDataDir = mkdtempSync(join(tmpdir(), "kindleaf-ui-smoke-"));
  chrome = spawn(CHROME_PATH, [
    "--headless=new",
    `--remote-debugging-port=${CDP_PORT}`,
    `--user-data-dir=${userDataDir}`,
    "--disable-gpu",
    "--no-first-run",
    "about:blank",
  ], { stdio: "ignore" });
  await waitUntil(async () => {
    const response = await fetch(`http://127.0.0.1:${CDP_PORT}/json/version`).catch(() => null);
    return Boolean(response?.ok);
  }, "Chrome remote debugging did not start");
}

async function openTab(url) {
  const response = await fetch(`http://127.0.0.1:${CDP_PORT}/json/new?${encodeURIComponent(url)}`, { method: "PUT" });
  if (!response.ok) {
    throw new Error(`failed to create Chrome tab: ${response.status}`);
  }
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

async function currentStorybookId() {
  return evaluate("location.pathname.split('/').filter(Boolean).at(-1)");
}

async function currentChildId() {
  return evaluate("location.pathname.split('/').filter(Boolean).at(-1)");
}

async function currentStorybookTitle() {
  return evaluate("document.querySelector('.page-header h1')?.innerText?.trim() || document.querySelector('h1')?.innerText?.trim() || ''");
}

async function extractSharePath() {
  const text = await bodyText();
  const match = text.match(/\/link\/share\/[a-z0-9-]+/i);
  if (!match) {
    throw new Error("share path not found in page notice");
  }
  return match[0];
}

async function extractIntakePath() {
  const text = await bodyText();
  const match = text.match(/\/link\/intake\/[a-z0-9-]+/i);
  if (!match) {
    throw new Error("intake path not found in page notice");
  }
  return match[0];
}

async function extractNoticeIntakePath() {
  const text = await evaluate("document.querySelector('.notice')?.innerText || ''");
  const match = text.match(/\/link\/intake\/[a-z0-9-]+/i);
  if (!match) {
    throw new Error("intake path not found in notice");
  }
  return match[0];
}

async function extractInvitePath() {
  const text = await bodyText();
  const match = text.match(/\/invite\/[a-z0-9-]+/i);
  if (!match) {
    throw new Error("invite path not found in page notice");
  }
  return match[0];
}

async function waitForText(text) {
  await waitUntil(async () => (await bodyText()).includes(text), `text not found: ${text}`);
}

async function waitForTextOrDump(text, context) {
  try {
    await waitForText(text);
  } catch (err) {
    const body = await bodyText().catch(() => "");
    throw new Error(`${context}: text not found: ${text}\n${body.slice(0, 1800)}`, { cause: err });
  }
}

async function pageHasText(text) {
  return (await bodyText()).includes(text);
}

async function expectNoText(text) {
  const exists = await pageHasText(text);
  if (exists) {
    throw new Error(`unexpected text found: ${text}`);
  }
}

async function expectNoSelectOption(text) {
  const exists = await evaluate(`Boolean([...document.querySelectorAll('option')].find((option) => option.innerText.includes(${JSON.stringify(text)})))`);
  if (exists) {
    throw new Error(`unexpected select option: ${text}`);
  }
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

async function fillByPlaceholder(placeholder, value) {
  await evaluate(`(${setControlValue.toString()})(document.querySelector(${JSON.stringify(`[placeholder="${placeholder}"]`)}), ${JSON.stringify(value)})`);
}

async function fillInputAt(index, value) {
  await evaluate(`(${setControlValue.toString()})([...document.querySelectorAll('input')][${index}], ${JSON.stringify(value)})`);
}

async function selectOptionByText(text) {
  await waitUntil(
    () => evaluate(`Boolean([...document.querySelectorAll('option')].find((item) => item.innerText.trim() === ${JSON.stringify(text)}))`),
    `option not found: ${text}; available options: ${await optionSummary()}`,
  );
  await evaluate(`(() => {
    const option = [...document.querySelectorAll('option')].find((item) => item.innerText.trim() === ${JSON.stringify(text)});
    if (!option) throw new Error('option not found: ${escapeForError(text)}');
    (${setControlValue.toString()})(option.closest('select'), option.value);
  })()`);
}

async function selectOptionByLabelInCard(cardText, labelText, optionText) {
  await waitUntil(
    () => evaluate(`(() => {
      const card = [...document.querySelectorAll('.card')].find((item) => item.innerText.includes(${JSON.stringify(cardText)}));
      if (!card) return false;
      const label = [...card.querySelectorAll('label')].find((item) => item.innerText.includes(${JSON.stringify(labelText)}));
      if (!label) return false;
      return Boolean([...label.querySelectorAll('option')].find((item) => item.innerText.trim() === ${JSON.stringify(optionText)}));
    })()`),
    `option not found in card: ${cardText} / ${labelText} / ${optionText}`,
  );
  await evaluate(`(() => {
    const card = [...document.querySelectorAll('.card')].find((item) => item.innerText.includes(${JSON.stringify(cardText)}));
    if (!card) throw new Error('card not found: ${escapeForError(cardText)}');
    const label = [...card.querySelectorAll('label')].find((item) => item.innerText.includes(${JSON.stringify(labelText)}));
    if (!label) throw new Error('label not found: ${escapeForError(labelText)}');
    const option = [...label.querySelectorAll('option')].find((item) => item.innerText.trim() === ${JSON.stringify(optionText)});
    if (!option) throw new Error('option not found: ${escapeForError(optionText)}');
    (${setControlValue.toString()})(label.querySelector('select'), option.value);
  })()`);
}

async function optionSummary() {
  return evaluate(`[...document.querySelectorAll('option')].map((item) => item.innerText.trim()).join(' | ')`);
}

async function clickByText(text) {
  await evaluate(`(() => {
    const candidates = [...document.querySelectorAll('button, a')].filter((item) => !item.disabled);
    const el = candidates.find((item) => item.innerText.trim() === ${JSON.stringify(text)})
      || candidates.find((item) => item.innerText.includes(${JSON.stringify(text)}));
    if (!el) throw new Error('click target not found: ${escapeForError(text)}');
    el.click();
  })()`);
}

async function expectButtonDisabled(text) {
  const started = Date.now();
  let lastResult = null;
  while (Date.now() - started < 10_000) {
    lastResult = await evaluate(`(() => {
      const button = [...document.querySelectorAll('button')].find((item) => item.innerText.includes(${JSON.stringify(text)}));
      if (!button) throw new Error('button not found: ${escapeForError(text)}');
      return { disabled: Boolean(button.disabled), html: button.outerHTML, page: document.body.innerText.slice(0, 1200) };
    })()`);
    if (lastResult.disabled) return;
    await sleep(100);
  }
  throw new Error(`button is not disabled: ${text}\n${lastResult?.html || ""}\n${lastResult?.page || ""}`);
}

async function clickCardContaining(text) {
  await evaluate(`(() => {
    const el = [...document.querySelectorAll('button.select-card, a.storybook-card, .storybook-card, a, button')]
      .find((item) => item.innerText.includes(${JSON.stringify(text)}));
    if (!el) throw new Error('card not found: ${escapeForError(text)}');
    el.click();
  })()`);
}

async function clickCompactRowContaining(text) {
  await waitUntil(
    () => evaluate(`Boolean([...document.querySelectorAll('button.compact-row, a.compact-row')].find((item) => item.innerText.includes(${JSON.stringify(text)})))`),
    `compact row not found: ${text}`,
  );
  await evaluate(`(() => {
    const el = [...document.querySelectorAll('button.compact-row, a.compact-row')]
      .find((item) => item.innerText.includes(${JSON.stringify(text)}));
    if (!el) throw new Error('compact row not found: ${escapeForError(text)}');
    el.click();
  })()`);
}

async function waitForEnabledButton(text) {
  await waitUntil(
    () => evaluate(`Boolean([...document.querySelectorAll('button')].find((item) => !item.disabled && item.innerText.includes(${JSON.stringify(text)})))`),
    `enabled button not found: ${text}`,
  );
}

async function waitForCardContaining(text) {
  await waitUntil(async () => {
    const found = await evaluate(`Boolean([...document.querySelectorAll('button.select-card, a.storybook-card, .storybook-card')].find((item) => item.innerText.includes(${JSON.stringify(text)})))`);
    if (!found) throw new Error(`card not found yet: ${text}`);
  });
}

async function clickRowButton(rowText, buttonText) {
  await waitUntil(
    () => evaluate(`Boolean([...document.querySelectorAll('.table-row')].find((item) => item.innerText.includes(${JSON.stringify(rowText)})))`),
    `row not found: ${rowText}`,
  );
  await evaluate(`(() => {
    const row = [...document.querySelectorAll('.table-row')].find((item) => item.innerText.includes(${JSON.stringify(rowText)}));
    if (!row) throw new Error('row not found: ${escapeForError(rowText)}');
    const button = [...row.querySelectorAll('button')].find((item) => item.innerText.includes(${JSON.stringify(buttonText)}));
    if (!button) throw new Error('row button not found: ${escapeForError(buttonText)}');
    button.click();
  })()`);
}

function setControlValue(control, value) {
  if (!control) throw new Error("form control not found");
  const descriptor = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(control), "value");
  descriptor?.set?.call(control, value);
  control.dispatchEvent(new Event("input", { bubbles: true }));
  control.dispatchEvent(new Event("change", { bubbles: true }));
}

async function waitUntil(check, message, timeoutMs = 10_000) {
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

async function cleanup() {
  const sql = `
with ui_users as (
  select id from users where email like 'ui-smoke-%@example.com'
), ui_workspaces as (
  select wm.workspace_id as id
  from workspace_members wm
  join ui_users u on u.id = wm.user_id
  union
  select id from workspaces where name like 'UI Smoke 注册老师 %的个人空间'
), ui_books as (
  select id from storybooks where title like 'UI Smoke %' or title like '${childName}%' or workspace_id in (select id from ui_workspaces)
), ui_children as (
  select id from children where nickname like 'UI Smoke %'
), ui_classrooms as (
  select id from classrooms where name like 'UI Smoke %'
), ui_members as (
  select id from workspace_members
  where user_id in (select id from ui_users)
     or workspace_id in (select id from ui_workspaces)
     or id::text in (
       select wm.id::text
       from workspace_members wm
       join users u on u.id = wm.user_id
       where u.email like 'ui-smoke-%@example.com'
     )
), deleted_audit_logs as (
  delete from audit_logs
  where resource_id in (
    select id from ui_books
    union
    select id from ui_children
    union
    select id from ui_classrooms
    union
    select id from ui_members
  )
     or workspace_id in (select id from ui_workspaces)
     or actor_user_id in (select id from ui_users)
     or metadata_json::text like '%UI Smoke%'
  returning id
), deleted_templates as (
  delete from marketplace_templates where title like 'UI Smoke %' returning id
), deleted_submissions as (
  delete from marketplace_submissions where title like 'UI Smoke %' or source_storybook_id in (select id from ui_books) returning id
), deleted_intakes as (
  delete from parent_intakes where child_nickname like 'UI Smoke %' or confirmed_child_id in (select id from ui_children) returning id
), deleted_exports as (
  delete from export_jobs where storybook_id in (select id from ui_books) returning id
), deleted_shares as (
  delete from share_links where storybook_id in (select id from ui_books) returning id
), deleted_jobs as (
  delete from generation_jobs
  where storybook_id in (select id from ui_books)
     or input_json::text like '%UI Smoke%'
     or locked_by = 'ui-smoke-stale-worker'
  returning id
), deleted_roles as (
  delete from storybook_roles where storybook_id in (select id from ui_books) returning id
), deleted_pages as (
  delete from storybook_pages where storybook_id in (select id from ui_books) returning id
), deleted_books as (
  delete from storybooks where id in (select id from ui_books) returning id
), deleted_workspace_members as (
  delete from workspace_members
  where id in (select id from ui_members)
     or user_id in (select id from ui_users)
     or workspace_id in (select id from ui_workspaces)
  returning id
), deleted_workspaces as (
  delete from workspaces where id in (select id from ui_workspaces) returning id
), deleted_children as (
  delete from children where id in (select id from ui_children) returning id
), deleted_classrooms as (
  delete from classrooms where id in (select id from ui_classrooms) returning id
)
delete from users where id in (select id from ui_users);
`;
  spawnSync("docker", ["exec", "-i", DB_CONTAINER, "psql", "-U", "postgres", "-d", DB_NAME, "-v", "ON_ERROR_STOP=1"], {
    input: sql,
    stdio: ["pipe", "ignore", "ignore"],
  });
}

async function shutdown() {
  if (cdp) {
    cdp.close();
  }
  if (chrome && !chrome.killed) {
    chrome.kill("SIGTERM");
  }
  if (userDataDir) {
    rmSync(userDataDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
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
