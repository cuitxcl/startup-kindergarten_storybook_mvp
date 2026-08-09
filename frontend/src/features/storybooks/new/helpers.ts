import type { StorybookPage, StorybookRole } from "../../../types/domain";
import type { EditablePage, EditablePlan, EditableRole } from "./types";

export function linesFromRows(value: string) {
  return value.split(/\n+/).map((item) => item.trim()).filter(Boolean);
}

export function planDraftFromOutput(output: unknown, form: { title: string; theme: string }): EditablePlan {
  const value = output as {
    plan?: {
      summary?: string;
      outline?: { page_range?: string; goal?: string; beat?: string }[];
      role_requirements?: string[];
      review_points?: string[];
    };
  } | undefined;
  const plan = value?.plan;
  return {
    summary: plan?.summary || `围绕「${form.theme || "教学目标"}」组织一个适合班级共读的短绘本，先呈现冲突，再由老师引导孩子练习规则。`,
    outlineText: plan?.outline?.length
      ? plan.outline.map((item) => `第 ${item.page_range || "?"} 页：${item.goal || "情节推进"} - ${item.beat || "待确认"}`).join("\n")
      : "第 1 页：进入熟悉场景，引出核心道具和规则需求\n第 2 页：孩子产生真实冲突或等待困难\n第 3 页：老师识别情绪并给出清楚办法\n第 4-5 页：孩子尝试规则，朋友给予回应\n第 6 页：规则被内化，故事温暖收束",
    roleRequirementsText: plan?.role_requirements?.join("\n") || "主角：有明确外观、性格和转变\n同伴：与主角产生互动\n老师：温柔稳定，引导规则\n关键道具：推动情节，便于画面识别",
    reviewPointsText: plan?.review_points?.join("\n") || "故事规则是否简单明确\n情绪是否安全、不恐吓\n画面是否适合幼儿园共读\n角色名称和形象能否跨页保持一致",
  };
}

export function generationInputFor(
  jobType: string,
  form: { title: string; theme: string; ageGroup: string; pageCount: string; useScene: string; style: string; storyStyle?: string; storyFramework?: string },
  plan: EditablePlan,
  roles: EditableRole[],
  pages: EditablePage[],
) {
  const base: Record<string, unknown> = {
    title: form.title,
    theme: form.theme,
    age_group: form.ageGroup,
    page_count: form.pageCount,
    use_scene: form.useScene,
    style: form.style,
  };
  if (form.storyStyle?.trim()) {
    base.story_style = form.storyStyle.trim();
  }
  if (form.storyFramework?.trim()) {
    base.story_framework = form.storyFramework.trim();
  }

  if (jobType === "storybook_plan") {
    return base;
  }
  if (jobType === "storybook_roles") {
    return {
      ...base,
      plan: planPayload(plan, form),
    };
  }
  if (jobType === "storybook_pages") {
    return {
      ...base,
      plan: planPayload(plan, form),
      confirmed_roles: roles.map((role) => rolePayload(role, form.style)),
      confirmed_pages: pages.length ? pages.map(pagePayload) : undefined,
    };
  }
  return base;
}

function planPayload(plan: EditablePlan, form: { title: string; theme: string }) {
  return {
    title: form.title,
    theme: form.theme,
    summary: plan.summary,
    outline: linesFromRows(plan.outlineText),
    role_requirements: linesFromRows(plan.roleRequirementsText),
    review_points: linesFromRows(plan.reviewPointsText),
  };
}

function roleReferenceStyle(style: string) {
  const trimmed = style.trim().replace(/。+$/, "");
  if (!trimmed) return "柔和水彩绘本风格，圆润饱满造型，大而富有表现力的眼睛";
  if (trimmed.includes("皮克斯") || trimmed.includes("3D")) {
    return `${trimmed}，高质量3D动画电影质感，立体圆润角色，柔和棚拍光，细腻材质，真实体积感`;
  }
  return trimmed;
}

function hasConflictingDefaultStyle(prompt: string, style: string) {
  const trimmedStyle = style.trim();
  if (!trimmedStyle) return false;
  return prompt.includes("柔和水彩绘本风格") && !trimmedStyle.includes("水彩");
}

function isLimbFreeRole(role: Pick<EditableRole, "name" | "roleType">, appearance: string) {
  const text = `${role.name} ${role.roleType} ${appearance}`;
  return ["无手", "没有手", "无脚", "没有脚", "无手和脚", "没有手和脚", "无四肢", "没有四肢", "蛇", "小蛇", "蚯蚓", "毛毛虫", "蜗牛", "球形"].some((keyword) =>
    text.includes(keyword),
  );
}

