import { type ReactNode, useEffect, useState } from "react";
import { Link, useNavigate, useOutletContext } from "react-router-dom";
import {
  createGenerationJob,
  createStorybook,
  getGenerationJob,
  getStorybook,
  getWorkspaceGenerationProvider,
  retryGenerationJob,
  updateStorybook,
  updateStorybookPage,
  updateStorybookRole,
  type GenerationJob,
  type GenerationProviderStatus,
} from "../../api/client";
import { Badge, Card, Notice, PageHeader, WizardSideNav } from "../../components/ui";
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
  const targetBook = createdBookId;
  const generatedRoles = rolesFromOutput(generationOutputs.storybook_roles);
  const generatedPages = pagesFromOutput(generationOutputs.storybook_pages);
  const currentRoles = editableRoles.length ? editableRoles : generatedRoles;
  const currentPages = editablePages.length ? editablePages : generatedPages;
  const hasPlan = Boolean(generationOutputs.storybook_plan || planDraft.summary || planDraft.outlineText);
  const hasRoles = currentRoles.length > 0;
  const hasPages = currentPages.length > 0;
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
    getWorkspaceGenerationProvider(workspace.id).then(setProvider).catch(() => setProvider(null));
  }, [workspace.id]);
  const ensureStorybookCreated = async () => {
    if (createdBookId) return createdBookId;
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
        input: generationInputFor(jobType, form, planDraft, currentRoles, currentPages),
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
        const roles = job.storybookId
          ? rolesFromStorybook((await getStorybook(workspace.id, job.storybookId)).roles)
          : rolesFromOutput(job.output);
        setEditableRoles(roles.length ? roles : rolesFromOutput(job.output));
      }
      if (job.jobType === "storybook_pages") {
        const pages = job.storybookId
          ? pagesFromStorybook((await getStorybook(workspace.id, job.storybookId)).pages)
          : pagesFromOutput(job.output);
        setEditablePages(pages.length ? pages : pagesFromOutput(job.output));
      }
    }
    setNotice({ title, copy: `生成任务${generationStatusLabel(job.status)}，任务编号：${job.id.slice(0, 8)}。` });
    return true;
  };
  const persistRoles = async (bookId: string, rolesToPersist = currentRoles) => {
    if (!rolesToPersist.length) return;
    const book = await getStorybook(workspace.id, bookId);
    const updated = await Promise.all(rolesToPersist.map(async (role, index) => {
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
  const persistPages = async (bookId: string, pagesToPersist = currentPages) => {
    if (!pagesToPersist.length) return;
    const book = await getStorybook(workspace.id, bookId);
    const updated = await Promise.all(pagesToPersist.map(async (page, index) => {
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
    if (step === 1 && !createdBookId) {
      try {
        const bookId = await ensureStorybookCreated();
        if (bookId) await persistStorybookMeta(bookId);
        setNotice({ title: "普通绘本已创建", copy: "后续角色和分页生成会直接写入这本绘本，进入详情后可继续编辑、导出或派生定制版本。" });
      } catch (err) {
        setNotice({ title: "创建失败", copy: err instanceof Error ? err.message : "请稍后重试" });
        return;
      }
    } else if (step === 1 && createdBookId) {
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
        await persistRoles(bookId, currentRoles);
      }
      if (await runGeneration("storybook_pages", "分页图文已生成并写入绘本")) {
        if (bookId) {
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
        const bookId = createdBookId;
        if (bookId) {
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
      {provider && !provider.realTextReady && (
        <Notice
          title="真实文本生成暂不可用"
          copy={`${provider.diagnostic}${provider.missingConfiguration.length ? ` 缺少：${provider.missingConfiguration.join("、")}` : ""}`}
          tone="warn"
        />
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
          {step === 2 && <ReviewBlock title="角色与关键道具" output={generationOutputs.storybook_roles} items={storybookRoleItems(generationOutputs.storybook_roles, currentRoles, planDraft, form)} regenerating={generatingStep === "storybook_roles"} onRegenerate={() => runGeneration("storybook_roles", "已重新生成角色")} onEdit={() => setEditingReview(editingReview === "roles" ? null : "roles")} editing={editingReview === "roles"} editor={<RoleEditor roles={currentRoles.length ? currentRoles : roleDraftsFromPlan(planDraft, form)} onChange={setEditableRoles} />} />}
          {step === 3 && <ReviewBlock title="分页图文" output={generationOutputs.storybook_pages} items={storybookPageItems(generationOutputs.storybook_pages, currentPages, planDraft, form)} regenerating={generatingStep === "storybook_pages"} onRegenerate={() => runGeneration("storybook_pages", "已重新生成分页")} onEdit={() => setEditingReview(editingReview === "pages" ? null : "pages")} editing={editingReview === "pages"} editor={<PageEditor pages={currentPages.length ? currentPages : pageDraftsFromPlan(planDraft, form)} onChange={setEditablePages} roles={currentRoles} />} />}
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
      "操作：点击“生成角色道具”后，再审核或手动修改具体名称、稳定外观和故事作用。",
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
              <option value="peer">同伴角色</option>
              <option value="teacher">老师形象</option>
              <option value="prop">关键道具</option>
            </select>
          </label>
          <label className="span-2">稳定外观<textarea rows={4} value={role.appearance} onChange={(event) => update(index, { appearance: event.target.value })} /></label>
          <label className="span-2">故事作用<textarea rows={3} value={role.storyFunction} onChange={(event) => update(index, { storyFunction: event.target.value })} /></label>
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
  const appearance = cleanVisualAppearance(role.appearance);
  return {
    name: role.name,
    role_type: role.roleType,
    appearance,
    story_function: role.storyFunction,
    reference_image_prompt: role.needsConsistency ? role.referenceImagePrompt || `${role.name}，${appearance || "绘本角色"}，温暖绘本风格，单一角色标准图，白底或简洁背景，画面只有这个角色，无人类，无其他角色，保持跨页一致` : undefined,
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
    appearance: cleanVisualAppearance(role.appearance),
    storyFunction: role.storyFunction,
    needsConsistency: role.needsConsistency,
    referenceImagePrompt: role.referenceImagePrompt || "",
  };
}

function rolesFromStorybook(roles: StorybookRole[]) {
  return roles.map(roleFromStorybook);
}

function rolesFromOutput(output: unknown): EditableRole[] {
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

function roleDraftsFromPlan(plan: EditablePlan, form: { title: string; theme: string }): EditableRole[] {
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

function cleanVisualAppearance(value: string) {
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
  if (!value?.pages?.length) return [];
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
