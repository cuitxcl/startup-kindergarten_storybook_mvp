import type { ReactNode } from "react";
import type { ExportJob, GenerationJob, ShareLink } from "../../../api/client";
import type { Storybook, StorybookQualityReport, StorybookRole } from "../../../types/domain";
import { generationErrorMessage } from "../../../utils/generation";
import { generationJobTypeLabel } from "../../../utils/labels";

export function qualityStatusLabel(status: string) {
  return {
    passed: "检查通过",
    needs_review: "需要复核",
    blocked: "存在阻断",
  }[status] || status;
}

export function qualityTone(status: string): "neutral" | "good" | "warn" | "danger" | "info" {
  if (status === "passed") return "good";
  if (status === "blocked") return "danger";
  if (status === "needs_review") return "warn";
  return "neutral";
}

export function qualityPageSummary(page: StorybookQualityReport["pages"][number]) {
  if (page.issues.length && page.suggestions.length) return `${page.issues.length} 个问题，${page.suggestions.length} 条建议。`;
  if (page.issues.length) return `${page.issues.length} 个问题需要先处理。`;
  if (page.suggestions.length) return `${page.suggestions.length} 条建议，老师确认后可继续。`;
  return "这一页暂未发现明显问题。";
}

export function teacherReviewLabel(status?: string) {
  return status === "confirmed" ? "老师已复核" : "待老师复核";
}

export function buildLocalStorybookQuality(book: Storybook): StorybookQualityReport {
  const consistencyRoles = book.roles.filter((role) => roleNeedsReference(book, role));
  const checks: StorybookQualityReport["checks"] = [
    {
      key: "structure",
      label: "内容结构",
      status: book.pages.length && book.roles.length ? "passed" : "blocked",
      message: book.pages.length && book.roles.length ? "已包含分页内容和角色/道具设定。" : "分页或角色设定不完整。",
    },
  ];
  const missingReferences = consistencyRoles.filter((role) => !role.referenceImageUrl);
  const staleReferences = consistencyRoles.filter((role) => role.referenceImageUrl && role.referenceStatus !== "ready");
  checks.push({
    key: "role_references",
    label: "角色参考图",
    status: missingReferences.length || staleReferences.length ? "needs_review" : "passed",
    message: !consistencyRoles.length
      ? "没有跨页重复出现的角色或道具需要参考图；只出现一次的事物无需参考图。"
      : missingReferences.length
      ? staleReferences.length
        ? `缺少参考图：${missingReferences.map((role) => role.name).join("、")}；建议更新参考图：${staleReferences.map((role) => role.name).join("、")}。`
        : `以下跨页出现的角色/道具还需要先生成参考图：${missingReferences.map((role) => role.name).join("、")}。`
      : staleReferences.length
        ? `以下角色/道具已有参考图但建议更新：${staleReferences.map((role) => role.name).join("、")}；当前已有图仍可用于生成。`
        : "跨页重复出现的角色/道具都已有参考图；只出现一次的事物无需参考图。",
  });

  let blockedPages = 0;
  let reviewPages = 0;
  const pages = book.pages.map((page) => {
    const issues: string[] = [];
    const suggestions: string[] = [];
    const pageText = `${page.title} ${page.body}`;
    const combinedText = `${pageText} ${page.illustrationPrompt}`;
    const pageRoles = book.roles.filter((role) => combinedText.includes(role.name));
    const promptHasConfirmedRole = pageRoles.some((role) => page.illustrationPrompt.includes(role.name));
    if (pageRoles.length && !promptHasConfirmedRole) issues.push("插图描述没有明确带入已确认角色/道具名称。");
    if (page.status === "generating") issues.push("插图仍在生成中。");
    if (page.status === "failed") issues.push("插图生成失败，需要重新生成。");
    if (page.status === "needs_regeneration") suggestions.push("当前页标记为需重绘，建议重新生成插图。");
    pageRoles.forEach((role) => {
      if (pageText.includes(role.name) && !page.illustrationPrompt.includes(role.name)) {
        issues.push(`正文出现了「${role.name}」，但插图描述没有同步这个名称。`);
      }
    });
    const status: StorybookQualityReport["status"] = issues.length ? "blocked" : suggestions.length ? "needs_review" : "passed";
    if (status === "blocked") blockedPages += 1;
    if (status === "needs_review") reviewPages += 1;
    return {
      pageId: page.id,
      pageNumber: page.pageNumber,
      status,
      issues,
      suggestions,
    };
  });

  checks.push({
    key: "page_prompts",
    label: "分页一致性",
    status: blockedPages ? "blocked" : reviewPages ? "needs_review" : pages.length ? "passed" : "blocked",
    message: blockedPages
      ? `${blockedPages} 个分页存在阻断问题，需要先修正插图描述或重新生成。`
      : reviewPages
        ? `${reviewPages} 个分页需要老师复核或补充描述。`
        : pages.length
          ? "分页描述已带入角色/道具名称，没有发现明显一致性问题。"
          : "还没有可检查的分页。",
  });

  const status: StorybookQualityReport["status"] = checks.some((check) => check.status === "blocked")
    ? "blocked" as const
    : checks.some((check) => check.status === "needs_review")
      ? "needs_review" as const
      : "passed" as const;
  return {
    status,
    summary: status === "passed"
      ? "系统检查通过，建议老师做最终阅读确认。"
      : status === "blocked"
        ? "系统发现阻断问题，请先修正角色、插图描述或重新生成。"
        : "系统发现需要复核的项目，建议老师确认后再导出或分享。",
    checks,
    pages,
  };
}