function roleReferenceAnatomyClause(role: Pick<EditableRole, "name" | "roleType">, appearance: string) {
  if (isLimbFreeRole(role, appearance)) {
    return "身体结构必须严格符合外观设定：没有手、没有脚、没有手臂和腿，不要生成手指、鞋子、胳膊或人形四肢，用头部、眼睛、身体弯曲、尾部和整体姿态表达动作";
  }
  return "身体结构必须严格符合外观设定；有手、脚、爪或翅膀时可以清晰表现，但不要凭空添加外观没有写到的肢体";
}

function roleReferencePrompt(role: EditableRole, style: string, appearance: string) {
  const existingPrompt = role.referenceImagePrompt.trim();
  if (existingPrompt && !hasConflictingDefaultStyle(existingPrompt, style)) {
    const styleText = roleReferenceStyle(style);
    return existingPrompt.includes(styleText) || existingPrompt.includes("画面风格")
      ? existingPrompt
      : `${existingPrompt}，画面风格必须与整本绘本一致：${styleText}`;
  }
  const styleText = roleReferenceStyle(style);
  const anatomy = roleReferenceAnatomyClause(role, appearance);
  return `${role.name}，${roleTypeLabel(role.roleType)}，${appearance || "绘本角色"}，画面风格必须与整本绘本一致：${styleText}，${anatomy}，表情自然生动、富有神采，姿态自然放松，单一角色标准图，白底或简洁背景，画面只有这个角色，无人类，无其他角色，保持跨页一致，不要僵硬对称的证件照式站姿`;
}

function rolePayload(role: EditableRole, style: string) {
  const appearance = cleanVisualAppearance(role.appearance);
  return {
    name: role.name,
    role_type: role.roleType,
    appearance,
    story_function: role.storyFunction,
    reference_image_prompt: role.needsConsistency ? roleReferencePrompt(role, style, appearance) : undefined,
    needs_consistency: role.needsConsistency,
  };
}

function pagePayload(page: EditablePage) {
  return {
    page_number: page.pageNumber,
    title: page.title,
    body: page.body,
    illustration_prompt: page.illustrationPrompt,
  };
}

export function roleFromStorybook(role: StorybookRole): EditableRole {
  return {
    id: role.id,
    name: role.name,
    roleType: role.roleType,
    appearance: cleanVisualAppearance(role.appearance),
    storyFunction: role.storyFunction,
    needsConsistency: role.needsConsistency,
    referenceImagePrompt: role.referenceImagePrompt || "",
    referenceImageUrl: role.referenceImageUrl,
    referenceStatus: role.referenceStatus,
  };
}

export function rolesFromStorybook(roles: StorybookRole[]) {
  return roles.map(roleFromStorybook);
}

export function rolesFromOutput(output: unknown): EditableRole[] {
  const value = output as { roles?: { name?: string; role_type?: string; appearance?: string; story_function?: string; reference_image_prompt?: string; needs_consistency?: boolean }[] } | undefined;
  if (!value?.roles?.length) return [];
  return value.roles.map((role, index) => normalizeEditableRole({
    name: role.name || `角色 ${index + 1}`,
    roleType: normalizeRoleType(role.role_type),
    appearance: role.appearance || "",
    storyFunction: role.story_function || "",
    referenceImagePrompt: role.reference_image_prompt || "",
    needsConsistency: role.needs_consistency ?? true,
  }));
}

export function roleDraftsFromPlan(plan: EditablePlan, form: { title: string; theme: string }): EditableRole[] {
  const requirements = linesFromRows(plan.roleRequirementsText);
  if (!requirements.length) {
    return [
      normalizeEditableRole({
        name: "主角",
        roleType: "protagonist",
        appearance: `围绕《${form.title || form.theme || "当前绘本"}》设计的主角，请补充物种或人物身份、颜色、服装或材质、体型轮廓、表情和跨页识别特征。不要写动作或剧情行为。`,
        storyFunction: `带领孩子进入「${form.theme || "当前主题"}」故事，并在情节中完成一次清楚的变化。`,
        needsConsistency: true,
        referenceImagePrompt: "",
      }),
      normalizeEditableRole({
        name: "引导者",
        roleType: "teacher",
        appearance: "温柔稳定的老师或智慧引导角色，请补充外观、服装、姿态和可重复识别的特征。",
        storyFunction: "在故事关键处帮助主角理解情绪、规则或解决办法。",
        needsConsistency: true,
        referenceImagePrompt: "",
      }),
    ];
  }

  return requirements.slice(0, 6).map((requirement, index) => {
    const [rawName, ...rest] = requirement.split(/[:：]/);
    const name = (rest.length ? rawName : `角色 ${index + 1}`).trim() || `角色 ${index + 1}`;
    const detail = (rest.length ? rest.join("：") : requirement).trim();
    return normalizeEditableRole({
      name,
      roleType: inferRoleType(name, detail, index),
      appearance: `根据第 2 步方案需求：${detail}。请补充颜色、服装或材质、体型轮廓、表情和一个可跨页重复识别的小特征。不要写动作或剧情行为。`,
      storyFunction: detail || `服务于「${form.theme || "当前主题"}」的故事推进。`,
      needsConsistency: true,
      referenceImagePrompt: "",
    });
  });
}

