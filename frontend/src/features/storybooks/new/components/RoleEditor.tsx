import { useEffect, useState } from "react";
import { Badge } from "../../../../components/ui";
import type { StorybookRole } from "../../../../types/domain";
import { roleTypeLabel } from "../helpers";
import type { EditableRole } from "../types";
import { ReferenceImagePicker } from "./ReferenceImagePicker";

export function RoleEditor({
  workspaceId,
  storybookId,
  roles,
  onChange,
  onGenerateReference,
  onRolesRefresh,
  roleReferenceBusyId,
  variantRefreshKey,
}: {
  workspaceId: string;
  storybookId?: string;
  roles: EditableRole[];
  onChange: (roles: EditableRole[]) => void;
  onGenerateReference?: (role: EditableRole, roleIndex: number) => Promise<void>;
  onRolesRefresh?: (roles: EditableRole[]) => void;
  roleReferenceBusyId?: string | null;
  variantRefreshKey?: number;
}) {
  const sortedRoleEntries = sortRoleEntriesByImportance(roles);
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const selectedEntry = sortedRoleEntries.find((entry) => entry.index === selectedIndex) || sortedRoleEntries[0];
  const selectedRole = selectedEntry?.role;
  const activeIndex = selectedEntry?.index ?? 0;
  const selectedRoleBusyKey = selectedRole ? roleReferenceBusyKey(selectedRole, activeIndex) : "";
  const referenceIsGenerating = Boolean(selectedRole?.needsConsistency && (
    selectedRole.referenceStatus === "generating"
    || roleReferenceBusyId === selectedRole?.id
    || roleReferenceBusyId === selectedRoleBusyKey
  ));
  const update = (index: number, patch: Partial<EditableRole>) => {
    onChange(roles.map((role, roleIndex) => roleIndex === index ? { ...role, ...patch } : role));
  };
  useEffect(() => {
    if (selectedIndex === -1 && sortedRoleEntries.length) {
      setSelectedIndex(sortedRoleEntries[0].index);
    }
  }, [selectedIndex, sortedRoleEntries]);
  if (!selectedRole) {
    return (
      <div className="review-editor role-editor-empty">
        <p>还没有角色内容，请先重新生成角色与道具。</p>
      </div>
    );
  }
  return (
    <div className="review-editor role-editor">
      <aside className="role-editor-list" aria-label="角色列表">
        <div className="role-editor-list-head">
          <strong>角色与关键道具</strong>
          <span>{roles.length} 个</span>
        </div>
        {sortedRoleEntries.map(({ role, index }) => (
          <button
            key={`${role.id || role.name}-${index}`}
            type="button"
            className={`role-editor-item ${index === activeIndex ? "active" : ""}`}
            onClick={() => setSelectedIndex(index)}
          >
            <span className="role-editor-item-main">
              <strong>{role.name || "未命名角色"}</strong>
              <small>{roleTypeLabel(role.roleType)} · {role.needsConsistency ? "跨页保持一致" : "单页或可变化"}</small>
            </span>
          </button>
        ))}
      </aside>
      <section className="role-editor-detail">
        <div className="role-editor-detail-head">
          <div>
            <p className="eyebrow">当前编辑</p>
            <h3>{selectedRole.name || "未命名角色"}</h3>
          </div>
          <Badge tone={selectedRole.needsConsistency ? "info" : "neutral"}>{selectedRole.needsConsistency ? "跨页一致" : "可变化"}</Badge>
        </div>
        <div className="role-editor-basic">
          <label>
            名称
            <input value={selectedRole.name} onChange={(event) => update(activeIndex, { name: event.target.value })} />
          </label>
          <label>
            类型
            <select value={selectedRole.roleType} onChange={(event) => update(activeIndex, { roleType: event.target.value as StorybookRole["roleType"] })}>
              <option value="protagonist">主角</option>
              <option value="supporting">配角</option>
              <option value="peer">同伴角色</option>
              <option value="teacher">老师形象</option>
              <option value="prop">关键道具</option>
            </select>
          </label>
        </div>
        <label className="role-editor-field">
          <span>稳定外观</span>
          <small>只写长相、颜色、服饰、固定识别特征，不写剧情动作。</small>
          <textarea rows={4} value={selectedRole.appearance} onChange={(event) => update(activeIndex, { appearance: event.target.value })} />
        </label>
        <label className="role-editor-field">
          <span>故事作用</span>
          <small>说明它为什么出现在故事里，帮助老师判断是否符合教学目标。</small>
          <textarea rows={3} value={selectedRole.storyFunction} onChange={(event) => update(activeIndex, { storyFunction: event.target.value })} />
        </label>
        <label className="role-consistency-toggle">
          <input type="checkbox" checked={selectedRole.needsConsistency} onChange={(event) => update(activeIndex, { needsConsistency: event.target.checked })} />
          <span>
            <strong>后续分页插图保持同一形象</strong>
            <small>主角、老师、反复出现的同伴建议开启；只出现一次的道具或群体可以关闭。</small>
          </span>
        </label>
        <section className="role-reference-step-card">
          <div className="role-reference-step-head">
            <div>
              <strong>角色参考图</strong>
              <span>用于后续分页插图保持同一形象，应该在生成分页前先确认。</span>
            </div>
            <Badge tone={selectedRole.needsConsistency ? referenceStatusTone(selectedRole.referenceStatus) : "neutral"}>
              {selectedRole.needsConsistency ? roleReferenceStatusLabel(selectedRole.referenceStatus) : "无需参考图"}
            </Badge>
          </div>
          {selectedRole.needsConsistency ? (
            <ReferenceImagePicker
              workspaceId={workspaceId}
              storybookId={storybookId}
              role={selectedRole}
              roleIndex={activeIndex}
              referenceIsGenerating={referenceIsGenerating}
              variantRefreshKey={variantRefreshKey}
              onGenerateReference={onGenerateReference}
              onRolesRefresh={onRolesRefresh}
            />
          ) : (
            <div className="reference-prompt-preview muted">
              <div>
                <strong>当前不生成参考图</strong>
                <span>这个角色或道具被设为“单页或可变化”，分页插图会直接按每页插图描述绘制。</span>
              </div>
            </div>
          )}
        </section>
      </section>
    </div>
  );
}

function roleReferenceStatusLabel(status?: string) {
  return {
    not_started: "未生成",
    generating: "生成中",
    ready: "已确认",
    needs_regeneration: "需要重绘",
    failed: "生成失败",
  }[status || "not_started"] || "待确认";
}

function referenceStatusTone(status?: string): "neutral" | "good" | "warn" | "danger" | "info" {
  if (status === "ready") return "good";
  if (status === "failed") return "danger";
  if (status === "generating") return "info";
  if (status === "needs_regeneration") return "warn";
  return "neutral";
}

function sortRoleEntriesByImportance(roles: EditableRole[]) {
  const priority: Record<StorybookRole["roleType"], number> = {
    protagonist: 0,
    teacher: 1,
    peer: 2,
    supporting: 3,
    prop: 4,
  };
  return roles
    .map((role, index) => ({ role, index }))
    .sort((left, right) => {
      const leftPriority = priority[left.role.roleType] ?? 9;
      const rightPriority = priority[right.role.roleType] ?? 9;
      if (leftPriority !== rightPriority) return leftPriority - rightPriority;
      if (left.role.needsConsistency !== right.role.needsConsistency) {
        return left.role.needsConsistency ? -1 : 1;
      }
      return left.index - right.index;
    });
}

function roleReferenceBusyKey(role: EditableRole, index: number) {
  return role.id || `${index}:${role.roleType}:${role.name || "未命名角色"}`;
}
