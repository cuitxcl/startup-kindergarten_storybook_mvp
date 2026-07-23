import type { Storybook, StorybookPage, WorkspaceRole } from "../types/domain";

export const storybookStatusLabel: Record<Storybook["status"], string> = {
  draft: "草稿",
  plan_pending: "方案待确认",
  roles_pending: "角色待确认",
  editing: "编辑中",
  image_pending: "插图待生成",
  exportable: "可导出",
  submitted: "投稿审核中",
  listed: "已上架",
};

export const pageStatusLabel: Record<StorybookPage["status"], string> = {
  ready: "插图已完成",
  needs_regeneration: "需要重绘",
  generating: "生成中",
};

export const roleLabel: Record<WorkspaceRole, string> = {
  personal_owner: "个人拥有者",
  school_teacher: "老师",
  school_admin: "管理员",
  platform_operator: "平台运营",
};

export const memberStatusLabel: Record<string, string> = {
  active: "已加入",
  invited: "待接受",
  expired: "已过期",
  revoked: "已撤销",
};

export const submissionStatusLabel: Record<string, string> = {
  draft: "投稿草稿",
  submitted: "待审核",
  approved: "已通过",
  rejected: "已驳回",
  listed: "已上架",
  unlisted: "已下架",
};

export const classroomStatusLabel: Record<string, string> = {
  active: "使用中",
  archived: "已归档",
};

export const generationJobTypeLabel: Record<string, string> = {
  storybook_plan: "故事方案",
  storybook_roles: "角色与道具",
  storybook_pages: "分页图文",
  storybook_role_reference_image: "角色参考图",
  storybook_page_image: "插图生成",
  customization_plan: "定制方案",
};

export const generationJobStatusLabel: Record<string, string> = {
  queued: "排队中",
  running: "正在生成",
  succeeded: "已完成",
  failed: "生成失败",
  canceled: "已取消",
};

export function generationJobNextAction(job: { status: string; lastError?: string | null; nextRunAt?: string | null }) {
  if (job.status === "failed") return job.lastError ? `可重试：${job.lastError}` : "可稍后重试";
  if (job.status === "queued") return job.nextRunAt ? `预计下次执行：${job.nextRunAt}` : "等待 worker 领取";
  if (job.status === "running") return "正在执行中，完成后会回写结果";
  if (job.status === "succeeded") return "已完成，可继续查看对应内容";
  if (job.status === "canceled") return "已停止，不会继续执行";
  return "状态未知";
}

const generationPrivacyAuditLabels: Record<string, string> = {
  sensitive_field: "儿童/家长资料",
  uuid: "内部 ID",
  email: "邮箱",
  phone: "手机号",
  private_detail: "家庭或医疗信息",
};

export function generationPrivacyAuditSummary(output: unknown) {
  const value = output as {
    privacy_audit?: { redacted?: boolean; labels?: unknown[] };
    image?: { privacy_audit?: { redacted?: boolean; labels?: unknown[] } };
  } | null | undefined;
  const audit = value?.privacy_audit || value?.image?.privacy_audit;
  if (!audit?.redacted) return null;
  const labels = (audit.labels || [])
    .map((label) => typeof label === "string" ? generationPrivacyAuditLabels[label] || label : "")
    .filter(Boolean);
  return labels.length ? `已脱敏：${labels.join("、")}` : "已脱敏敏感信息";
}

export function storybookNextAction(book: Storybook) {
  if (book.status === "exportable") return "可导出 PDF，也可继续生成定制副本";
  if (book.status === "submitted") return "等待审核，重点确认隐私和投稿信息";
  if (book.status === "image_pending") return "继续生成或重绘插图";
  if (book.status === "roles_pending") return "确认主角和重复出现的配角";
  if (book.status === "plan_pending") return "确认故事方案后再生成角色";
  if (book.status === "listed") return "已在市场中可被复制复用";
  return "继续编辑文字、分页和角色";
}

export function storybookSourceLabel(book: Storybook) {
  if (book.source === "duplicate") return book.sourceTitle ? `复制自《${book.sourceTitle}》` : "复制副本";
  if (book.source === "marketplace") return book.sourceTitle ? `来自市场《${book.sourceTitle}》` : "来自市场";
  if (book.source === "derived") return book.sourceTitle ? `定制自《${book.sourceTitle}》` : "定制副本";
  return "原创绘本";
}
