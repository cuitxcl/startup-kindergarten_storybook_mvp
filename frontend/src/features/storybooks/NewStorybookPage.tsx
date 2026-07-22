import { useEffect, useState } from "react";
import { Link, useOutletContext } from "react-router-dom";
import { createGenerationJob, createStorybook, getGenerationJob, getWorkspaceGenerationProvider, retryGenerationJob, shouldUseApi, updateStorybook, type GenerationJob, type GenerationProviderStatus } from "../../api/client";
import { Badge, Card, Notice, PageHeader, WizardSideNav } from "../../components/ui";
import { storybooks } from "../../data/mock";
import type { Workspace } from "../../types/domain";
import { generationPrivacyAuditSummary } from "../../utils/labels";

const steps = ["需求", "绘本方案", "角色道具", "分页编辑", "预览导出"];

export function NewStorybookPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const [step, setStep] = useState(0);
  const [notice, setNotice] = useState<{ title: string; copy: string } | null>(null);
  const [creating, setCreating] = useState(false);
  const [generatingStep, setGeneratingStep] = useState<string | null>(null);
  const [createdBookId, setCreatedBookId] = useState<string | null>(null);
  const [retryJob, setRetryJob] = useState<GenerationJob | null>(null);
  const [generationOutputs, setGenerationOutputs] = useState<Record<string, unknown>>({});
  const [provider, setProvider] = useState<GenerationProviderStatus | null>(null);
  const [form, setForm] = useState({
    title: "一起玩小汽车",
    theme: "学会分享和轮流",
    ageGroup: "4-5 岁",
    pageCount: "6",
    useScene: "规则引导",
    style: "温暖、生活化，有清晰的老师引导。",
  });
  const targetBook = shouldUseApi ? createdBookId : storybooks.find((item) => item.workspaceId === workspace.id)?.id || "storybook-1";
  const primaryLabels = ["生成绘本方案", "确认方案，继续角色", "确认角色，继续分页", "确认分页，进入预览", "已完成"];
  const showNotice = (title: string, copy: string) => {
    setRetryJob(null);
    setNotice({ title, copy });
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
        input: {
          title: form.title,
          theme: form.theme,
          age_group: form.ageGroup,
          page_count: form.pageCount,
          use_scene: form.useScene,
          style: form.style,
        },
      });
      const settledJob = await waitForGenerationJob(job);
      return handleGenerationJob(settledJob, title);
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
      handleGenerationJob(settledJob, "已重新生成");
    } catch (err) {
      setNotice({ title: "重试失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setGeneratingStep(null);
    }
  };
  const handleGenerationJob = (job: GenerationJob, title: string) => {
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
    }
    setNotice({ title, copy: `生成任务${generationStatusLabel(job.status)}，任务编号：${job.id.slice(0, 8)}。` });
    return true;
  };
  const handlePrimary = async () => {
    setNotice(null);
    if (step === 0) {
      if (await runGeneration("storybook_plan", "绘本方案已生成")) {
        setStep(1);
      }
      return;
    }
    if (shouldUseApi && step === 1 && !createdBookId) {
      try {
        await ensureStorybookCreated();
        setNotice({ title: "普通绘本已创建", copy: "后续角色和分页生成会直接写入这本绘本，进入详情后可继续编辑、导出或派生定制版本。" });
      } catch (err) {
        setNotice({ title: "创建失败", copy: err instanceof Error ? err.message : "请稍后重试" });
        return;
      }
    }
    if (step === 2) {
      if (await runGeneration("storybook_roles", "角色与道具已生成并写入绘本")) {
        if (shouldUseApi && createdBookId) {
          await updateStorybook(workspace.id, createdBookId, { status: "roles_pending" });
        }
        setStep(3);
      }
      return;
    }
    if (step === 3) {
      if (await runGeneration("storybook_pages", "分页图文已生成并写入绘本")) {
        if (shouldUseApi && createdBookId) {
          await updateStorybook(workspace.id, createdBookId, { status: "editing" });
          await updateStorybook(workspace.id, createdBookId, { status: "exportable" });
        }
        setStep(4);
      }
      return;
    }
    setStep((value) => Math.min(steps.length - 1, value + 1));
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
              <label>绘本标题<input value={form.title} onChange={(event) => setForm({ ...form, title: event.target.value })} /></label>
              <label>绘本主题<input value={form.theme} onChange={(event) => setForm({ ...form, theme: event.target.value })} /></label>
              <label>年龄段<select value={form.ageGroup} onChange={(event) => setForm({ ...form, ageGroup: event.target.value })}><option>3-4 岁</option><option>4-5 岁</option><option>5-6 岁</option></select></label>
              <label>页数<input type="number" value={form.pageCount} onChange={(event) => setForm({ ...form, pageCount: event.target.value })} /></label>
              <label>使用场景<select value={form.useScene} onChange={(event) => setForm({ ...form, useScene: event.target.value })}><option>课堂共读</option><option>规则引导</option><option>家园沟通</option></select></label>
              <label className="span-2">故事风格<textarea rows={3} value={form.style} onChange={(event) => setForm({ ...form, style: event.target.value })} /></label>
            </div>
          )}
          {step === 1 && <ReviewBlock title="绘本方案" output={generationOutputs.storybook_plan} items={storybookPlanItems(generationOutputs.storybook_plan, form)} regenerating={generatingStep === "storybook_plan"} onRegenerate={() => runGeneration("storybook_plan", "已重新生成方案")} onEdit={() => showNotice("已进入手动修改模式", "你可以回到需求页调整标题、主题和故事风格，再重新生成方案。")} />}
          {step === 2 && <ReviewBlock title="角色与关键道具" output={generationOutputs.storybook_roles} items={storybookRoleItems(generationOutputs.storybook_roles)} regenerating={generatingStep === "storybook_roles"} onRegenerate={() => runGeneration("storybook_roles", "已重新生成角色")} onEdit={() => showNotice("已打开角色修改", "当前版本先在绘本详情页继续编辑角色设定，后续会支持向导内逐个修改。")} />}
          {step === 3 && <ReviewBlock title="分页图文" output={generationOutputs.storybook_pages} items={storybookPageItems(generationOutputs.storybook_pages)} regenerating={generatingStep === "storybook_pages"} onRegenerate={() => runGeneration("storybook_pages", "已重新生成分页")} onEdit={() => showNotice("已打开分页修改", "创建绘本后可进入详情页逐页修改标题、正文和插图描述。")} />}
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
  return {
    queued: "已加入队列",
    running: "正在生成",
    succeeded: "已完成",
    failed: "失败",
  }[status] || `状态：${status}`;
}

