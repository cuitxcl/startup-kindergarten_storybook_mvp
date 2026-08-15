import { type ReactNode } from "react";
import { Badge } from "./ui";
import { generationOutputMeta } from "../utils/generation";
import { generationJobTypeLabel } from "../utils/labels";

function generationModeLabel(mode: string) {
  if (mode === "等待任务") return "等待任务";
  return generationJobTypeLabel[mode] || mode;
}

/**
 * 生成结果复核区块：新建向导与定制向导共用。
 * - showMeta 控制是否渲染来源/任务/结构元信息（默认 output 存在时显示）。
 * - 传入 onRegenerate 或 onEdit 任一回调时才渲染操作按钮行。
 */
export function GenerationReviewBlock({
  title,
  items,
  output,
  showMeta = output !== undefined,
  variant = "draft",
  onRegenerate,
  onEdit,
  editor,
  reviewContent,
  editing = false,
  regenerating = false,
  regenerateLabel = "重新生成",
  editLabel = "手动修改",
  collapseLabel = "收起编辑",
}: {
  title: string;
  items: string[];
  output?: unknown;
  showMeta?: boolean;
  variant?: "draft" | "technical";
  onRegenerate?: () => void;
  onEdit?: () => void;
  editor?: ReactNode;
  reviewContent?: ReactNode;
  editing?: boolean;
  regenerating?: boolean;
  regenerateLabel?: string;
  editLabel?: string;
  collapseLabel?: string;
}) {
  const meta = showMeta ? generationOutputMeta(output) : null;
  const hasActions = Boolean(onRegenerate || onEdit);
  const draftMode = variant === "draft";
  return (
    <div className={`review-block review-block-${variant}`}>
      {meta ? (
        <>
          <div className="section-head compact">
            <div>
              {!draftMode && <p className="eyebrow">老师审核</p>}
              <h2>{title}</h2>
              {!draftMode && <p>{meta.message}</p>}
            </div>
            {!draftMode && (
              <div className="review-block-head-actions">
                <Badge tone={meta.real ? "good" : "neutral"}>{meta.real ? "真实生成" : meta.provider}</Badge>
              </div>
            )}
          </div>
          {draftMode ? (
            <details className="compact-disclosure technical-details">
              <summary>生成记录</summary>
              <div className="review-meta">
                <span>来源：{meta.provider}</span>
                <span>任务：{generationModeLabel(meta.mode)}</span>
                <span>结构：{meta.schema}</span>
                {meta.privacy && <span>{meta.privacy}</span>}
              </div>
            </details>
          ) : (
            <div className="review-meta">
              <span>来源：{meta.provider}</span>
              <span>任务：{generationModeLabel(meta.mode)}</span>
              <span>结构：{meta.schema}</span>
              {meta.privacy && <span>{meta.privacy}</span>}
            </div>
          )}
          {hasActions && (
            <details className="section-tools review-tools-disclosure">
              <summary>{draftMode ? "更多操作" : editing ? "正在编辑，可收起或重新生成" : "复核工具"}</summary>
              <div className="review-block-tools">
                {onRegenerate && (
                  <button className="button secondary" type="button" disabled={regenerating} onClick={onRegenerate}>
                    {regenerating ? "生成中..." : regenerateLabel}
                  </button>
                )}
                {onEdit && (
                  <button className={editing ? "button ghost" : "button secondary"} type="button" onClick={onEdit}>
                    {editing ? collapseLabel : editLabel}
                  </button>
                )}
              </div>
            </details>
          )}
        </>
      ) : (
        <h2>{title}</h2>
      )}
      {editing && editor ? (
        editor
      ) : reviewContent ? (
        reviewContent
      ) : (
        <div className="review-list">
          {(draftMode ? items.slice(0, 3) : items).map((item) => <div key={item}><span>{draftMode ? "摘要" : "确认项"}</span><strong>{item}</strong></div>)}
        </div>
      )}
    </div>
  );
}
