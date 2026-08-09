import { useState } from "react";
import { Badge } from "../../../../components/ui";
import type { EditablePage, EditableRole } from "../types";

export function PageEditor({ pages, roles, onChange }: { pages: EditablePage[]; roles: EditableRole[]; onChange: (pages: EditablePage[]) => void }) {
  const [selectedIndex, setSelectedIndex] = useState(0);
  const activeIndex = Math.min(selectedIndex, Math.max(0, pages.length - 1));
  const activePage = pages[activeIndex];
  const update = (index: number, patch: Partial<EditablePage>) => {
    onChange(pages.map((page, pageIndex) => pageIndex === index ? { ...page, ...patch } : page));
  };
  const roleNames = roles.filter((role) => role.needsConsistency).map((role) => role.name).filter(Boolean);
  if (!pages.length) {
    return <div className="review-editor page-editor"><span className="role-editor-empty">还没有可编辑的分页内容。</span></div>;
  }
  return (
    <div className="review-editor page-editor">
      {roleNames.length > 0 && (
        <div className="reference-guard-callout">
          <Badge tone="info">一致性检查</Badge>
          <div>
            <strong>分页应继续使用第 3 步确认的角色</strong>
            <span>已确认角色：{roleNames.join("、")}。如果正文或插图中需要这些角色，请直接写角色名称，不要改成“朋友”“老师”这种泛称。</span>
          </div>
        </div>
      )}
      <div className="page-editor-main">
        <aside className="role-editor-list page-editor-list" aria-label="分页列表">
          <div className="role-editor-list-head">
            <strong>分页</strong>
            <span>{pages.length} 页</span>
          </div>
          {pages.map((page, index) => (
            <button
              key={`${page.id || page.pageNumber}-${index}`}
              type="button"
              className={`role-editor-item page-editor-item ${index === activeIndex ? "active" : ""}`}
              onClick={() => setSelectedIndex(index)}
            >
              <span className="role-editor-item-main">
                <strong>{page.title || `第 ${page.pageNumber} 页`}</strong>
                <small>第 {page.pageNumber} 页 · {page.body.trim() ? "正文已生成" : "缺少正文"} · {page.illustrationPrompt.trim() ? "插图描述已生成" : "缺少插图描述"}</small>
              </span>
            </button>
          ))}
        </aside>
        <div className="role-editor-detail page-editor-detail">
          <div className="role-editor-detail-head">
            <div>
              <p className="eyebrow">当前编辑</p>
              <h3>{activePage.title || `第 ${activePage.pageNumber} 页`}</h3>
            </div>
            <Badge tone={activePage.body.trim() && activePage.illustrationPrompt.trim() ? "good" : "warn"}>
              第 {activePage.pageNumber} 页
            </Badge>
          </div>
          <div className="role-editor-basic page-editor-fields">
            <label className="role-editor-field">
              <span>页面标题</span>
              <input value={activePage.title} onChange={(event) => update(activeIndex, { title: event.target.value })} />
            </label>
            <label className="role-editor-field span-2">
              <span>正文</span>
              <textarea rows={5} value={activePage.body} onChange={(event) => update(activeIndex, { body: event.target.value })} />
            </label>
            <label className="role-editor-field span-2">
              <span>插图描述</span>
              <small>只写这一页画面需要看到的场景、角色、动作和构图。</small>
              <textarea rows={6} value={activePage.illustrationPrompt} onChange={(event) => update(activeIndex, { illustrationPrompt: event.target.value })} />
            </label>
          </div>
        </div>
      </div>
    </div>
  );
}