function generationErrorMessage(job: GenerationJob) {
  const output = job.output as { error?: { message?: string } } | undefined;
  return output?.error?.message || "生成任务失败，可稍后重试";
}

function providerStatusTitle(provider: GenerationProviderStatus) {
  if (provider.productionReady) return "真实文本和图片生成已就绪";
  if (provider.realTextReady) return "真实文本生成已就绪";
  if (provider.realImageReady) return "真实图片生成已就绪";
  return "当前使用 mock 生成";
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

function storybookPlanItems(output: unknown, form: { title: string; theme: string }) {
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

function storybookRoleItems(output: unknown) {
  const value = output as {
    roles?: { name?: string; role_type?: string; appearance?: string; story_function?: string }[];
    consistency_guide?: string[];
  } | undefined;
  if (!value?.roles?.length) {
    return ["小兔米米：白色小兔，黄色背带裤", "小熊乐乐：友好朋友，蓝色上衣", "鹿老师：戴圆眼镜，温柔引导", "红色小汽车：引发轮流的核心道具"];
  }

  return [
    ...value.roles.map((role) => `${role.name || "未命名角色"}：${role.appearance || "外观待确认"}；作用：${role.story_function || role.role_type || "参与故事推进"}`),
    value.consistency_guide?.length ? `一致性要求：${value.consistency_guide.join("、")}` : null,
  ].filter(Boolean) as string[];
}

function storybookPageItems(output: unknown) {
  const value = output as {
    pages?: { page_number?: number; title?: string; body?: string; illustration_prompt?: string }[];
    editor_notes?: string[];
  } | undefined;
  if (!value?.pages?.length) {
    return ["第 1 页：小汽车来到教室", "第 2 页：朋友也想玩", "第 3 页：老师给出办法", "第 4-6 页：尝试轮流并整理玩具"];
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
  regenerating = false,
}: {
  title: string;
  items: string[];
  output?: unknown;
  onRegenerate: () => void;
  onEdit: () => void;
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
      <div className="inline-actions">
        <button className="button secondary" type="button" disabled={regenerating} onClick={onRegenerate}>{regenerating ? "生成中..." : "重新生成"}</button>
        <button className="button secondary" type="button" onClick={onEdit}>手动修改</button>
      </div>
    </div>
  );
}

function generationModeLabel(mode: string) {
  return {
    storybook_plan: "故事方案",
    storybook_roles: "角色与道具",
    storybook_pages: "分页图文",
    customization_plan: "定制方案",
    "等待任务": "等待任务",
  }[mode] || mode;
}