function normalizeEditableRole(role: EditableRole): EditableRole {
  const appearance = role.appearance.trim();
  const visualAppearance = cleanVisualAppearance(appearance);
  return {
    ...role,
    appearance: visualAppearance.length >= 28 ? visualAppearance : enrichShortAppearance(role.name, visualAppearance, role.roleType),
    storyFunction: role.storyFunction.trim() || "推动故事冲突和转变，帮助幼儿理解规则与情绪。",
    referenceImagePrompt: role.referenceImagePrompt.trim(),
  };
}

function normalizeRoleType(value?: string): StorybookRole["roleType"] {
  if (value === "protagonist" || value === "supporting" || value === "peer" || value === "teacher" || value === "prop") return value;
  return "supporting";
}

function inferRoleType(name: string, detail: string, index: number): StorybookRole["roleType"] {
  const text = `${name} ${detail}`;
  if (text.includes("道具") || text.includes("物品") || text.includes("指南针") || text.includes("星盘")) return "prop";
  if (text.includes("老师") || text.includes("妈妈") || text.includes("引导")) return "teacher";
  if (text.includes("同伴") || text.includes("朋友")) return "peer";
  if (text.includes("主角") || index === 0) return "protagonist";
  return "supporting";
}

function enrichShortAppearance(name: string, appearance: string, roleType: StorybookRole["roleType"]) {
  const base = appearance || (roleType === "prop" ? "关键道具" : "绘本角色");
  return `${cleanVisualAppearance(base)}；请补足颜色、服装或材质、体型轮廓、表情和一个可跨页重复识别的小特征，作为「${name}」的稳定参考设定。不要写动作或剧情行为。`;
}

export function pageFromStorybook(page: StorybookPage): EditablePage {
  return {
    id: page.id,
    pageNumber: page.pageNumber,
    title: page.title,
    body: page.body,
    illustrationPrompt: page.illustrationPrompt,
  };
}

export function pagesFromStorybook(pages: StorybookPage[]) {
  return pages.map(pageFromStorybook);
}

export function pagesFromOutput(output: unknown): EditablePage[] {
  const value = output as { pages?: { page_number?: number; title?: string; body?: string; illustration_prompt?: string }[] } | undefined;
  if (!value?.pages?.length) return [];
  return value.pages.map((page, index) => ({
    pageNumber: page.page_number || index + 1,
    title: page.title || `第 ${index + 1} 页`,
    body: page.body || "",
    illustrationPrompt: page.illustration_prompt || "",
  }));
}

export function pageDraftsFromPlan(plan: EditablePlan, form: { title: string; theme: string }): EditablePage[] {
  const outline = linesFromRows(plan.outlineText);
  if (!outline.length) {
    return [{
      pageNumber: 1,
      title: form.title || "第 1 页",
      body: `围绕「${form.theme || "当前主题"}」展开第一段故事。`,
      illustrationPrompt: `画面需要体现《${form.title || "当前绘本"}》的核心场景和主角。`,
    }];
  }
  return outline.slice(0, 8).map((item, index) => ({
    pageNumber: index + 1,
    title: item.replace(/^第\s*[^：:]+[：:]\s*/, "").split(/[-。]/)[0]?.trim() || `第 ${index + 1} 页`,
    body: item,
    illustrationPrompt: `根据本页内容绘制，必须延续第 3 步确认的角色名称和外观：${item}`,
  }));
}

export function roleTypeLabel(roleType: StorybookRole["roleType"]) {
  const labels: Record<StorybookRole["roleType"], string> = {
    protagonist: "主角",
    supporting: "配角",
    peer: "同伴角色",
    teacher: "老师形象",
    prop: "关键道具",
  };
  return labels[roleType] || "角色";
}

export function cleanVisualAppearance(value: string) {
  const behaviorKeywords = [
    "喜欢",
    "总喜欢",
    "经常",
    "常常",
    "总是",
    "常和",
    "离开队伍",
    "交流",
    "适合",
    "带领",
    "制定",
    "强调",
    "学习",
    "代表",
    "推动",
    "帮助",
    "引导",
    "鼓励",
    "提醒",
    "跑",
    "跳",
    "蹦",
    "玩",
    "等待",
    "分享",
  ];
  const parts = value
    .split(/[，,。；;、]/)
    .map((part) => part.trim())
    .filter(Boolean)
    .filter((part) => !behaviorKeywords.some((keyword) => part.includes(keyword)));
  return parts.join("，") || value.trim();
}
