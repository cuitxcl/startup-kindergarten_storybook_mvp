import { getGenerationJob, type GenerationJob } from "../api/client";
import { generationJobStatusLabel, generationPrivacyAuditSummary } from "./labels";

export function isActiveJobStatus(status: string) {
  return status === "queued" || status === "running";
}

export function generationErrorMessage(job: Pick<GenerationJob, "output" | "lastError">) {
  const output = job.output as { error?: { message?: string } } | undefined;
  const message = output?.error?.message || job.lastError || "";
  if (/provider 输出 .*storybook_plan.*outline.*page_range/.test(message)) {
    return "故事大纲格式不完整，系统未能自动修复";
  }
  if (/provider 输出 .*storybook_plan/.test(message)) {
    return "故事大纲格式不完整，系统未能自动修复";
  }
  return message || "生成任务失败，可稍后重试";
}

export function generationStatusLabel(status: string) {
  if (status === "queued") return "已加入队列";
  return generationJobStatusLabel[status] || `状态：${status}`;
}

export function generationOutputMeta(output: unknown) {
  const value = output as { provider?: string; schema_version?: string; mode?: string; message?: string } | undefined;
  return {
    provider: value?.provider || "待生成",
    schema: value?.schema_version || "尚无输出",
    mode: value?.mode || "等待任务",
    message: value?.message || "生成后会在这里显示可审核内容。",
    real: value?.schema_version === "generation.provider.v1",
    privacy: generationPrivacyAuditSummary(output),
  };
}

type PollOptions = {
  /** 轮询间隔，默认 1000ms */
  intervalMs?: number;
  /** 最长等待时间，默认 3 分钟；超时后返回最后一次状态，由调用方决定提示 */
  timeoutMs?: number;
  /** 每次状态刷新时回调（包括初始状态） */
  onUpdate?: (job: GenerationJob) => void;
};

function sleep(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

/**
 * 统一的任务轮询（生成任务、导出任务通用）。
 * - 固定 1s 间隔、按耗时预算超时（取代各处不一致的尝试次数口径）
 * - 页面在后台时降频轮询而不是停摆：生成结果回来后无论页面是否在前台都能继续流程
 */
export async function pollUntilSettled<T extends { status: string }>(
  fetcher: () => Promise<T>,
  initial: T,
  options: { intervalMs?: number; timeoutMs?: number; onUpdate?: (job: T) => void } = {},
): Promise<T> {
  const intervalMs = options.intervalMs ?? 1000;
  const timeoutMs = options.timeoutMs ?? 180_000;
  const startedAt = Date.now();
  let current = initial;
  options.onUpdate?.(current);
  while (isActiveJobStatus(current.status)) {
    if (Date.now() - startedAt > timeoutMs) return current;
    const hidden = typeof document !== "undefined" && document.visibilityState === "hidden";
    await sleep(hidden ? Math.max(intervalMs * 5, 3000) : intervalMs);
    current = await fetcher();
    options.onUpdate?.(current);
  }
  return current;
}

/** 生成任务轮询（pollUntilSettled 的便捷封装）。 */
export async function pollGenerationJob(
  workspaceId: string,
  initialJob: GenerationJob,
  options: PollOptions = {},
): Promise<GenerationJob> {
  return pollUntilSettled(() => getGenerationJob(workspaceId, initialJob.id), initialJob, options);
}
