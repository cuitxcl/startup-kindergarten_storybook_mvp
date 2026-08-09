import type { FormEvent } from "react";
import { Modal } from "../../../../components/ui";

export type StorybookMetaForm = {
  title: string;
  ageGroup: string;
  useScene: string;
  teachingGoal: string;
  coverTone: string;
};

export function DeleteStorybookModal({
  title,
  deleting,
  onClose,
  onConfirm,
}: {
  title: string;
  deleting: boolean;
  onClose: () => void;
  onConfirm: () => void;
}) {
  return (
    <Modal title={`删除《${title}》？`} onClose={onClose}>
      <div className="form-stack">
        <p>删除后不可恢复：这本绘本的分页、角色、生成记录、分享链接和导出记录会一并移除。</p>
        <div className="modal-actions">
          <button className="button secondary" type="button" disabled={deleting} onClick={onClose}>取消</button>
          <button className="button danger" type="button" disabled={deleting} onClick={onConfirm}>{deleting ? "删除中..." : "确认删除"}</button>
        </div>
      </div>
    </Modal>
  );
}

export function EditStorybookMetaModal({
  form,
  saving,
  onClose,
  onChange,
  onSubmit,
}: {
  form: StorybookMetaForm;
  saving: boolean;
  onClose: () => void;
  onChange: (form: StorybookMetaForm) => void;
  onSubmit: (event: FormEvent) => void;
}) {
  return (
    <Modal title="编辑绘本信息" onClose={onClose}>
      <form onSubmit={onSubmit}>
        <label>绘本标题<input value={form.title} onChange={(event) => onChange({ ...form, title: event.target.value })} /></label>
        <label>
          年龄段
          <select value={form.ageGroup} onChange={(event) => onChange({ ...form, ageGroup: event.target.value })}>
            <option>3-4 岁</option>
            <option>4-5 岁</option>
            <option>5-6 岁</option>
          </select>
        </label>
        <label>使用场景<input value={form.useScene} onChange={(event) => onChange({ ...form, useScene: event.target.value })} /></label>
        <label>教学目标<textarea rows={3} value={form.teachingGoal} onChange={(event) => onChange({ ...form, teachingGoal: event.target.value })} /></label>
        <label>封面风格<input value={form.coverTone} onChange={(event) => onChange({ ...form, coverTone: event.target.value })} /></label>
        <div className="modal-actions">
          <button className="button secondary" type="button" onClick={onClose}>取消</button>
          <button className="button primary" type="submit" disabled={saving}>{saving ? "保存中" : "保存信息"}</button>
        </div>
      </form>
    </Modal>
  );
}

export function DuplicateStorybookModal({
  title,
  duplicating,
  onClose,
  onTitleChange,
  onSubmit,
}: {
  title: string;
  duplicating: boolean;
  onClose: () => void;
  onTitleChange: (title: string) => void;
  onSubmit: () => void;
}) {
  return (
    <Modal title="复制为新绘本" onClose={onClose}>
      <form onSubmit={(event) => { event.preventDefault(); onSubmit(); }}>
        <label>副本名称<input value={title} onChange={(event) => onTitleChange(event.target.value)} /></label>
        <p className="task-summary">系统会复制分页正文、插图描述、角色设定和参考图，创建为新的私有草稿，不会覆盖当前绘本。</p>
        <div className="modal-actions">
          <button className="button secondary" type="button" onClick={onClose}>取消</button>
          <button className="button primary" type="submit" disabled={duplicating}>{duplicating ? "复制中..." : "确认复制"}</button>
        </div>
      </form>
    </Modal>
  );
}