export function customizationBlockerFor(book: Storybook, quality?: StorybookQualityReport) {
  if (book.type !== "plain") return "只有普通绘本可以继续创作专属版本";
  if (!book.pages.length) return "请先生成绘本分页";
  if (!book.roles.length) return "请先确认角色与道具";
  const generatingPages = book.pages.filter((page) => page.status === "generating");
  if (generatingPages.length) return "仍有分页插图正在生成，请完成后再创作专属版本";
  const failedPages = book.pages.filter((page) => page.status === "failed");
  if (failedPages.length) return "仍有分页插图生成失败，请修复后再创作专属版本";
  const redrawPages = book.pages.filter((page) => page.status === "needs_regeneration");
  if (redrawPages.length) return `仍有 ${redrawPages.length} 页需要重绘，请先完成基础故事`;
  const missingReferences = book.roles.filter((role) => roleNeedsReference(book, role) && (role.referenceStatus !== "ready" || !role.referenceImageUrl));
  if (missingReferences.length) return `跨页角色参考图未完成：${missingReferences.map((role) => role.name).join("、")}`;
  if (quality?.status === "blocked") return "质量检查存在阻断项，请先修正";
  if (book.status !== "exportable" && book.status !== "listed") return "请先完成基础故事的整本验收";
  return "";
}

export function roleNeedsReference(book: Storybook, role: StorybookRole) {
  return role.needsConsistency && rolePageUsageCount(book, role) >= 2;
}

export function activeRoleReferenceJob(jobs: GenerationJob[], roleId: string) {
  return jobs.find((job) => {
    if (job.jobType !== "storybook_role_reference_image") return false;
    if (job.status !== "queued" && job.status !== "running") return false;
    const input = job.input;
    if (!input || typeof input !== "object" || !("role_id" in input)) return false;
    return (input as { role_id?: unknown }).role_id === roleId;
  });
}

export function rolePageUsageCount(book: Storybook, role: StorybookRole) {
  return book.pages.filter((page) => `${page.title} ${page.body} ${page.illustrationPrompt}`.includes(role.name)).length;
}

export function roleLabelMap(roleType: string) {
  return {
    protagonist: "主角",
    supporting: "配角",
    peer: "同伴角色",
    teacher: "老师",
    prop: "道具",
  }[roleType] || roleType;
}

export function visibilityLabel(value: string) {
  return {
    private: "仅当前空间私有",
    workspace: "园所/空间内共享",
    market_submission: "市场投稿中",
    market_listed: "市场已上架",
  }[value] || value;
}

export function exportStatusLabel(status: string) {
  return {
    queued: "排队中",
    running: "导出中",
    succeeded: "已完成",
    failed: "导出失败",
  }[status] || status;
}

export function exportFailureText(job: ExportJob) {
  return job.lastError ? `失败原因：${job.lastError}` : "导出任务没有成功完成，请稍后重新导出。";
}

export function shareExpiryToIso(value: "7d" | "30d" | "never") {
  if (value === "never") return undefined;
  const days = value === "30d" ? 30 : 7;
  const expiresAt = new Date();
  expiresAt.setDate(expiresAt.getDate() + days);
  return expiresAt.toISOString();
}

export function shareExpiryLabel(expiresAt?: string) {
  if (!expiresAt) return "长期有效";
  return `有效期至 ${new Date(expiresAt).toLocaleDateString("zh-CN")}`;
}

