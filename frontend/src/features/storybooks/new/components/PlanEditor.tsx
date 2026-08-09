import type { EditablePlan } from "../types";

type PlanEditorForm = {
  title: string;
  theme: string;
  ageGroup: string;
  pageCount: string;
  useScene: string;
  style: string;
  storyStyle: string;
  storyFramework: string;
};

export function PlanEditor({
  form,
  plan,
  onFormChange,
  onPlanChange,
}: {
  form: PlanEditorForm;
  plan: EditablePlan;
  onFormChange: (value: PlanEditorForm) => void;
  onPlanChange: (plan: EditablePlan) => void;
}) {
  return (
    <div className="review-editor">
      <label>绘本标题<input value={form.title} onChange={(event) => onFormChange({ ...form, title: event.target.value })} /></label>
      <label>教学目标<input value={form.theme} onChange={(event) => onFormChange({ ...form, theme: event.target.value })} /></label>
      <label className="span-2">故事概述<textarea rows={4} value={plan.summary} onChange={(event) => onPlanChange({ ...plan, summary: event.target.value })} /></label>
      <label className="span-2">分页节奏（每页一条，可逐页修改）<OutlineLinesEditor text={plan.outlineText} onChange={(outlineText) => onPlanChange({ ...plan, outlineText })} /></label>
      <label className="span-2">角色需求<textarea rows={4} value={plan.roleRequirementsText} onChange={(event) => onPlanChange({ ...plan, roleRequirementsText: event.target.value })} /></label>
      <label className="span-2">老师确认重点<textarea rows={4} value={plan.reviewPointsText} onChange={(event) => onPlanChange({ ...plan, reviewPointsText: event.target.value })} /></label>
    </div>
  );
}

/** 分页节奏逐页编辑：按行拆分，每页一个独立输入框，写回时按行合并。
 *  页数由生成结果固定：输入框内禁止换行（回车忽略、粘贴内容去掉换行），避免多出空白页。 */
function OutlineLinesEditor({ text, onChange }: { text: string; onChange: (text: string) => void }) {
  const lines = text.split("\n");
  return (
    <div className="outline-lines">
      {lines.map((line, index) => (
        <textarea
          key={index}
          rows={2}
          value={line}
          placeholder={`第 ${index + 1} 页`}
          onKeyDown={(event) => {
            if (event.key === "Enter") event.preventDefault();
          }}
          onChange={(event) => {
            const next = [...lines];
            next[index] = event.target.value.replace(/\r?\n+/g, " ");
            onChange(next.join("\n"));
          }}
        />
      ))}
    </div>
  );
}
