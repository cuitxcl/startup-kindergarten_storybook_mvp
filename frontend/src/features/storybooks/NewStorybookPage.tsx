import { type ReactNode, useEffect, useState } from "react";
import { Link, useNavigate, useOutletContext } from "react-router-dom";
import {
  createGenerationJob,
  createStorybook,
  getGenerationJob,
  getStorybook,
  getWorkspaceGenerationProvider,
  retryGenerationJob,
  shouldUseApi,
  updateStorybook,
  updateStorybookPage,
  updateStorybookRole,
  type GenerationJob,
  type GenerationProviderStatus,
} from "../../api/client";
import { Badge, Card, Notice, PageHeader, WizardSideNav } from "../../components/ui";
import { storybooks } from "../../data/mock";
import type { Storybook, StorybookPage, StorybookRole, Workspace } from "../../types/domain";
import {
  generationJobStatusLabel,
  generationJobTypeLabel,
  generationPrivacyAuditSummary,
} from "../../utils/labels";

const steps = ["需求", "绘本方案", "角色道具", "分页编辑", "预览导出"];

type EditablePlan = {
  summary: string;
  outlineText: string;
  roleRequirementsText: string;
  reviewPointsText: string;
};

type EditableRole = {
  id?: string;
  name: string;
  roleType: StorybookRole["roleType"];
  appearance: string;
  storyFunction: string;
  needsConsistency: boolean;
  referenceImagePrompt: string;
};

type EditablePage = {
  id?: string;
  pageNumber: number;
  title: string;
  body: string;
  illustrationPrompt: string;
};

