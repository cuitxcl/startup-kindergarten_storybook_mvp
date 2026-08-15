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
  const summary = alignedSummary(plan.summary, form);

  return (
    <div className="plan-review-summary">
      <section className="plan-summary-grid draft-summary-grid" aria-label="故事草稿摘要">
        <div className="plan-summary-card">
          <span>故事主题</span>
          <strong>{form.title || "未命名绘本"}</strong>
          <p>{form.theme || "待确认教学目标"}</p>
        </div>
        <div className="plan-summary-card">
          <span>故事走向</span>
          <strong>{storyArc.length ? storyArc.join(" → ") : "待生成分页节奏"}</strong>
          <p>{summary}</p>
        </div>
        <div className="plan-summary-card">
          <span>适合这样讲吗？</span>
          <strong>{reviewPoints[0] || "故事是否适合班级共读"}</strong>
          <p>{reviewPoints.slice(1, 2).join("；") || "看方向即可，细节之后还能改。"}</p>
        </div>
      </section>

      <section className="plan-review-section">
        <div className="section-title-row">
          <div>
            <h3>故事怎么展开</h3>
            <p>看方向即可，细节之后还能改。</p>
          </div>
          <Badge tone="info">{outline.length || Number(form.pageCount) || 0} 页</Badge>
        </div>
        <div className="plan-page-grid draft-page-list">
          {outline.length ? outline.map((line, index) => {
            const parsed = splitPageLine(line);
            const showDetail = parsed.detail && parsed.detail !== parsed.title;
            return (
              <article className="plan-page-card" key={`${index}-${line}`}>
                <span>{parsed.pageLabel || `第 ${index + 1} 页`}</span>
                <strong>{parsed.title || "分页情节"}</strong>
                {showDetail && <p>{parsed.detail}</p>}
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
            <h3>会出现的人物和场景</h3>
            <p>默认会按这些元素继续生成图文。</p>
          </div>
        </div>
        <div className="plan-tag-groups">
          {roleLines.length ? roleLines.slice(0, 2).map((line) => {
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
        {roleLines.length > 2 && (
          <details className="compact-disclosure">
            <summary>查看更多人物和场景</summary>
            <div className="plan-tag-groups">
              {roleLines.slice(2).map((line) => {
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
              })}
            </div>
          </details>
        )}
      </section>
    </div>
  );
}

function compactLine(line: string) {
  const parsed = splitPageLine(line);
  const text = (parsed.title || parsed.detail || line)
    .replace(/\s+/g, " ")
    .trim();
  const [first] = text.split(/[：:，,。；;]/).map((part) => part.trim()).filter(Boolean);
  return (first || text).slice(0, 12);
}

function splitPageLine(line: string) {
  const raw = line.trim();
  const pageMatch = raw.match(/^(第\s*[\d一二三四五六七八九十]+(?:\s*[-—－~至到]\s*[\d一二三四五六七八九十]+)?\s*页)\s*[:：]?\s*/);
  const pageLabel = pageMatch?.[1]?.replace(/\s+/g, " ");
  const text = pageMatch ? raw.slice(pageMatch[0].length).trim() : raw;
  const [title, ...rest] = text.split(/\s*[-—－]\s*/);
  if (rest.length) return { pageLabel, title: title.trim(), detail: rest.join(" - ").trim() };
  const [lead, ...tail] = text.split(/[：:]/);
  return { pageLabel, title: lead?.trim(), detail: tail.join("：").trim() };
}

function alignedSummary(summary: string, form: StorybookRequestForm) {
  const trimmed = summary.trim();
  const intent = [form.title, form.theme, form.useScene, form.quickIdea]
    .map((value) => value.trim())
    .filter(Boolean);
  if (!trimmed) {
    return fallbackSummary(form);
  }
  if (!intent.length) return trimmed;
  const aligned = intent.some((value) => value.length >= 2 && trimmed.includes(value.slice(0, Math.min(value.length, 8))));
  return aligned ? trimmed : fallbackSummary(form);
}

function fallbackSummary(form: StorybookRequestForm) {
  if (form.quickIdea.trim()) return form.quickIdea.trim();
  const title = form.title || "这本绘本";
  const theme = form.theme || "当前教学目标";
  return `围绕《${title}》和「${theme}」展开，让孩子在熟悉场景里看见问题、练习方法，并带着安全感完成一次小小的成长。`;
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
