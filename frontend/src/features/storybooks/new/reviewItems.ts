import type { StorybookRole } from "../../../types/domain";
import { linesFromRows } from "./helpers";
import type { EditablePage, EditablePlan, EditableRole } from "./types";

export function storybookPlanItems(output: unknown, form: { title: string; theme: string }, draft?: EditablePlan) {
  const value = output as {
    plan?: {
      title?: string;
      theme?: string;
      summary?: string;
      outline?: { page_range?: string; goal?: string; beat?: string }[];
      role_requirements?: string[];
      review_points?: string[];
    };
  } | undefined;
  const plan = value?.plan;
  if (draft && (draft.summary || draft.outlineText || draft.roleRequirementsText || draft.reviewPointsText)) {
    return [
      `标题：《${form.title || "未命名绘本"}》`,
      `目标：${form.theme || "待确认"}`,
      draft.summary ? `故事概述：${draft.summary}` : null,
      ...linesFromRows(draft.outlineText).map((line) => `分页节奏：${line}`),
      draft.roleRequirementsText ? `角色需求：${linesFromRows(draft.roleRequirementsText).join("、")}` : null,
      draft.reviewPointsText ? `确认重点：${linesFromRows(draft.reviewPointsText).join("、")}` : null,
    ].filter(Boolean) as string[];
  }
  if (!plan) {
    return [
      `标题：《${form.title || "一起玩小汽车"}》`,
      `目标：${form.theme || "学习轮流、等待和表达感受"}`,
      "结构：带来玩具 -> 朋友想玩 -> 老师引导 -> 沙漏轮流 -> 开心整理",
      "角色需求：主角、朋友、老师、关键道具",
    ];
  }

  return [
    `标题：《${plan.title || form.title || "未命名绘本"}》`,
    `目标：${plan.theme || form.theme || "待确认"}`,
    plan.summary ? `故事概述：${plan.summary}` : null,
    ...(plan.outline || []).map((item) => `第 ${item.page_range || "?"} 页：${item.goal || "情节"} - ${item.beat || "待确认"}`),
    plan.role_requirements?.length ? `角色需求：${plan.role_requirements.join("、")}` : null,
    plan.review_points?.length ? `确认重点：${plan.review_points.join("、")}` : null,
  ].filter(Boolean) as string[];
}

export function storybookRoleItems(output: unknown, editableRoles: EditableRole[] = [], plan?: EditablePlan, form?: { title: string; theme: string }) {
  const value = output as {
    roles?: { name?: string; role_type?: string; appearance?: string; story_function?: string }[];
    consistency_guide?: string[];
  } | undefined;
  if (editableRoles.length) {
    const sortedRoles = sortRolesByImportance(editableRoles);
    return [
      ...sortedRoles.map((role) => `${role.name}：${role.appearance}；故事作用：${role.storyFunction}；参考图提示：${role.referenceImagePrompt || "沿用外观设定生成稳定参考图"}`),
      "一致性要求：分页正文和插图描述必须使用上面确认的角色名称，不新增替代动物或泛称角色。",
    ];
  }
  if (!value?.roles?.length) {
    const requirements = linesFromRows(plan?.roleRequirementsText || "");
    return [
      `待生成：将根据第 2 步《${form?.title || "当前绘本"}》方案生成角色，不会使用固定示例角色。`,
      requirements.length ? `来自方案的角色需求：${requirements.join("、")}` : `角色会围绕「${form?.theme || "当前主题"}」和故事方案生成。`,
      "操作：点击“生成角色道具”后，再审核或手动修改具体名称、稳定外观和故事作用。",
    ];
  }

  return [
    ...value.roles.map((role) => `${role.name || "未命名角色"}：${role.appearance || "外观待确认"}；故事作用：${role.story_function || role.role_type || "参与故事推进"}`),
    value.consistency_guide?.length ? `一致性要求：${value.consistency_guide.join("、")}` : null,
  ].filter(Boolean) as string[];
}

export function storybookPageItems(output: unknown, editablePages: EditablePage[] = [], plan?: EditablePlan, form?: { title: string; theme: string }) {
  const value = output as {
    pages?: { page_number?: number; title?: string; body?: string; illustration_prompt?: string }[];
    editor_notes?: string[];
  } | undefined;
  if (editablePages.length) {
    return editablePages.map((page) => `第 ${page.pageNumber} 页：${page.title} - ${page.body}；插图：${page.illustrationPrompt}`);
  }
  if (!value?.pages?.length) {
    const outline = linesFromRows(plan?.outlineText || "");
    return [
      `待生成：将根据《${form?.title || "当前绘本"}》方案和第 3 步确认角色生成分页。`,
      outline.length ? `来自方案的分页节奏：${outline.join("、")}` : `分页会围绕「${form?.theme || "当前主题"}」展开。`,
      "操作：点击“生成分页图文”后，再逐页审核或手动修改标题、正文和插图描述。",
    ];
  }

  return [
    ...value.pages.map((page) => `第 ${page.page_number || "?"} 页：${page.title || "未命名分页"} - ${page.body || page.illustration_prompt || "待确认"}`),
    value.editor_notes?.length ? `编辑提示：${value.editor_notes.join("、")}` : null,
  ].filter(Boolean) as string[];
}

function sortRolesByImportance(roles: EditableRole[]) {
  return sortRoleEntriesByImportance(roles).map((entry) => entry.role);
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