export function NewStorybookPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const navigate = useNavigate();
  const [step, setStep] = useState(0);
  const [unlockedStep, setUnlockedStep] = useState(0);
  const [notice, setNotice] = useState<{ title: string; copy: string } | null>(null);
  const [creating, setCreating] = useState(false);
  const [generatingStep, setGeneratingStep] = useState<string | null>(null);
  const [createdBookId, setCreatedBookId] = useState<string | null>(null);
  const [retryJob, setRetryJob] = useState<GenerationJob | null>(null);
  const [generationOutputs, setGenerationOutputs] = useState<Record<string, unknown>>({});
  const [provider, setProvider] = useState<GenerationProviderStatus | null>(null);
  const [editingReview, setEditingReview] = useState<null | "plan" | "roles" | "pages">(null);
  const [planDraft, setPlanDraft] = useState<EditablePlan>({
    summary: "",
    outlineText: "",
    roleRequirementsText: "",
    reviewPointsText: "",
  });
  const [editableRoles, setEditableRoles] = useState<EditableRole[]>([]);
  const [editablePages, setEditablePages] = useState<EditablePage[]>([]);
  const [form, setForm] = useState({
    title: "一起玩小汽车",
    theme: "学会分享和轮流",
    ageGroup: "4-5 岁",
    pageCount: "6",
    useScene: "规则引导",
    style: "温暖、生活化，有清晰的老师引导。",
  });
  const targetBook = shouldUseApi ? createdBookId : storybooks.find((item) => item.workspaceId === workspace.id)?.id || "storybook-1";
  const hasPlan = Boolean(generationOutputs.storybook_plan || planDraft.summary || planDraft.outlineText);
  const hasRoles = editableRoles.length > 0;
  const hasPages = editablePages.length > 0;
  const primaryLabels = [
    "生成绘本方案",
    hasPlan ? "确认方案，继续角色" : "生成绘本方案",
    hasRoles ? "确认角色，生成分页" : "生成角色道具",
    hasPages ? "确认分页，进入预览" : "生成分页图文",
    "已完成",
  ];
  const showNotice = (title: string, copy: string) => {
    setRetryJob(null);
    setNotice({ title, copy });
  };
  const goToStep = (nextStep: number) => {
    setUnlockedStep((value) => Math.max(value, nextStep));
    setStep(nextStep);
  };
  const updateRequestForm = (patch: Partial<typeof form>) => {
    setForm((current) => ({ ...current, ...patch }));
    setGenerationOutputs({});
    setPlanDraft({ summary: "", outlineText: "", roleRequirementsText: "", reviewPointsText: "" });
    setEditableRoles([]);
    setEditablePages([]);
    setCreatedBookId(null);
    setUnlockedStep(0);
    setNotice(null);
    setRetryJob(null);
  };

  useEffect(() => {
    if (!shouldUseApi) return;
    getWorkspaceGenerationProvider(workspace.id).then(setProvider).catch(() => setProvider(null));
  }, [workspace.id]);
  const ensureStorybookCreated = async () => {
    if (!shouldUseApi || createdBookId) return createdBookId;
    setCreating(true);
    try {
      const book = await createStorybook(workspace.id, {
        title: form.title.trim() || form.theme.trim() || "新建普通绘本",
        ageGroup: form.ageGroup,
        useScene: form.useScene,
        teachingGoal: form.theme.trim() || "帮助孩子理解班级规则和生活习惯",
      });
      setCreatedBookId(book.id);
      return book.id;
    } finally {
      setCreating(false);
    }
  };
  const runGeneration = async (jobType: string, title: string) => {
    if (!shouldUseApi) {
      showNotice(title, "当前为本地原型反馈；接入 API 后会创建生成任务。");
      return true;
    }
    setGeneratingStep(jobType);
    setRetryJob(null);
    setNotice(null);
    try {
      const bookId = jobType === "storybook_roles" || jobType === "storybook_pages"
        ? await ensureStorybookCreated()
        : createdBookId;
      const job = await createGenerationJob(workspace.id, {
        jobType,
        storybookId: bookId || undefined,
        input: generationInputFor(jobType, form, planDraft, editableRoles, editablePages),
      });
      const settledJob = await waitForGenerationJob(job);
      return await handleGenerationJob(settledJob, title);
    } catch (err) {
      setRetryJob(null);
      setNotice({ title: "生成失败", copy: err instanceof Error ? err.message : "请稍后重试" });
      return false;
    } finally {
      setGeneratingStep(null);
    }
  };

  const waitForGenerationJob = async (initialJob: GenerationJob) => {
    let currentJob = initialJob;
    for (let attempt = 0; attempt < 20 && ["queued", "running"].includes(currentJob.status); attempt += 1) {
      await new Promise((resolve) => window.setTimeout(resolve, 800));
      currentJob = await getGenerationJob(workspace.id, currentJob.id);
    }
    return currentJob;
  };
  const retryFailedGeneration = async () => {
    if (!retryJob) return;
    setGeneratingStep(retryJob.jobType);
    setNotice(null);
    try {
      const job = await retryGenerationJob(workspace.id, retryJob.id);
      const settledJob = await waitForGenerationJob(job);
      await handleGenerationJob(settledJob, "已重新生成");
    } catch (err) {
      setNotice({ title: "重试失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setGeneratingStep(null);
    }
  };
  const handleGenerationJob = async (job: GenerationJob, title: string) => {
    if (job.status === "failed") {
      setRetryJob(job);
      setNotice({
        title: "生成失败",
        copy: `${generationErrorMessage(job)}。任务编号：${job.id.slice(0, 8)}。`,
      });
      return false;
    }
    if (["queued", "running"].includes(job.status)) {
      setRetryJob(null);
      setNotice({
        title: "生成任务仍在处理",
        copy: `当前状态：${generationStatusLabel(job.status)}。任务编号：${job.id.slice(0, 8)}，稍后可重新点击继续。`,
      });
      return false;
    }
    setRetryJob(null);
    if (job.output) {
      setGenerationOutputs((outputs) => ({ ...outputs, [job.jobType]: job.output }));
      if (job.jobType === "storybook_plan") {
        setPlanDraft(planDraftFromOutput(job.output, form));
      }
      if (job.jobType === "storybook_roles") {
        const roles = shouldUseApi && job.storybookId
          ? rolesFromStorybook((await getStorybook(workspace.id, job.storybookId)).roles)
          : rolesFromOutput(job.output);
        setEditableRoles(roles);
      }
      if (job.jobType === "storybook_pages") {
        const pages = shouldUseApi && job.storybookId
          ? pagesFromStorybook((await getStorybook(workspace.id, job.storybookId)).pages)
          : pagesFromOutput(job.output);
        setEditablePages(pages);
      }
    }
    setNotice({ title, copy: `生成任务${generationStatusLabel(job.status)}，任务编号：${job.id.slice(0, 8)}。` });
    return true;
  };
  const persistRoles = async (bookId: string) => {
    if (!shouldUseApi || !editableRoles.length) return;
    const book = await getStorybook(workspace.id, bookId);
    const updated = await Promise.all(editableRoles.map(async (role, index) => {
      const existing = role.id
        ? book.roles.find((item) => item.id === role.id)
        : book.roles.find((item) => item.name === role.name) || book.roles[index];
      if (!existing) return role;
      const saved = await updateStorybookRole(workspace.id, bookId, existing.id, {
        name: role.name,
        roleType: role.roleType,
        appearance: role.appearance,
        storyFunction: role.storyFunction,
        needsConsistency: role.needsConsistency,
        referenceImagePrompt: role.referenceImagePrompt,
      });
      return roleFromStorybook(saved);
    }));
    setEditableRoles(updated);
  };
  const persistPages = async (bookId: string) => {
    if (!shouldUseApi || !editablePages.length) return;
    const book = await getStorybook(workspace.id, bookId);
    const updated = await Promise.all(editablePages.map(async (page, index) => {
      const existing = page.id
        ? book.pages.find((item) => item.id === page.id)
        : book.pages.find((item) => item.pageNumber === page.pageNumber) || book.pages[index];
      if (!existing) return page;
      const saved = await updateStorybookPage(workspace.id, bookId, existing.id, {
        title: page.title,
        body: page.body,
        illustrationPrompt: page.illustrationPrompt,
      });
      return pageFromStorybook(saved);
    }));
    setEditablePages(updated);
  };
  const persistStorybookMeta = async (bookId: string) => {
    if (!shouldUseApi) return;
    await updateStorybook(workspace.id, bookId, {
      title: form.title.trim() || form.theme.trim() || "新建普通绘本",
      ageGroup: form.ageGroup,
      useScene: form.useScene,
      teachingGoal: form.theme.trim() || "帮助孩子理解班级规则和生活习惯",
    });
  };
  const handlePrimary = async () => {
    setNotice(null);
    if (step === 0) {
      if (await runGeneration("storybook_plan", "绘本方案已生成")) {
        goToStep(1);
      }
      return;
    }
    if (shouldUseApi && step === 1 && !createdBookId) {
      try {
        const bookId = await ensureStorybookCreated();
        if (bookId) await persistStorybookMeta(bookId);
        setNotice({ title: "普通绘本已创建", copy: "后续角色和分页生成会直接写入这本绘本，进入详情后可继续编辑、导出或派生定制版本。" });
      } catch (err) {
        setNotice({ title: "创建失败", copy: err instanceof Error ? err.message : "请稍后重试" });
        return;
      }
    } else if (shouldUseApi && step === 1 && createdBookId) {
      await persistStorybookMeta(createdBookId);
    }
    if (step === 2) {
      const bookId = await ensureStorybookCreated();
      if (!hasRoles) {
        await runGeneration("storybook_roles", "角色与道具已生成并写入绘本");
        setEditingReview("roles");
        return;
      }
      if (bookId) {
        await persistRoles(bookId);
      }
      if (await runGeneration("storybook_pages", "分页图文已生成并写入绘本")) {
        if (shouldUseApi && bookId) {
          await updateStorybook(workspace.id, bookId, { status: "roles_pending" });
        }
        goToStep(3);
        setEditingReview("pages");
      }
      return;
    }
    if (step === 3) {
      if (!hasPages) {
        if (await runGeneration("storybook_pages", "分页图文已生成并写入绘本")) {
          setEditingReview("pages");
        }
        return;
      }
      try {
        const bookId = shouldUseApi ? createdBookId : targetBook;
        if (shouldUseApi && bookId) {
          await persistPages(bookId);
          await updateStorybook(workspace.id, bookId, { status: "editing" });
          await updateStorybook(workspace.id, bookId, { status: "exportable" });
        }
        goToStep(4);
        if (bookId) {
          navigate(`/app/${workspace.id}/storybooks/${bookId}?result=plain`);
        }
      } catch (err) {
        setNotice({ title: "保存分页失败", copy: err instanceof Error ? err.message : "请稍后重试" });
      }
      return;
    }
    goToStep(Math.min(steps.length - 1, step + 1));
  };

  return (
    <div className="page-stack">
      <PageHeader
        eyebrow="创建普通绘本"
        title="新建普通绘本"
        copy={`这本绘本会创建在 ${workspace.name}，后续可直接导出或派生定制版本。`}
      />
      {provider && (
        <Card>
          <div className="section-head">
            <div>
              <p className="eyebrow">生成状态</p>
              <h2>{providerStatusTitle(provider)}</h2>
              <p>{provider.diagnostic}</p>
            </div>
            <Badge tone={provider.realTextReady ? "good" : "warn"}>{provider.provider}</Badge>
          </div>
          <div className="review-list">
            <div><span>文本真实可用</span><strong>{provider.realTextReady ? "是" : "否"}</strong></div>
            <div><span>图片真实可用</span><strong>{provider.realImageReady ? "是" : "否"}</strong></div>
            <div><span>缺失配置</span><strong>{provider.missingConfiguration.length ? provider.missingConfiguration.join(" · ") : "无"}</strong></div>
            {provider.components.map((component) => (
              <div key={`${component.kind}-${component.provider}`}>
                <span>{componentKindLabel(component.kind)}组件</span>
                <strong>{component.provider} · {component.ready ? "已就绪" : `缺少 ${component.requiredConfiguration.join(" · ")}`}</strong>
              </div>
            ))}
          </div>
        </Card>
      )}
      <div className="wizard-shell">
        <WizardSideNav
          title="普通绘本流程"
          copy="先确认故事方案，再确认角色道具，最后编辑分页并导出。"
          steps={steps}
          active={step}
          maxUnlockedStep={unlockedStep}
          onSelect={setStep}
        />
        <Card className="wizard-card">
          {notice && (
            <Notice
              title={notice.title}
              copy={notice.copy}
              tone={retryJob ? "danger" : "info"}
              action={retryJob ? <button className="button secondary" type="button" disabled={generatingStep === retryJob.jobType} onClick={retryFailedGeneration}>重新生成</button> : undefined}
            />
          )}
          {step === 0 && (
            <div className="form-grid">
              <label>绘本标题<input value={form.title} onChange={(event) => updateRequestForm({ title: event.target.value })} /></label>
              <label>绘本主题<input value={form.theme} onChange={(event) => updateRequestForm({ theme: event.target.value })} /></label>
              <label>年龄段<select value={form.ageGroup} onChange={(event) => updateRequestForm({ ageGroup: event.target.value })}><option>3-4 岁</option><option>4-5 岁</option><option>5-6 岁</option></select></label>
              <label>页数<input type="number" value={form.pageCount} onChange={(event) => updateRequestForm({ pageCount: event.target.value })} /></label>
              <label>使用场景<select value={form.useScene} onChange={(event) => updateRequestForm({ useScene: event.target.value })}><option>课堂共读</option><option>规则引导</option><option>家园沟通</option></select></label>
              <label className="span-2">故事风格<textarea rows={3} value={form.style} onChange={(event) => updateRequestForm({ style: event.target.value })} /></label>
            </div>
          )}
          {step === 1 && <ReviewBlock title="绘本方案" output={generationOutputs.storybook_plan} items={storybookPlanItems(generationOutputs.storybook_plan, form, planDraft)} regenerating={generatingStep === "storybook_plan"} onRegenerate={() => runGeneration("storybook_plan", "已重新生成方案")} onEdit={() => setEditingReview(editingReview === "plan" ? null : "plan")} editing={editingReview === "plan"} editor={<PlanEditor form={form} plan={planDraft} onFormChange={setForm} onPlanChange={setPlanDraft} />} />}
          {step === 2 && <ReviewBlock title="角色与关键道具" output={generationOutputs.storybook_roles} items={storybookRoleItems(generationOutputs.storybook_roles, editableRoles, planDraft, form)} regenerating={generatingStep === "storybook_roles"} onRegenerate={() => runGeneration("storybook_roles", "已重新生成角色")} onEdit={() => setEditingReview(editingReview === "roles" ? null : "roles")} editing={editingReview === "roles"} editor={<RoleEditor roles={editableRoles.length ? editableRoles : roleDraftsFromPlan(planDraft, form)} onChange={setEditableRoles} />} />}
          {step === 3 && <ReviewBlock title="分页图文" output={generationOutputs.storybook_pages} items={storybookPageItems(generationOutputs.storybook_pages, editablePages, planDraft, form)} regenerating={generatingStep === "storybook_pages"} onRegenerate={() => runGeneration("storybook_pages", "已重新生成分页")} onEdit={() => setEditingReview(editingReview === "pages" ? null : "pages")} editing={editingReview === "pages"} editor={<PageEditor pages={editablePages.length ? editablePages : pageDraftsFromPlan(planDraft, form)} onChange={setEditablePages} roles={editableRoles} />} />}
          {step === 4 && (
            <div className="preview-complete">
              <Badge tone="good">可导出</Badge>
              <h2>《{form.title || "一起玩小汽车"}》已准备好</h2>
              <p>你可以继续编辑，也可以导出 PDF，或之后基于它生成定制绘本。</p>
              {targetBook ? (
                <Link className="button primary" to={`/app/${workspace.id}/storybooks/${targetBook}`}>进入绘本详情</Link>
              ) : (
                <button className="button primary" type="button" disabled title="需要先成功创建绘本">等待绘本创建完成</button>
              )}
            </div>
          )}
          <div className="wizard-actions">
            <button className="button secondary" disabled={step === 0} title={step === 0 ? "当前已经是第一步" : undefined} onClick={() => { setNotice(null); setStep((value) => Math.max(0, value - 1)); }}>上一步</button>
            <button className="button primary" disabled={step === steps.length - 1 || creating || Boolean(generatingStep)} title={step === steps.length - 1 ? "绘本已生成，请进入详情继续编辑或导出" : undefined} onClick={handlePrimary}>{creating ? "正在创建..." : generatingStep ? "生成中..." : primaryLabels[step]}</button>
          </div>
        </Card>
      </div>
    </div>
  );
}

function generationStatusLabel(status: string) {
  if (status === "queued") return "已加入队列";
  return generationJobStatusLabel[status] || `状态：${status}`;
}

function generationErrorMessage(job: GenerationJob) {
  const output = job.output as { error?: { message?: string } } | undefined;
  return output?.error?.message || "生成任务失败，可稍后重试";
}

function providerStatusTitle(provider: GenerationProviderStatus) {
  if (provider.productionReady) return "真实文本和图片生成已就绪";
  if (provider.realTextReady) return "真实文本生成已就绪";
  if (provider.realImageReady) return "真实图片生成已就绪";
  return "当前使用本地演示生成";
}

function componentKindLabel(kind: string) {
  return kind === "image" ? "图片" : kind === "text" ? "文本" : kind;
}

function generationOutputMeta(output: unknown) {
  const value = output as { provider?: string; schema_version?: string; mode?: string; message?: string } | undefined;
  return {
    provider: value?.provider || "待生成",
    schema: value?.schema_version || "尚无输出",
    mode: value?.mode || "等待任务",
    message: value?.message || "生成后会在这里显示可审核内容。",
    real: value?.schema_version === "generation.provider.v1",
    privacy: generationPrivacyAuditSummary(output),
  };
}

function storybookPlanItems(output: unknown, form: { title: string; theme: string }, draft?: EditablePlan) {
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

function storybookRoleItems(output: unknown, editableRoles: EditableRole[] = [], plan?: EditablePlan, form?: { title: string; theme: string }) {
  const value = output as {
    roles?: { name?: string; role_type?: string; appearance?: string; story_function?: string }[];
    consistency_guide?: string[];
  } | undefined;
  if (editableRoles.length) {
    return [
      ...editableRoles.map((role) => `${role.name}：${role.appearance}；故事作用：${role.storyFunction}；参考图提示：${role.referenceImagePrompt || "沿用外观设定生成稳定参考图"}`),
      "一致性要求：分页正文和插图描述必须使用上面确认的角色名称，不新增替代动物或泛称角色。",
    ];
  }
  if (!value?.roles?.length) {
    const requirements = linesFromRows(plan?.roleRequirementsText || "");
    return [
      `待生成：将根据第 2 步《${form?.title || "当前绘本"}》方案生成角色，不会使用固定示例角色。`,
      requirements.length ? `来自方案的角色需求：${requirements.join("、")}` : `角色会围绕「${form?.theme || "当前主题"}」和故事方案生成。`,
      "操作：点击“生成角色道具”后，再审核或手动修改具体名称、外观、故事作用和参考图提示词。",
    ];
  }

  return [
    ...value.roles.map((role) => `${role.name || "未命名角色"}：${role.appearance || "外观待确认"}；故事作用：${role.story_function || role.role_type || "参与故事推进"}`),
    value.consistency_guide?.length ? `一致性要求：${value.consistency_guide.join("、")}` : null,
  ].filter(Boolean) as string[];
}

function storybookPageItems(output: unknown, editablePages: EditablePage[] = [], plan?: EditablePlan, form?: { title: string; theme: string }) {
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

function ReviewBlock({
  title,
  items,
  output,
  onRegenerate,
  onEdit,
  editor,
  editing = false,
  regenerating = false,
}: {
  title: string;
  items: string[];
  output?: unknown;
  onRegenerate: () => void;
  onEdit: () => void;
  editor?: ReactNode;
  editing?: boolean;
  regenerating?: boolean;
}) {
  const meta = generationOutputMeta(output);
  return (
    <div className="review-block">
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
      <div className="review-list">
        {items.map((item) => <div key={item}><span>确认项</span><strong>{item}</strong></div>)}
      </div>
      {editing && editor}
      <div className="inline-actions">
        <button className="button secondary" type="button" disabled={regenerating} onClick={onRegenerate}>{regenerating ? "生成中..." : "重新生成"}</button>
        <button className="button secondary" type="button" onClick={onEdit}>{editing ? "收起修改" : "手动修改"}</button>
      </div>
    </div>
  );
}

function PlanEditor({
  form,
  plan,
  onFormChange,
  onPlanChange,
}: {
  form: { title: string; theme: string; ageGroup: string; pageCount: string; useScene: string; style: string };
  plan: EditablePlan;
  onFormChange: (value: { title: string; theme: string; ageGroup: string; pageCount: string; useScene: string; style: string }) => void;
  onPlanChange: (value: EditablePlan) => void;
}) {
  return (
    <div className="review-editor">
      <label>绘本标题<input value={form.title} onChange={(event) => onFormChange({ ...form, title: event.target.value })} /></label>
      <label>教学目标<input value={form.theme} onChange={(event) => onFormChange({ ...form, theme: event.target.value })} /></label>
      <label className="span-2">故事概述<textarea rows={3} value={plan.summary} onChange={(event) => onPlanChange({ ...plan, summary: event.target.value })} /></label>
      <label className="span-2">分页节奏<textarea rows={5} value={plan.outlineText} onChange={(event) => onPlanChange({ ...plan, outlineText: event.target.value })} /></label>
      <label className="span-2">角色需求<textarea rows={3} value={plan.roleRequirementsText} onChange={(event) => onPlanChange({ ...plan, roleRequirementsText: event.target.value })} /></label>
      <label className="span-2">老师确认重点<textarea rows={3} value={plan.reviewPointsText} onChange={(event) => onPlanChange({ ...plan, reviewPointsText: event.target.value })} /></label>
    </div>
  );
}

function RoleEditor({ roles, onChange }: { roles: EditableRole[]; onChange: (roles: EditableRole[]) => void }) {
  const update = (index: number, patch: Partial<EditableRole>) => {
    onChange(roles.map((role, roleIndex) => roleIndex === index ? { ...role, ...patch } : role));
  };
  return (
    <div className="review-editor role-editor">
      {roles.map((role, index) => (
        <div className="editable-review-card" key={`${role.id || role.name}-${index}`}>
          <div className="section-head compact">
            <div>
              <p className="eyebrow">角色 {index + 1}</p>
              <h3>{role.name || "未命名角色"}</h3>
            </div>
            <Badge tone={role.needsConsistency ? "info" : "neutral"}>{role.needsConsistency ? "跨页一致" : "可变化"}</Badge>
          </div>
          <label>名称<input value={role.name} onChange={(event) => update(index, { name: event.target.value })} /></label>
          <label>
            类型
            <select value={role.roleType} onChange={(event) => update(index, { roleType: event.target.value as StorybookRole["roleType"] })}>
              <option value="protagonist">主角</option>
              <option value="supporting">配角</option>
              <option value="peer">同伴儿童</option>
              <option value="teacher">老师形象</option>
              <option value="prop">关键道具</option>
            </select>
          </label>
          <label className="span-2">外观细节<textarea rows={4} value={role.appearance} onChange={(event) => update(index, { appearance: event.target.value })} /></label>
          <label className="span-2">故事作用<textarea rows={3} value={role.storyFunction} onChange={(event) => update(index, { storyFunction: event.target.value })} /></label>
          <label className="span-2">参考图提示词<textarea rows={3} value={role.referenceImagePrompt} onChange={(event) => update(index, { referenceImagePrompt: event.target.value })} /></label>
          <label className="check-row"><input type="checkbox" checked={role.needsConsistency} onChange={(event) => update(index, { needsConsistency: event.target.checked })} />后续分页插图保持同一形象</label>
        </div>
      ))}
    </div>
  );
}

function PageEditor({ pages, roles, onChange }: { pages: EditablePage[]; roles: EditableRole[]; onChange: (pages: EditablePage[]) => void }) {
  const update = (index: number, patch: Partial<EditablePage>) => {
    onChange(pages.map((page, pageIndex) => pageIndex === index ? { ...page, ...patch } : page));
  };
  const roleNames = roles.filter((role) => role.needsConsistency).map((role) => role.name).filter(Boolean);
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
      {pages.map((page, index) => (
        <div className="editable-review-card" key={`${page.id || page.pageNumber}-${index}`}>
          <p className="eyebrow">第 {page.pageNumber} 页</p>
          <label>页面标题<input value={page.title} onChange={(event) => update(index, { title: event.target.value })} /></label>
          <label className="span-2">正文<textarea rows={4} value={page.body} onChange={(event) => update(index, { body: event.target.value })} /></label>
          <label className="span-2">插图描述<textarea rows={4} value={page.illustrationPrompt} onChange={(event) => update(index, { illustrationPrompt: event.target.value })} /></label>
        </div>
      ))}
    </div>
  );
}

function generationModeLabel(mode: string) {
  if (mode === "等待任务") return "等待任务";
  return generationJobTypeLabel[mode] || mode;
}

function linesFromText(value: string) {
  return value.split(/\n|、|；|;/).map((item) => item.trim()).filter(Boolean);
}

function linesFromRows(value: string) {
  return value.split(/\n+/).map((item) => item.trim()).filter(Boolean);
}

function planDraftFromOutput(output: unknown, form: { title: string; theme: string }): EditablePlan {
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

function generationInputFor(
  jobType: string,
  form: { title: string; theme: string; ageGroup: string; pageCount: string; useScene: string; style: string },
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
      confirmed_roles: roles.map(rolePayload),
      confirmed_pages: pages.length ? pages.map(pagePayload) : undefined,
    };
  }
  return base;
}

function rolePayload(role: EditableRole) {
  return {
    name: role.name,
    role_type: role.roleType,
    appearance: role.appearance,
    story_function: role.storyFunction,
    reference_image_prompt: role.referenceImagePrompt,
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

function roleFromStorybook(role: StorybookRole): EditableRole {
  return {
    id: role.id,
    name: role.name,
    roleType: role.roleType,
    appearance: role.appearance,
    storyFunction: role.storyFunction,
    needsConsistency: role.needsConsistency,
    referenceImagePrompt: role.referenceImagePrompt || `${role.name}，${role.appearance}，儿童绘本角色参考图，保持跨页一致`,
  };
}

function rolesFromStorybook(roles: StorybookRole[]) {
  return roles.map(roleFromStorybook);
}

function rolesFromOutput(output: unknown): EditableRole[] {
  const value = output as { roles?: { name?: string; role_type?: string; appearance?: string; story_function?: string; reference_image_prompt?: string; needs_consistency?: boolean }[] } | undefined;
  if (!value?.roles?.length) return defaultEditableRoles();
  return value.roles.map((role, index) => normalizeEditableRole({
    name: role.name || `角色 ${index + 1}`,
    roleType: normalizeRoleType(role.role_type),
    appearance: role.appearance || "",
    storyFunction: role.story_function || "",
    referenceImagePrompt: role.reference_image_prompt || "",
    needsConsistency: role.needs_consistency ?? true,
  }));
}

function roleDraftsFromPlan(plan: EditablePlan, form: { title: string; theme: string }): EditableRole[] {
  const requirements = linesFromRows(plan.roleRequirementsText);
  if (!requirements.length) {
    return [
      normalizeEditableRole({
        name: "主角",
        roleType: "protagonist",
        appearance: `围绕《${form.title || form.theme || "当前绘本"}》设计的主角，请补充物种或人物身份、颜色、服装、表情动作和跨页识别特征。`,
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
      appearance: `根据第 2 步方案需求：${detail}。请补充颜色、服装或材质、体型轮廓、表情动作和一个可跨页重复识别的小特征。`,
      storyFunction: detail || `服务于「${form.theme || "当前主题"}」的故事推进。`,
      needsConsistency: true,
      referenceImagePrompt: "",
    });
  });
}

function normalizeEditableRole(role: EditableRole): EditableRole {
  const appearance = role.appearance.trim();
  return {
    ...role,
    appearance: appearance.length >= 28 ? appearance : enrichShortAppearance(role.name, appearance, role.roleType),
    storyFunction: role.storyFunction.trim() || "推动故事冲突和转变，帮助幼儿理解规则与情绪。",
    referenceImagePrompt: role.referenceImagePrompt.trim() || `${role.name}，${appearance || "幼儿绘本角色"}，正面半身角色参考图，温暖手绘风，清晰服装和颜色，保持跨页一致`,
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

function defaultEditableRoles(): EditableRole[] {
  return [
    normalizeEditableRole({
      name: "小兔米米",
      roleType: "protagonist",
      appearance: "白色小兔，圆脸，长耳朵内侧带淡粉色，穿黄色背带裤和浅米色小鞋，表情好奇但有点急切，动作灵活，适合做主角。",
      storyFunction: "主角，一开始想马上玩小汽车，后来学会等待、表达和轮流。",
      needsConsistency: true,
      referenceImagePrompt: "小兔米米，白色圆脸小兔，长耳朵淡粉内侧，黄色背带裤，浅米色小鞋，好奇温暖表情，儿童绘本角色参考图",
    }),
    normalizeEditableRole({
      name: "小熊乐乐",
      roleType: "peer",
      appearance: "浅棕色小熊，圆耳朵，穿蓝色开衫和白色 T 恤，身体微胖，笑容友好，常站在主角旁边等待一起玩。",
      storyFunction: "同伴儿童，和主角一起经历轮流冲突，用友好回应帮助主角完成转变。",
      needsConsistency: true,
      referenceImagePrompt: "小熊乐乐，浅棕色小熊，圆耳朵，蓝色开衫，白色 T 恤，友好笑容，儿童绘本角色参考图",
    }),
    normalizeEditableRole({
      name: "鹿老师",
      roleType: "teacher",
      appearance: "温柔的鹿老师，浅棕色鹿角，戴圆框眼镜，穿米白色针织外套和绿色围裙，常蹲下来与孩子平视交流。",
      storyFunction: "老师引导者，帮助孩子看见情绪，提出简单规则，并鼓励孩子自己尝试。",
      needsConsistency: true,
      referenceImagePrompt: "鹿老师，浅棕色鹿角，圆框眼镜，米白针织外套，绿色围裙，蹲下平视孩子，温柔老师形象参考图",
    }),
    normalizeEditableRole({
      name: "红色小汽车",
      roleType: "prop",
      appearance: "亮红色玩具小汽车，圆润车身，黄色车灯，黑色小轮子，车顶有一颗白色星星贴纸，大小适合两个孩子轮流推着玩。",
      storyFunction: "关键道具，引发轮流等待的故事冲突，也作为规则练习的共同目标。",
      needsConsistency: true,
      referenceImagePrompt: "红色玩具小汽车，圆润车身，黄色车灯，黑色轮子，车顶白色星星贴纸，儿童绘本道具参考图",
    }),
  ];
}

function enrichShortAppearance(name: string, appearance: string, roleType: StorybookRole["roleType"]) {
  const base = appearance || (roleType === "prop" ? "关键道具" : "儿童绘本角色");
  return `${base}；请补足颜色、服装或材质、体型轮廓、表情动作和一个可跨页重复识别的小特征，作为「${name}」的稳定参考设定。`;
}

function pageFromStorybook(page: StorybookPage): EditablePage {
  return {
    id: page.id,
    pageNumber: page.pageNumber,
    title: page.title,
    body: page.body,
    illustrationPrompt: page.illustrationPrompt,
  };
}

function pagesFromStorybook(pages: StorybookPage[]) {
  return pages.map(pageFromStorybook);
}

function pagesFromOutput(output: unknown): EditablePage[] {
  const value = output as { pages?: { page_number?: number; title?: string; body?: string; illustration_prompt?: string }[] } | undefined;
  if (!value?.pages?.length) return defaultEditablePages();
  return value.pages.map((page, index) => ({
    pageNumber: page.page_number || index + 1,
    title: page.title || `第 ${index + 1} 页`,
    body: page.body || "",
    illustrationPrompt: page.illustration_prompt || "",
  }));
}

function pageDraftsFromPlan(plan: EditablePlan, form: { title: string; theme: string }): EditablePage[] {
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

function defaultEditablePages(): EditablePage[] {
  return [
    { pageNumber: 1, title: "小汽车来到教室", body: "小兔米米看见红色小汽车，眼睛一下子亮了起来。", illustrationPrompt: "明亮教室里，小兔米米看着红色小汽车，小熊乐乐站在旁边，鹿老师在远处微笑观察。" },
    { pageNumber: 2, title: "朋友也想玩", body: "小熊乐乐也想试一试，小兔米米抱着小汽车不想松手。", illustrationPrompt: "小兔米米抱着红色小汽车，小熊乐乐伸出手想一起玩，鹿老师蹲下来准备引导。" },
    { pageNumber: 3, title: "鹿老师给出办法", body: "鹿老师说：我们可以排队，用沙漏提醒轮流时间。", illustrationPrompt: "鹿老师蹲在小兔米米和小熊乐乐身边，手里拿着沙漏，红色小汽车放在地毯上。" },
    { pageNumber: 4, title: "轮到我，也轮到你", body: "小兔米米先玩一会儿，再把小汽车推给小熊乐乐。", illustrationPrompt: "小兔米米把红色小汽车推向小熊乐乐，两个人都看着沙漏，鹿老师在旁边鼓励。" },
  ];
}
