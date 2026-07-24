import { useEffect, useState } from "react";
import { Link, useLocation, useOutletContext, useParams } from "react-router-dom";
import {
  createGenerationJob,
  deriveCustomStorybooksBatch,
  deriveCustomStorybook,
  getChild,
  getGenerationJob,
  getStorybook,
  getWorkspaceGenerationProvider,
  listChildrenPage,
  listStorybookGenerationJobs,
  retryGenerationJob,
  shouldUseApi,
  type GenerationJob,
  type GenerationProviderStatus,
  type PaginationMeta,
} from "../../api/client";
import { Badge, Card, EmptyState, Notice, PageHeader, WizardSideNav, statusTone } from "../../components/ui";
import { children, storybooks } from "../../data/mock";
import type { ChildProfile, Storybook, Workspace } from "../../types/domain";
import { generationJobNextAction, generationPrivacyAuditSummary } from "../../utils/labels";

const steps = ["选择孩子", "档案检查", "定制强度", "定制方案", "生成副本"];
const CHILD_PAGE_SIZE = 12;
const MAX_BATCH_CUSTOM_CHILDREN = 30;

export function CustomizeStorybookPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const { storybookId } = useParams();
  const location = useLocation();
  const [step, setStep] = useState(0);
  const [customMode, setCustomMode] = useState<"single" | "batch">("single");
  const [selectedChildId, setSelectedChildId] = useState<string | null>(null);
  const [selectedBatchChildIds, setSelectedBatchChildIds] = useState<string[]>([]);
  const [intensity, setIntensity] = useState<"quick" | "standard">("standard");
  const [notice, setNotice] = useState<{ title: string; copy: string } | null>(null);
  const [remoteSource, setRemoteSource] = useState<Storybook | null>(null);
  const [remoteChildren, setRemoteChildren] = useState<ChildProfile[]>([]);
  const [childPageMeta, setChildPageMeta] = useState<PaginationMeta | null>(null);
  const [loadingMoreChildren, setLoadingMoreChildren] = useState(false);
  const [generatedBookId, setGeneratedBookId] = useState<string | null>(null);
  const [retryJob, setRetryJob] = useState<GenerationJob | null>(null);
  const [generationJobs, setGenerationJobs] = useState<GenerationJob[]>([]);
  const [customizationPlan, setCustomizationPlan] = useState<unknown>(null);
  const [loading, setLoading] = useState(shouldUseApi);
  const [generatingPlan, setGeneratingPlan] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState("");
  const [provider, setProvider] = useState<GenerationProviderStatus | null>(null);
  const source = shouldUseApi ? remoteSource : storybooks.find((item) => item.id === storybookId) || storybooks[0];
  const childList = shouldUseApi ? remoteChildren : children.filter((item) => item.workspaceId === workspace.id);
  const selected = childList.find((child) => child.id === selectedChildId) || childList[0];
  const selectedBatchChildren = childList.filter((child) => selectedBatchChildIds.includes(child.id));
  const generatedTarget = shouldUseApi ? generatedBookId : storybooks.find((item) => item.workspaceId === workspace.id && item.type === "custom")?.id || source?.id;
  const batchSelectionAtLimit = selectedBatchChildIds.length >= MAX_BATCH_CUSTOM_CHILDREN;
  const canContinueSelection = customMode === "batch" ? selectedBatchChildIds.length > 0 && selectedBatchChildIds.length <= MAX_BATCH_CUSTOM_CHILDREN : Boolean(selected);
  const primaryLabels = ["确认孩子", "确认档案", "生成定制方案", "生成定制副本", "已生成"];
  const nextStep = () => {
    setNotice(null);
    setRetryJob(null);
    setStep((value) => Math.min(steps.length - 1, value + 1));
  };

  const createCustomizationPlan = async () => {
    const planChild = customMode === "batch" ? selectedBatchChildren[0] : selected;
    if (!source || !planChild) return;
    if (!shouldUseApi) {
      setNotice({
        title: "定制方案已生成",
        copy: customMode === "batch"
          ? "当前为本地原型反馈；批量模式会按每个儿童资料创建独立定制副本。"
          : "当前为本地原型反馈；接入 API 后会创建定制方案任务。",
      });
      setStep(3);
      return;
    }
    setGeneratingPlan(true);
    setRetryJob(null);
    setNotice(null);
    try {
      const job = await createGenerationJob(workspace.id, {
        jobType: "customization_plan",
        storybookId: source.id,
        input: {
          child_id: planChild.id,
          child_nickname: customMode === "batch" ? `${planChild.nickname} 等 ${selectedBatchChildren.length} 个儿童` : planChild.nickname,
          intensity,
          source_title: source.title,
          interests: planChild.interests,
          focus: planChild.focus,
          batch_child_ids: customMode === "batch" ? selectedBatchChildIds : undefined,
          batch_child_count: customMode === "batch" ? selectedBatchChildren.length : undefined,
        },
      });
      const settledJob = await waitForGenerationJob(job);
      if (source?.id) {
        setGenerationJobs(await listStorybookGenerationJobs(workspace.id, source.id));
      }
      handleGenerationJob(settledJob);
    } catch (err) {
      setRetryJob(null);
      setNotice({ title: "方案生成失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setGeneratingPlan(false);
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

  const retryCustomizationPlan = async () => {
    if (!retryJob) return;
    setGeneratingPlan(true);
    setNotice(null);
    try {
      const job = await retryGenerationJob(workspace.id, retryJob.id);
      const settledJob = await waitForGenerationJob(job);
      if (source?.id) {
        setGenerationJobs(await listStorybookGenerationJobs(workspace.id, source.id));
      }
      handleGenerationJob(settledJob);
    } catch (err) {
      setNotice({ title: "重试失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setGeneratingPlan(false);
    }
  };

  const handleGenerationJob = (job: GenerationJob) => {
    if (job.status === "failed") {
      setRetryJob(job);
      setNotice({ title: "方案生成失败", copy: `${generationErrorMessage(job)}。任务编号：${job.id.slice(0, 8)}。` });
      return;
    }
    if (["queued", "running"].includes(job.status)) {
      setRetryJob(null);
      setNotice({ title: "定制方案仍在生成", copy: `当前状态：${generationStatusLabel(job.status)}。任务编号：${job.id.slice(0, 8)}，稍后可重新点击继续。` });
      return;
    }
    setRetryJob(null);
    if (job.output) {
      setCustomizationPlan(job.output);
    }
    setNotice({ title: "定制方案已生成", copy: `生成任务${generationStatusLabel(job.status)}，任务编号：${job.id.slice(0, 8)}。` });
    setStep(3);
  };

  useEffect(() => {
    if (!shouldUseApi || !storybookId) return;
    let mounted = true;
    setLoading(true);
    setRemoteSource(null);
    setRemoteChildren([]);
    setChildPageMeta(null);
    setGeneratedBookId(null);
    setGenerationJobs([]);
    setCustomizationPlan(null);
    setSelectedChildId(null);
    setError("");
    async function load() {
      try {
        const requestedChildId = new URLSearchParams(location.search).get("childId");
        const [book, childPage, requestedChild] = await Promise.all([
          getStorybook(workspace.id, storybookId!),
          listChildrenPage(workspace.id, { limit: CHILD_PAGE_SIZE, offset: 0 }),
          requestedChildId ? getChild(workspace.id, requestedChildId).catch(() => null) : Promise.resolve(null),
        ]);
        if (!mounted) return;
        const childRows = requestedChild && !childPage.data.some((child) => child.id === requestedChild.id)
          ? [requestedChild, ...childPage.data]
          : childPage.data;
        setRemoteSource(book);
        setRemoteChildren(childRows);
        setChildPageMeta(childPage.meta);
        setSelectedChildId((value) => requestedChild?.id || value || childRows[0]?.id || null);
        setSelectedBatchChildIds((value) => value.filter((id) => childRows.some((child) => child.id === id)));
        try {
          setGenerationJobs(await listStorybookGenerationJobs(workspace.id, book.id, { limit: 8 }));
        } catch {
          setGenerationJobs([]);
        }
        setError("");
      } catch (err) {
        if (!mounted) return;
        setRemoteSource(null);
        setRemoteChildren([]);
        setChildPageMeta(null);
        setGenerationJobs([]);
        setError(err instanceof Error ? err.message : "无法读取定制信息");
      } finally {
        if (mounted) setLoading(false);
      }
    }
    load();
    return () => {
      mounted = false;
    };
  }, [workspace.id, storybookId, location.search]);

  useEffect(() => {
    if (!shouldUseApi) return;
    getWorkspaceGenerationProvider(workspace.id).then(setProvider).catch(() => setProvider(null));
  }, [workspace.id]);

  useEffect(() => {
    const params = new URLSearchParams(location.search);
    const childId = params.get("childId");
    if (childId && childList.some((child) => child.id === childId)) {
      setSelectedChildId(childId);
    }
  }, [childList, location.search]);

  const createCustomCopy = async () => {
    if (!source || (customMode === "single" && !selected) || (customMode === "batch" && selectedBatchChildIds.length === 0)) return;
    if (!shouldUseApi) {
      setNotice({ title: customMode === "batch" ? "批量定制副本已生成" : "定制副本已生成", copy: "原型模式：会创建独立副本并进入编辑状态。" });
      setStep(4);
      return;
    }
    setGenerating(true);
    setRetryJob(null);
    setNotice(null);
    try {
      if (customMode === "batch") {
        const result = await deriveCustomStorybooksBatch(workspace.id, source.id, {
          childIds: selectedBatchChildIds,
          intensity,
          customizationPlan: customizationPlan || undefined,
        });
        setGeneratedBookId(result.storybooks[0]?.id || null);
        setNotice({ title: "批量定制副本已生成", copy: `后端已创建 ${result.createdCount} 本独立定制绘本，原普通绘本不会被覆盖。` });
      } else {
        const book = await deriveCustomStorybook(workspace.id, source.id, { childId: selected!.id, intensity, customizationPlan: customizationPlan || undefined });
        setGeneratedBookId(book.id);
        setNotice({ title: "定制副本已生成", copy: "后端已创建独立定制绘本，原普通绘本不会被覆盖。" });
      }
      if (source?.id) {
        setGenerationJobs(await listStorybookGenerationJobs(workspace.id, source.id));
      }
      setStep(4);
    } catch (err) {
      setNotice({ title: "生成失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setGenerating(false);
    }
  };

  const toggleBatchChild = (childId: string) => {
    setCustomizationPlan(null);
    setSelectedBatchChildIds((items) => items.includes(childId)
      ? items.filter((id) => id !== childId)
      : items.length >= MAX_BATCH_CUSTOM_CHILDREN
        ? items
        : [...items, childId]);
  };

  const loadMoreChildren = async () => {
    if (!shouldUseApi || !childPageMeta?.has_more) return;
    setLoadingMoreChildren(true);
    setNotice(null);
    try {
      const nextOffset = childPageMeta.offset + childPageMeta.limit;
      const page = await listChildrenPage(workspace.id, { limit: CHILD_PAGE_SIZE, offset: nextOffset });
      setRemoteChildren((items) => [
        ...items,
        ...page.data.filter((child) => !items.some((item) => item.id === child.id)),
      ]);
      setChildPageMeta(page.meta);
    } catch (err) {
      setNotice({ title: "儿童列表加载失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setLoadingMoreChildren(false);
    }
  };

  if (loading) {
    return <EmptyState title="正在加载定制信息" copy="正在读取源绘本和儿童档案。" />;
  }

  if (error || !source) {
    return <EmptyState title="无法生成定制绘本" copy={error || "没有找到源绘本。"} action={<Link className="button secondary" to={`/app/${workspace.id}/storybooks`}>返回绘本列表</Link>} />;
  }

  return (
    <div className="page-stack">
      <PageHeader
        eyebrow="生成定制绘本"
        title={`从《${source.title}》生成定制绘本`}
        copy="系统会先展示定制方案，确认后创建独立副本，不会覆盖原普通绘本。"
      />
      {selected && (
        <Card>
          <div className="section-head">
            <div>
              <p className="eyebrow">当前儿童</p>
              <h2>{customMode === "batch" ? `已选择 ${selectedBatchChildIds.length} 个儿童` : selected.nickname}</h2>
              <p>{customMode === "batch" ? "批量模式会为每个儿童创建独立副本。" : `${selected.ageGroup} · ${selected.focus}`}</p>
            </div>
            <div className="inline-actions">
              {customMode === "single" && <Link className="button secondary" to={`/app/${workspace.id}/children/${selected.id}`}>回到儿童档案</Link>}
              <Badge tone={customMode === "batch" ? selectedBatchChildIds.length > 0 ? "good" : "warn" : selected.completeness > 80 ? "good" : "warn"}>
                {customMode === "batch" ? `${selectedBatchChildIds.length} 个儿童` : `完整度 ${selected.completeness}%`}
              </Badge>
            </div>
          </div>
        </Card>
      )}
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
          title="定制绘本流程"
          copy="先确认孩子资料，再决定定制强度，最后生成独立副本。"
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
              action={retryJob ? <button className="button secondary" type="button" disabled={generatingPlan} onClick={retryCustomizationPlan}>重新生成</button> : undefined}
            />
          )}
          {step === 0 && (
            childList.length === 0 ? (
              <EmptyState
                title="还没有可定制的儿童资料"
                copy="先新增儿童档案，再回到这里选择孩子生成定制副本。"
                action={<Link className="button primary" to={`/app/${workspace.id}/children`}>去新增儿童资料</Link>}
              />
            ) : (
              <div className="review-block">
                <div className="privacy-callout">
                  选择孩子后，系统会带入称呼、年龄段、兴趣、性格和关注点，生成独立定制副本。批量生成会为每个儿童各创建一本副本，适合全班快速交付。
                </div>
                <div className="filter-row" role="group" aria-label="定制模式">
                  <button type="button" className={`filter ${customMode === "single" ? "active" : ""}`} onClick={() => setCustomMode("single")}>单个儿童</button>
                  <button type="button" className={`filter ${customMode === "batch" ? "active" : ""}`} onClick={() => setCustomMode("batch")}>批量生成</button>
                  {customMode === "batch" && <Badge tone={selectedBatchChildIds.length ? "good" : "warn"}>已选 {selectedBatchChildIds.length}</Badge>}
                  {customMode === "batch" && <Badge tone={batchSelectionAtLimit ? "warn" : "neutral"}>最多 {MAX_BATCH_CUSTOM_CHILDREN} 个</Badge>}
                </div>
                <div className="selection-grid">
                  {childList.map((child) => {
                    const active = customMode === "batch" ? selectedBatchChildIds.includes(child.id) : selected?.id === child.id;
                    const disabled = customMode === "batch" && batchSelectionAtLimit && !active;
                    return (
                      <button
                        key={child.id}
                        type="button"
                        className={`select-card ${active ? "active" : ""}`}
                        disabled={disabled}
                        title={disabled ? `一次最多选择 ${MAX_BATCH_CUSTOM_CHILDREN} 个儿童` : undefined}
                        onClick={() => {
                          if (customMode === "batch") {
                            toggleBatchChild(child.id);
                            return;
                          }
                          setSelectedChildId(child.id);
                          setCustomizationPlan(null);
                        }}
                      >
                        <strong>{child.nickname}</strong>
                        <span>{child.ageGroup} · {child.interests.join("、")}</span>
                        <Badge tone={child.completeness > 80 ? "good" : "warn"}>
                          {active ? customMode === "batch" ? "已加入 · " : "已选择 · " : ""}完整度 {child.completeness}%
                        </Badge>
                      </button>
                    );
                  })}
                </div>
                {shouldUseApi && childPageMeta?.has_more && (
                  <button className="button secondary" type="button" disabled={loadingMoreChildren} onClick={loadMoreChildren}>
                    {loadingMoreChildren ? "加载中..." : "继续加载儿童"}
                  </button>
                )}
              </div>
            )
          )}
          {step === 1 && (customMode === "batch"
            ? <ReviewBlock title="档案检查" items={[`批量儿童：${selectedBatchChildren.length} 个`, `平均完整度：${averageCompleteness(selectedBatchChildren)}%`, `本次模式：每个儿童生成独立副本`, ...selectedBatchChildren.slice(0, 6).map((child) => `${child.nickname}：${child.ageGroup} · ${child.focus}`)]} />
            : selected && <ReviewBlock title="档案检查" items={[`称呼：${selected.nickname}`, `年龄段：${selected.ageGroup}`, `可用个性化元素：${selected.interests.join("、")}`, `关注点：${selected.focus}`]} />
          )}
          {step === 2 && (
            <div className="selection-grid">
              <button type="button" className={`select-card ${intensity === "quick" ? "active" : ""}`} onClick={() => { setIntensity("quick"); setCustomizationPlan(null); }}><strong>快速定制</strong><span>替换称呼、标题和关键道具，适合批量交付。</span><Badge tone={intensity === "quick" ? "good" : "neutral"}>{intensity === "quick" ? "已选择" : "可选择"}</Badge></button>
              <button type="button" className={`select-card ${intensity === "standard" ? "active" : ""}`} onClick={() => { setIntensity("standard"); setCustomizationPlan(null); }}><strong>标准定制</strong><span>改写关键页面并重绘关键插图，适合单个孩子。</span><Badge tone={intensity === "standard" ? "good" : "neutral"}>{intensity === "standard" ? "已选择" : "可选择"}</Badge></button>
            </div>
          )}
          {step === 3 && (customMode === "batch"
            ? <ReviewBlock title="定制方案" output={customizationPlan} items={batchCustomizationPlanItems(customizationPlan, selectedBatchChildren, intensity)} />
            : selected && <ReviewBlock title="定制方案" output={customizationPlan} items={customizationPlanItems(customizationPlan, selected, intensity)} />
          )}
          {shouldUseApi && (
            <Card>
              <div className="section-head">
                <div>
                  <p className="eyebrow">Recent</p>
                  <h2>最近定制任务</h2>
                </div>
                <Badge tone={generationJobs.some((job) => job.status === "failed") ? "danger" : generationJobs.length ? "good" : "neutral"}>
                  {generationJobs.length ? `${generationJobs.length} 条` : "暂无记录"}
                </Badge>
              </div>
              {generationJobs.length === 0 ? (
                <EmptyState title="还没有定制任务" copy="选择孩子并生成定制方案后，这里会显示对应任务状态。" />
              ) : (
                <div className="compact-list generation-job-list">
                  {generationJobs.slice(0, 4).map((job) => (
                    <div key={job.id} className="compact-row static">
                      <div>
                        <strong>{job.jobType === "customization_plan" ? "定制方案" : job.jobType}</strong>
                        <span>{job.status === "failed" ? generationErrorMessage(job) : job.status === "running" ? "任务正在生成中。" : "已完成或已排队。"}</span>
                        <small>{generationJobNextAction(job)}</small>
                        <small>任务 {job.id.slice(0, 8)} · {job.finishedAt || job.createdAt}</small>
                      </div>
                      <Badge tone={statusTone(job.status)}>{generationStatusLabel(job.status)}</Badge>
                    </div>
                  ))}
                </div>
              )}
            </Card>
          )}
          {step === 4 && (
            <div className="preview-complete">
              <Badge tone="good">副本已创建</Badge>
              <h2>{customMode === "batch" ? "批量定制绘本已进入编辑状态" : "定制绘本已进入编辑状态"}</h2>
              <p>{customMode === "batch" ? "系统已为选中的儿童分别创建独立副本，你可以从第一本开始检查和导出。" : "你可以继续编辑页面正文和插图描述，再导出 PDF 或分享给家长。"}</p>
              {generatedTarget ? (
                <Link className="button primary" to={`/app/${workspace.id}/storybooks/${generatedTarget}`}>查看生成结果</Link>
              ) : (
                <button className="button primary" type="button" disabled title="需要先成功生成定制副本">等待副本生成完成</button>
              )}
            </div>
          )}
          <div className="wizard-actions">
            <button className="button secondary" disabled={step === 0} title={step === 0 ? "当前已经是第一步" : undefined} onClick={() => { setNotice(null); setStep((value) => Math.max(0, value - 1)); }}>上一步</button>
            <button
              className="button primary"
              disabled={step === steps.length - 1 || (step === 0 && (!canContinueSelection || childList.length === 0)) || generatingPlan || generating}
              title={step === 0 && childList.length === 0 ? "请先新增儿童资料" : step === 0 && !canContinueSelection ? "请至少选择一个儿童" : step === steps.length - 1 ? "副本已生成，请查看结果" : undefined}
              onClick={() => {
                if (step === 2) {
                  createCustomizationPlan();
                  return;
                }
                if (step === 3) {
                  createCustomCopy();
                  return;
                }
                nextStep();
              }}
            >{generatingPlan ? "正在生成方案..." : generating ? "正在生成..." : primaryLabels[step]}</button>
          </div>
        </Card>
      </div>
    </div>
  );
}

function generationErrorMessage(job: GenerationJob) {
  const output = job.output as { error?: { message?: string } } | undefined;
  return output?.error?.message || "生成任务失败，可稍后重试";
}

function generationStatusLabel(status: string) {
  return {
    queued: "排队中",
    running: "正在生成",
    succeeded: "已完成",
    failed: "生成失败",
  }[status] || `状态：${status}`;
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

function customizationPlanItems(output: unknown, child: ChildProfile, intensity: "quick" | "standard") {
  const value = output as {
    customization?: {
      intensity?: string;
      strategy?: string;
      rewrite_points?: { scope?: string; action?: string }[];
      risk_checks?: string[];
    };
  } | undefined;
  const customization = value?.customization;
  if (!customization) {
    return [
      `定制标题：《${child.nickname}学会一起玩》`,
      `主角会变成 ${child.nickname}`,
      `定制强度：${intensity === "standard" ? "标准定制" : "快速定制"}`,
      `加入元素：${child.interests[0] || "孩子兴趣"}、${child.interests[1] || "生活经验"}`,
      intensity === "standard" ? "改写并重绘：封面、第 1 页、第 4 页、结尾页" : "替换：称呼、标题和关键道具",
    ];
  }

  return [
    `主角会变成 ${child.nickname}`,
    `定制强度：${customization.intensity === "quick" ? "快速定制" : "标准定制"}`,
    customization.strategy ? `定制策略：${customization.strategy}` : null,
    ...(customization.rewrite_points || []).map((point) => `${point.scope || "内容"}：${point.action || "按孩子资料调整"}`),
    customization.risk_checks?.length ? `隐私检查：${customization.risk_checks.join("、")}` : null,
  ].filter(Boolean) as string[];
}

function averageCompleteness(children: ChildProfile[]) {
  if (children.length === 0) return 0;
  const total = children.reduce((sum, child) => sum + child.completeness, 0);
  return Math.round(total / children.length);
}

function batchCustomizationPlanItems(output: unknown, batchChildren: ChildProfile[], intensity: "quick" | "standard") {
  const value = output as {
    customization?: {
      intensity?: string;
      strategy?: string;
      rewrite_points?: { scope?: string; action?: string }[];
      risk_checks?: string[];
    };
  } | undefined;
  const customization = value?.customization;
  if (!customization) {
    return [
      `本次儿童：${batchChildren.length} 个`,
      `平均资料完整度：${averageCompleteness(batchChildren)}%`,
      `定制强度：${intensity === "standard" ? "标准定制" : "快速定制"}`,
      "生成方式：每个儿童一本独立副本，源普通绘本不被覆盖",
      intensity === "standard" ? "批量改写关键页面，并为每个儿童保留独立插图重绘空间" : "批量替换称呼、标题和关键道具，适合快速交付",
      ...batchChildren.slice(0, 4).map((child) => `${child.nickname}：${child.interests.slice(0, 2).join("、") || "暂无兴趣标签"} · ${child.focus}`),
    ];
  }

  return [
    `本次儿童：${batchChildren.length} 个`,
    `定制强度：${customization.intensity === "quick" ? "快速定制" : "标准定制"}`,
    customization.strategy ? `批量策略：${customization.strategy}` : null,
    ...(customization.rewrite_points || []).map((point) => `${point.scope || "内容"}：${point.action || "按儿童资料分别调整"}`),
    customization.risk_checks?.length ? `隐私检查：${customization.risk_checks.join("、")}` : null,
  ].filter(Boolean) as string[];
}

function ReviewBlock({ title, items, output }: { title: string; items: string[]; output?: unknown }) {
  const meta = output === undefined ? null : generationOutputMeta(output);
  return (
    <div className="review-block">
      {meta ? (
        <>
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
        </>
      ) : (
        <h2>{title}</h2>
      )}
      <div className="review-list">
        {items.map((item) => <div key={item}><span>确认项</span><strong>{item}</strong></div>)}
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
