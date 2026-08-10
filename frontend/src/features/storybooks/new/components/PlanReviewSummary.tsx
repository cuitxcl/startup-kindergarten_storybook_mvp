import { Badge } from "../../../../components/ui";
import { linesFromRows } from "../helpers";
import type { EditablePlan, StorybookRequestForm } from "../types";

export function PlanReviewSummary({
  form,
  plan,
}: {
  form: StorybookRequestForm;
  plan: EditablePlan;
}) {
  const outline = linesFromRows(plan.outlineText);
  const roleLines = linesFromRows(plan.roleRequirementsText);
  const reviewPoints = linesFromRows(plan.reviewPointsText);
  const storyArc = outline.slice(0, 4).map((line) => compactLine(line));

  return (
    <div className="plan-review-summary">
      <section className="plan-summary-grid" aria-label="方案摘要">
        <div className="plan-summary-card">
          <span>故事主题</span>
          <strong>{form.title || "未命名绘本"}</strong>
          <p>{form.theme || "待确认教学目标"}</p>
        </div>
        <div className="plan-summary-card">
          <span>故事结构</span>
          <strong>{storyArc.length ? storyArc.join(" → ") : "待生成分页节奏"}</strong>
          <p>{plan.summary || "生成后会展示故事概述。"}</p>
        </div>
        <div className="plan-summary-card">
          <span>老师确认</span>
          <strong>{reviewPoints[0] || "故事是否适合班级共读"}</strong>
          <p>{reviewPoints.slice(1, 3).join("；") || "确认教学目标、角色设定和分页节奏即可继续。"}</p>
        </div>
      </section>

      <section className="plan-review-section">
        <div className="section-title-row">
          <div>
            <h3>分页节奏</h3>
            <p>先扫每页作用，点“手动修改”再改完整文本。</p>
          </div>
          <Badge tone="info">{outline.length || Number(form.pageCount) || 0} 页</Badge>
        </div>
        <div className="plan-page-grid">
          {outline.length ? outline.map((line, index) => {
            const parsed = splitPageLine(line);
            return (
              <article className="plan-page-card" key={`${index}-${line}`}>
                <span>第 {index + 1} 页</span>
                <strong>{parsed.title || "分页情节"}</strong>
                <p>{parsed.detail || line}</p>
              </article>
            );
          }) : (
            <div className="plan-empty-note">生成后会按页展示故事节奏。</div>
          )}
        </div>
      </section>

      <section className="plan-review-section">
        <div className="section-title-row">
          <div>
            <h3>角色与场景</h3>
            <p>确认这些角色和场景是否符合你的课堂目标。</p>
          </div>
        </div>
        <div className="plan-tag-groups">
          {roleLines.length ? roleLines.map((line) => {
            const [label, content] = splitRoleLine(line);
            return (
              <div className="plan-tag-group" key={line}>
                <strong>{label}</strong>
                <div>
                  {splitTags(content).map((tag) => (
                    <span key={tag}>{tag}</span>
                  ))}
                </div>
              </div>
            );
          }) : (
            <div className="plan-empty-note">生成后会展示主角、配角、场景和关键道具。</div>
          )}
        </div>
      </section>
    </div>
  );
}

function compactLine(line: string) {
  const text = line
    .replace(/^第\s*\d+\s*页[:：]?\s*/, "")
    .replace(/\s+/g, " ")
    .trim();
  const [first] = text.split(/[：:，,。；;-]/).map((part) => part.trim()).filter(Boolean);
  return (first || text).slice(0, 12);
}

function splitPageLine(line: string) {
  const text = line.replace(/^第\s*\d+\s*页[:：]?\s*/, "").trim();
  const [title, ...rest] = text.split(/\s*[-—－]\s*/);
  if (rest.length) return { title: title.trim(), detail: rest.join(" - ").trim() };
  const [lead, ...tail] = text.split(/[：:]/);
  return { title: lead?.trim(), detail: tail.join("：").trim() };
}

function splitRoleLine(line: string) {
  const [label, ...rest] = line.split(/[：:]/);
  return [label?.trim() || "角色", rest.join("：").trim() || line] as const;
}

function splitTags(value: string) {
  return value
    .split(/[，,、；;\n]/)
    .map((part) => part.trim())
    .filter(Boolean);
}
