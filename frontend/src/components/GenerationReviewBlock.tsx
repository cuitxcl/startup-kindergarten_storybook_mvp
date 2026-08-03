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
  onRegenerate,
  onEdit,
  editor,
  editing = false,
  regenerating = false,
}: {
  title: string;
  items: string[];
  output?: unknown;
  showMeta?: boolean;
  onRegenerate?: () => void;
  onEdit?: () => void;
  editor?: ReactNode;
  editing?: boolean;
  regenerating?: boolean;
}) {
  const meta = showMeta ? generationOutputMeta(output) : null;
  const hasActions = Boolean(onRegenerate || onEdit);
  return (
    <div className="review-block">
      {meta ? (
        <>
          <div className="section-head compact">
            <div>
              <p className="eyebrow">老师审核</p>
              <h2>{title}</h2>
              <p>{meta.message}</p>
            </div>
            <Badge tone={meta.real ? "good" : "neutral"}>{meta.real ? "真实生成" : meta.provider}</Badge>
          </div>
          <div className="review-meta">
            <span>来源：{meta.provider}</span>
            <span>任务：{generationModeLabel(meta.mode)}</span>
            <span>结构：{meta.schema}</span>
            {meta.privacy && <span>{meta.privacy}</span>}
          </div>
        </>
      ) : (
        <h2>{title}</h2>
      )}
      {editing && editor ? (
        editor
      ) : (
        <div className="review-list">
          {items.map((item) => <div key={item}><span>确认项</span><strong>{item}</strong></div>)}
        </div>
      )}
      {hasActions && (
        <div className="inline-actions">
          {onRegenerate && (
            <button className="button secondary" type="button" disabled={regenerating} onClick={onRegenerate}>{regenerating ? "生成中..." : "重新生成"}</button>
          )}
          {onEdit && (
            <button className="button secondary" type="button" onClick={onEdit}>{editing ? "收起修改" : "手动修改"}</button>
          )}
        </div>
      )}
    </div>
  );
}