export function shareAccessLabel(link: ShareLink) {
  if (!link.accessCount) return "尚未访问";
  const lastAccess = link.lastAccessedAt ? `，最后访问 ${link.lastAccessedAt}` : "";
  return `已访问 ${link.accessCount} 次${lastAccess}`;
}

export function pageImageActionLabel(pageStatus: string, generating = false) {
  if (generating) return "生成中...";
  if (pageStatus === "needs_regeneration" || pageStatus === "failed") return "重新生成这一页";
  if (pageStatus === "ready") return "重画这一页";
  return "生成这一页";
}

export function roleReferenceStatusLabel(status?: string) {
  return {
    not_started: "未生成参考图",
    generating: "参考图生成中",
    ready: "参考图已确认",
    needs_regeneration: "需要重绘",
    failed: "生成失败",
  }[status || "not_started"] || "参考图待确认";
}

function roleReferenceStyleClause(coverTone: string) {
  const trimmed = coverTone.trim().replace(/。+$/, "");
  if (!trimmed || trimmed === "温暖、清楚") {
    return "柔和水彩绘本风格，圆润饱满造型，大而富有表现力的眼睛";
  }
  if (trimmed.includes("皮克斯") || trimmed.includes("3D")) {
    return `${trimmed}，高质量3D动画电影质感，立体圆润角色，柔和棚拍光，细腻材质，真实体积感`;
  }
  return trimmed;
}

export function buildRoleReferencePrompt(role: Pick<StorybookRole, "name" | "roleType" | "appearance">, coverTone: string) {
  const name = role.name.trim() || "未命名角色";
  const appearance = cleanVisualAppearance(role.appearance) || "请先补充外观设定";
  const style = roleReferenceStyleClause(coverTone);
  const anatomy = roleReferenceAnatomyClause(role, appearance);
  return `${name}，${roleTypeLabel(role.roleType)}，${appearance}，画面风格必须与整本绘本一致：${style}，${anatomy}，表情自然生动、富有神采，姿态自然放松，单一角色标准图，白底或简洁背景，清晰展示完整轮廓或半身，可微微侧身，画面只有这个角色，无人类，无其他角色，保持跨页一致，不要僵硬对称的证件照式站姿`;
}

function isLimbFreeRole(role: Pick<StorybookRole, "name" | "roleType">, appearance: string) {
  const text = `${role.name} ${role.roleType} ${appearance}`;
  return ["无手", "没有手", "无脚", "没有脚", "无手和脚", "没有手和脚", "无四肢", "没有四肢", "蛇", "小蛇", "蚯蚓", "毛毛虫", "蜗牛", "球形"].some((keyword) =>
    text.includes(keyword),
  );
}

function roleReferenceAnatomyClause(role: Pick<StorybookRole, "name" | "roleType">, appearance: string) {
  if (isLimbFreeRole(role, appearance)) {
    return "身体结构必须严格符合外观设定：没有手、没有脚、没有手臂和腿，不要生成手指、鞋子、胳膊或人形四肢，用头部、眼睛、身体弯曲、尾部和整体姿态表达动作";
  }
  return "身体结构必须严格符合外观设定；有手、脚、爪或翅膀时可以清晰表现，但不要凭空添加外观没有写到的肢体";
}

export function cleanVisualAppearance(value: string) {
  const behaviorKeywords = [
    "喜欢", "总喜欢", "经常", "常常", "总是", "常和", "离开队伍", "交流", "适合", "带领",
    "制定", "强调", "学习", "代表", "推动", "帮助", "引导", "鼓励", "提醒", "跑", "跳", "蹦",
    "玩", "等待", "分享",
  ];
  const parts = value
    .split(/[，,。；;、]/)
    .map((part) => part.trim())
    .filter(Boolean)
    .filter((part) => !behaviorKeywords.some((keyword) => part.includes(keyword)));
  return parts.join("，") || value.trim();
}

export function roleTypeLabel(roleType: StorybookRole["roleType"]) {
  return {
    protagonist: "主角",
    supporting: "配角",
    peer: "同伴角色",
    teacher: "老师形象",
    prop: "关键道具",
  }[roleType] || "角色";
}

export function compactPromptSummary(prompt: string) {
  const text = prompt.replace(/\s+/g, " ").trim();
  if (!text) return "还没有插图描述";
  const withoutPrefix = text.replace(/^儿童绘本插图，?/, "").replace(/^画面风格[:：]\s*[^。]+。?/, "").trim();
  const summary = withoutPrefix || text;
  return summary.length > 64 ? `${summary.slice(0, 64)}...` : summary;
}

