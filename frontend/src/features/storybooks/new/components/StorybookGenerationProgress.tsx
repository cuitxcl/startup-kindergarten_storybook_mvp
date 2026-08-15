import { useEffect, useState } from "react";
import type { GenerationJob } from "../../../../api/client";
import { Badge } from "../../../../components/ui";
import { generationErrorMessage } from "../../../../utils/generation";

export type GenerationPhase = "idle" | "plan" | "roles" | "pages" | "references" | "done" | "failed";

export function StorybookGenerationProgress({
  phase,
  generatingStep,
  failedJob,
  materialLabels = [],
  onBackToDraft,
  onRetry,
}: {
  phase: GenerationPhase;
  generatingStep: string | null;
  failedJob: GenerationJob | null;
  materialLabels?: string[];
  onBackToDraft: () => void;
  onRetry: () => void;
}) {
  const effectivePhase = phase === "idle" && generatingStep ? phaseFromJobType(generatingStep) : phase;
  const phases: { key: GenerationPhase; label: string; copy: string }[] = [
    { key: "plan", label: "故事方向", copy: "确认故事角度和创作意图" },
    { key: "roles", label: "角色素材", copy: "整理会反复出现的人物和道具" },
    { key: "pages", label: "分页故事", copy: "生成每一页正文和画面描述" },
    { key: "references", label: "主角形象", copy: "准备跨页一致的角色形象" },
  ];
  const activeIndex = phases.findIndex((item) => item.key === effectivePhase);
  const failedPhase = failedJob ? phaseFromJobType(failedJob.jobType) : null;
  const running = !failedJob && !["idle", "done", "failed"].includes(effectivePhase);
  const completedCount = phase === "done" ? phases.length : Math.max(0, activeIndex);
  const progress = phase === "done" ? 100 : failedJob || phase === "failed" ? Math.max(8, (activeIndex + 1) * 25) : Math.max(8, (activeIndex + 1) * 25 - 8);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);

  useEffect(() => {
    if (!running) {
      setElapsedSeconds(0);
      return;
    }
    const startedAt = Date.now();
    const timer = window.setInterval(() => setElapsedSeconds(Math.floor((Date.now() - startedAt) / 1000)), 1000);
    return () => window.clearInterval(timer);
  }, [effectivePhase, running]);

  return (
    <div className="generation-progress-card">
      <div>
        <Badge tone={failedJob || phase === "failed" ? "danger" : phase === "done" ? "good" : "info"}>
          {failedJob || phase === "failed" ? "需要处理" : phase === "done" ? "已完成" : "生成中"}
        </Badge>
        <h2>{failedJob || phase === "failed" ? "作品生成没有完成" : "正在把故事画出来"}</h2>
        <p>{failedJob || phase === "failed" ? "可以重试生成，或返回大纲调整后再继续。" : materialLabels.length ? `正在使用 ${materialLabels.slice(0, 3).join("、")} 等素材生成作品。` : "系统会自动整理故事、角色和画面，完成后进入验收。"}</p>
      </div>
      {!failedJob && phase !== "failed" && (
        <div className="generation-flow-hints">
          <span>预计还需约 {remainingTimeCopy(effectivePhase)}</span>
          <span>离开页面后，回来会按当前绘本恢复进度</span>
        </div>
      )}
      <section className="generation-progress-meter" aria-label="生成进度">
        <div className="generation-progress-meter-head">
          <strong>{phase === "done" ? "绘本内容已准备完成" : failedJob || phase === "failed" ? "生成停在当前阶段" : `已完成 ${completedCount} / ${phases.length} 个阶段`}</strong>
          <span>{progress}%</span>
        </div>
        <div
          className={`generation-progress-track${running ? " is-running" : ""}`}
          role="progressbar"
          aria-label="绘本生成阶段进度"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={progress}
        >
          <span style={{ width: `${progress}%` }} />
        </div>
        {running && <small>当前步骤已等待 {elapsedCopy(elapsedSeconds)}，请保持页面打开或稍后回来查看。</small>}
      </section>
      <ol>
        {phases.map((item, index) => {
          const failed = failedPhase === item.key;
          const done = !failed && activeIndex > index;
          const running = !failed && activeIndex === index && !["idle", "done", "failed"].includes(effectivePhase);
          return (
            <li key={item.key} className={failed ? "failed" : done ? "done" : running ? "running" : ""}>
              <span>{failed ? "!" : done ? "✓" : index + 1}</span>
              <div>
                <strong>{item.label}</strong>
                <small>{running ? runningCopy(item.key) : item.copy}</small>
              </div>
            </li>
          );
        })}
      </ol>
      {failedJob && (
        <details className="compact-disclosure">
          <summary>查看错误详情</summary>
          <p className="task-summary">{generationErrorMessage(failedJob)}。可以重试这一段生成，或返回大纲调整后再继续。</p>
        </details>
      )}
      {(failedJob || phase === "failed") && (
        <div className="inline-actions">
          <button className="button primary" type="button" onClick={onRetry}>重试生成</button>
          <button className="button secondary" type="button" onClick={onBackToDraft}>返回大纲</button>
        </div>
      )}
    </div>
  );
}

function elapsedCopy(seconds: number) {
  if (seconds < 60) return `${seconds} 秒`;
  return `${Math.floor(seconds / 60)} 分 ${seconds % 60} 秒`;
}

function remainingTimeCopy(phase: GenerationPhase) {
  if (phase === "references") return "10-30 秒";
  if (phase === "pages") return "30-60 秒";
  if (phase === "roles") return "45-90 秒";
  return "1-2 分钟";
}

function phaseFromJobType(jobType: string): GenerationPhase {
  if (jobType === "storybook_plan") return "plan";
  if (jobType === "storybook_roles") return "roles";
  if (jobType === "storybook_pages") return "pages";
  if (jobType === "storybook_role_reference_image") return "references";
  return "idle";
}

function runningCopy(phase: GenerationPhase) {
  if (phase === "roles") return "正在整理角色和道具...";
  if (phase === "pages") return "正在生成分页图文...";
  if (phase === "references") return "正在准备跨页一致的参考图...";
  return "正在整理故事方向...";
}