export function illustrationShotLabel(prompt: string) {
  const match = prompt.match(/(远景|全景|中景|中近景|近景|特写|局部特写|俯视|跟随视角)/);
  return match?.[1] || "镜头";
}

export function extractPageId(output: unknown) {
  const value = output as { image?: { page_id?: string; target_id?: string; target_type?: string } } | undefined;
  if (value?.image?.page_id) return value.image.page_id;
  return value?.image?.target_type === "page" ? value.image.target_id : undefined;
}

export function extractPageIdFromInput(input: unknown) {
  const value = input as { page_id?: string; target_id?: string; target_type?: string } | undefined;
  if (value?.page_id) return value.page_id;
  return value?.target_type === "page" ? value.target_id : undefined;
}

export function extractImageResult(output: unknown): { imageUrl: string; altText?: string; prompt?: string; styleNotes: string[]; provider?: string; message?: string } | null {
  const value = output as {
    provider?: string;
    message?: string;
    image?: {
      image_url?: string;
      alt_text?: string;
      prompt?: string;
      style_notes?: string[];
    };
  } | undefined;
  const image = value?.image;
  if (!image?.image_url) return null;
  return {
    imageUrl: image.image_url,
    altText: image.alt_text,
    prompt: image.prompt,
    styleNotes: image.style_notes || [],
    provider: value?.provider,
    message: value?.message,
  };
}

export function latestPageImageJob(jobs: GenerationJob[], pageId?: string) {
  if (!pageId) return undefined;
  return jobs
    .filter((job) => job.jobType === "storybook_page_image" && job.output && extractPageId(job.output) === pageId)
    .sort((a, b) => generationJobTimestamp(b) - generationJobTimestamp(a))[0];
}

export function activePageImageJob(jobs: GenerationJob[], pageId?: string) {
  if (!pageId) return undefined;
  return jobs
    .filter((job) => (
      job.jobType === "storybook_page_image"
      && (job.status === "queued" || job.status === "running")
      && extractPageIdFromInput(job.input) === pageId
    ))
    .sort((a, b) => generationJobTimestamp(b) - generationJobTimestamp(a))[0];
}

export function generationJobTimestamp(job: GenerationJob) {
  return new Date(job.finishedAt || job.createdAt).getTime();
}

export function generationJobIdFromImageUrl(url: string) {
  return url.match(/\/generation-jobs\/([^/]+)\/image/)?.[1];
}

export function generationJobTitle(job: GenerationJob) {
  return generationJobTypeLabel[job.jobType] || job.jobType;
}

export function generationJobCopy(job: GenerationJob) {
  if (job.status === "failed") return generationErrorMessage(job);
  if (job.status === "queued") return "任务已进入队列。";
  if (job.status === "running") return "任务正在生成中。";
  if (job.status === "canceled") return "任务已取消，不会继续执行。";
  if (job.storybookId) return "已写入本书内容。";
  return "已生成结构化结果。";
}

export function generationJobTime(job: GenerationJob) {
  return job.finishedAt || job.createdAt;
}

export function resultNoticeFromSearch(search: string): { title: string; copy: string; tone: "good"; action?: ReactNode } | null {
  const params = new URLSearchParams(search);
  const result = params.get("result");
  const from = params.get("from");
  if (result === "plain") {
    if (from === "new") {
      return {
        title: "图文草稿已生成",
        copy: "可以开始检查插图、正文和角色一致性，细节还能在这里继续调整。",
        tone: "good",
      };
    }
    return {
      title: "生成结果已展示",
      copy: "基础故事已经生成完成。请先检查故事、角色和分页插图，再导出 PDF 或派生定制版本。",
      tone: "good",
    };
  }
  if (result === "custom") {
    return {
      title: "定制结果已展示",
      copy: "这本定制绘本已经生成完成。请检查儿童信息、故事改写和插图一致性，再导出或分享给家长。",
      tone: "good",
    };
  }
  if (result === "personalized") {
    return {
      title: "专属绘本已生成",
      copy: "请在修改与交付页检查素材落点、分页内容和插图一致性，确认没有问题后再导出或分享。",
      tone: "good",
    };
  }
  if (result === "batch-custom") {
    return {
      title: "批量定制结果已展示",
      copy: "已打开第一本定制绘本。请从这里开始逐本检查儿童信息、故事改写和插图一致性。",
      tone: "good",
    };
  }
  return null;
}

export function canCancelGenerationJob(job: GenerationJob) {
  return job.status === "queued" || job.status === "failed";
}
