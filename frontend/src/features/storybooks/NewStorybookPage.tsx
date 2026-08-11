import { useEffect, useRef, useState } from "react";
import { Link, useNavigate, useOutletContext, useSearchParams } from "react-router-dom";
import {
  createGenerationJob,
  createRoleReferenceImageTask,
  createStorybook,
  getStorybook,
  getWorkspaceGenerationProvider,
  listGenerationJobsPage,
  retryGenerationJob,
  updateStorybook,
  updateStorybookPage,
  updateStorybookRole,
  type GenerationJob,
  type GenerationProviderStatus,
} from "../../api/client";
import { ActionButton, Badge, Card, Notice } from "../../components/ui";
import { GenerationReviewBlock } from "../../components/GenerationReviewBlock";
import type { Storybook, StorybookRole, Workspace } from "../../types/domain";
import {
  generationErrorMessage,
  generationStatusLabel,
  isActiveJobStatus,
  pollGenerationJob,
} from "../../utils/generation";
import { generationJobTypeLabel } from "../../utils/labels";
import { PageEditor } from "./new/components/PageEditor";
import { PlanEditor } from "./new/components/PlanEditor";
import { PlanReviewSummary } from "./new/components/PlanReviewSummary";
import { RequestStepForm } from "./new/components/RequestStepForm";
import { RoleEditor } from "./new/components/RoleEditor";
import { WizardTopNav } from "./new/components/WizardTopNav";
import {
  generationInputFor,
  pageDraftsFromPlan,
  pageFromStorybook,
  pagesFromOutput,
  pagesFromStorybook,
  planDraftFromOutput,
  roleDraftsFromPlan,
  roleFromStorybook,
  rolesFromOutput,
  rolesFromStorybook,
} from "./new/helpers";
import { STORY_STYLE_PRESETS, STYLE_PRESETS } from "./new/presets";
import { storybookPageItems, storybookPlanItems, storybookRoleItems } from "./new/reviewItems";
import type { EditablePage, EditablePlan, EditableRole, StorybookRequestForm } from "./new/types";

const steps = ["需求", "绘本方案", "角色道具", "分页编辑", "预览导出"];

export function NewStorybookPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const navigate = useNavigate();
  const [step, setStep] = useState(0);
  const [unlockedStep, setUnlockedStep] = useState(0);
  const [notice, setNotice] = useState<{ title: string; copy: string } | null>(null);
  const [creating, setCreating] = useState(false);
  const [styleCardsExpanded, setStyleCardsExpanded] = useState(false);
  const [generatingStep, setGeneratingStep] = useState<string | null>(null);
  const [createdBookId, setCreatedBookId] = useState<string | null>(null);
  const [retryJob, setRetryJob] = useState<GenerationJob | null>(null);
  const [generationOutputs, setGenerationOutputs] = useState<Record<string, unknown>>({});
  const [provider, setProvider] = useState<GenerationProviderStatus | null>(null);
  const [editingReview, setEditingReview] = useState<null | "plan" | "roles" | "pages">(null);
  const [roleReferenceBusyId, setRoleReferenceBusyId] = useState<string | null>(null);
  const [roleVariantRefreshKey, setRoleVariantRefreshKey] = useState(0);
  const [requestDirtyAfterGeneration, setRequestDirtyAfterGeneration] = useState(false);
  const [planDraft, setPlanDraft] = useState<EditablePlan>({
    summary: "",
    outlineText: "",
    roleRequirementsText: "",
    reviewPointsText: "",
  });
  const [editableRoles, setEditableRoles] = useState<EditableRole[]>([]);
  const [editablePages, setEditablePages] = useState<EditablePage[]>([]);
  const [form, setForm] = useState<StorybookRequestForm>({
    title: "一起玩小汽车",
    theme: "学会分享和轮流",
    ageGroup: "4-5 岁",
    pageCount: "6",
    useScene: "规则引导",
    style: STYLE_PRESETS[0].value,
    pageAspectRatio: "portrait_4_5",
    storyStyle: STORY_STYLE_PRESETS[0].value,
    storyFramework: "",
  });
  const [searchParams, setSearchParams] = useSearchParams();
  const resumeBookId = searchParams.get("bookId");
  const resumedBookIdRef = useRef<string | null>(null);
  const suppressAutoRecoverRef = useRef(false);
  const targetBook = createdBookId;
  const generatedRoles = rolesFromOutput(generationOutputs.storybook_roles);
  const generatedPages = pagesFromOutput(generationOutputs.storybook_pages);
  const currentRoles = editableRoles.length ? editableRoles : generatedRoles;
  const currentPages = editablePages.length ? editablePages : generatedPages;
  const hasPlan = Boolean(generationOutputs.storybook_plan || planDraft.summary || planDraft.outlineText);
  const hasRoles = currentRoles.length > 0;
  const hasPages = currentPages.length > 0;
  const generatingStepLabel = {
    storybook_plan: "绘本方案生成中...",
    storybook_roles: "角色道具生成中...",
    storybook_pages: "分页图文生成中...",
  }[generatingStep || ""];
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
  const reviewForStep = (nextStep: number): null | "plan" | "roles" | "pages" => {
    if (nextStep === 2) return "roles";
    if (nextStep === 3) return "pages";
    return null;
  };
  const goToStep = (nextStep: number) => {
    setUnlockedStep((value) => Math.max(value, nextStep));
    setEditingReview(reviewForStep(nextStep));
    setStep(nextStep);
  };
  const rememberStorybookInUrl = (bookId: string) => {
    // 这是向导内部写入 URL，用 ref 标记后避免触发一次完整的“恢复进度”覆盖当前生成态。
    resumedBookIdRef.current = bookId;
    setSearchParams({ bookId }, { replace: true });
  };
  const updateRequestForm = (patch: Partial<StorybookRequestForm>) => {
    const hasGenerated = Boolean(
      createdBookId
      || generationOutputs.storybook_plan
      || editableRoles.length
      || editablePages.length
      || planDraft.summary
      || planDraft.outlineText,
    );
    setForm((current) => {
      const next = { ...current, ...patch };
      if (hasGenerated && Object.entries(patch).some(([key, value]) => current[key as keyof typeof current] !== value)) {
        setRequestDirtyAfterGeneration(true);
      }
      return next;
    });
  };
  const staleRecoveredJobNotice = (job: GenerationJob) => ({
    title: "上次生成任务已中断",
    copy: `检测到未完成的${generationJobTypeLabel[job.jobType] || "生成"}任务，但它是在服务器重启或页面离开前开始的，无法继续执行。请点击重新生成。任务编号：${job.id.slice(0, 8)}。`,
  });

  useEffect(() => {
    getWorkspaceGenerationProvider(workspace.id).then(setProvider).catch(() => setProvider(null));
  }, [workspace.id]);

  // 断线恢复：刷新后如果还有向导类生成任务在跑，恢复表单上下文并继续等待结果。
  useEffect(() => {
    if (resumeBookId) return;
    if (suppressAutoRecoverRef.current) return;
    let mounted = true;
    listGenerationJobsPage(workspace.id, { limit: 10 })
      .then((page) => {
        if (!mounted) return;
        const active = page.data.find((job) => (
          ["storybook_plan", "storybook_roles", "storybook_pages"].includes(job.jobType)
          && isActiveJobStatus(job.status)
        ));
        if (!active) {
          return;
        }
        const input = (active.input || {}) as Record<string, unknown>;
        setForm((current) => ({
          ...current,
          title: typeof input.title === "string" && input.title ? input.title : current.title,
          theme: typeof input.theme === "string" && input.theme ? input.theme : current.theme,
          ageGroup: typeof input.age_group === "string" && input.age_group ? input.age_group : current.ageGroup,
          pageCount: typeof input.page_count === "string" && input.page_count ? input.page_count : current.pageCount,
          useScene: typeof input.use_scene === "string" && input.use_scene ? input.use_scene : current.useScene,
          style: typeof input.style === "string" && input.style ? input.style : current.style,
          pageAspectRatio: input.page_aspect_ratio === "landscape_16_9" || input.page_aspect_ratio === "square_1_1" || input.page_aspect_ratio === "portrait_4_5" ? input.page_aspect_ratio : current.pageAspectRatio,
          storyStyle: typeof input.story_style === "string" && input.story_style ? input.story_style : current.storyStyle,
          storyFramework: typeof input.story_framework === "string" ? input.story_framework : current.storyFramework,
        }));
        if (active.storybookId) {
          setCreatedBookId(active.storybookId);
          rememberStorybookInUrl(active.storybookId);
        }
        if (active.status === "running" || active.lockedAt) {
          setRetryJob(active);
          setNotice(staleRecoveredJobNotice(active));
          return;
        }
        setNotice({
          title: "已恢复排队中的生成任务",
          copy: `检测到未完成的${generationJobTypeLabel[active.jobType] || "生成"}任务，任务仍在队列中；如果长时间没有变化，请重新生成。任务编号：${active.id.slice(0, 8)}。`,
        });
        setGeneratingStep(active.jobType);
        waitForGenerationJob(active)
          .then((settled) => { if (mounted) void handleGenerationJob(settled, "生成任务已完成"); })
          .catch(() => {
            if (mounted) {
              setNotice({
                title: "原生成任务已失效",
                copy: "未完成的任务已不存在或无法读取，请直接重新生成。",
              });
            }
          })
          .finally(() => { if (mounted) setGeneratingStep(null); });
      })
      .catch(() => undefined);
    return () => {
      mounted = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspace.id, resumeBookId]);

  // 从工作台「继续编辑」带 bookId 进入：恢复向导进度，
  // 载入绘本信息和该绘本最近一次成功的方案/角色/分页产物，跳到对应步骤。
  useEffect(() => {
    if (!resumeBookId || resumedBookIdRef.current === resumeBookId) return;
    resumedBookIdRef.current = resumeBookId;
    let mounted = true;
    void (async () => {
      try {
        const book = await getStorybook(workspace.id, resumeBookId);
        if (!mounted) return;
        setCreatedBookId(book.id);
        setGenerationOutputs({});
        setEditableRoles([]);
        setEditablePages([]);
        setPlanDraft({ summary: "", outlineText: "", roleRequirementsText: "", reviewPointsText: "" });
        const jobsPage = await listGenerationJobsPage(workspace.id, { storybookId: book.id, limit: 50 });
        if (!mounted) return;
        const wizardJobs = [...jobsPage.data]
          .filter((job) => (
            job.storybookId === book.id
            && ["storybook_plan", "storybook_roles", "storybook_pages"].includes(job.jobType)
          ))
          .sort((a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt));
        const latestByType = new Map<string, GenerationJob>();
        wizardJobs.forEach((job) => {
            if (job.status !== "succeeded" || !job.output) return;
            if (!latestByType.has(job.jobType)) latestByType.set(job.jobType, job);
          });
        const planJob = latestByType.get("storybook_plan");
        const rolesJob = latestByType.get("storybook_roles");
        const pagesJob = latestByType.get("storybook_pages");
        const activeJob = wizardJobs.find((job) => {
          if (!isActiveJobStatus(job.status)) return false;
          const latestSucceededJob = latestByType.get(job.jobType);
          return !latestSucceededJob || Date.parse(job.createdAt) > Date.parse(latestSucceededJob.createdAt);
        });
        // 需求表单以最近一次方案任务的 input 为准（含画风、故事风格、故事框架、页数），
        // 绘本记录兜底，保证刷新后需求页恢复原填内容而不是默认值。
        const planInput = ((planJob?.input || {}) as Record<string, unknown>);
        setForm((current) => ({
          ...current,
          title: typeof planInput.title === "string" && planInput.title ? planInput.title : book.title || current.title,
          theme: typeof planInput.theme === "string" && planInput.theme ? planInput.theme : book.teachingGoal || current.theme,
          ageGroup: typeof planInput.age_group === "string" && planInput.age_group ? planInput.age_group : book.ageGroup || current.ageGroup,
          pageCount: typeof planInput.page_count === "string" && planInput.page_count ? planInput.page_count : current.pageCount,
          useScene: typeof planInput.use_scene === "string" && planInput.use_scene ? planInput.use_scene : book.useScene || current.useScene,
          style: typeof planInput.style === "string" && planInput.style ? planInput.style : book.coverTone || current.style,
          pageAspectRatio: planInput.page_aspect_ratio === "landscape_16_9" || planInput.page_aspect_ratio === "square_1_1" || planInput.page_aspect_ratio === "portrait_4_5" ? planInput.page_aspect_ratio : book.pageAspectRatio || current.pageAspectRatio,
          storyStyle: typeof planInput.story_style === "string" ? planInput.story_style : current.storyStyle,
          storyFramework: typeof planInput.story_framework === "string" ? planInput.story_framework : current.storyFramework,
        }));
        const outputs: Record<string, unknown> = {};
        if (planJob?.output) {
          outputs.storybook_plan = planJob.output;
          setPlanDraft(planDraftFromOutput(planJob.output, { title: book.title, theme: book.teachingGoal }));
        }
        if (rolesJob?.output) outputs.storybook_roles = rolesJob.output;
        if (pagesJob?.output) outputs.storybook_pages = pagesJob.output;
        setGenerationOutputs(outputs);
        // 已写入绘本的角色/分页以绘本为准；否则回退到任务输出。
        if (book.roles.length) setEditableRoles(rolesFromStorybook(book.roles));
        if (pagesJob && book.pages.length) setEditablePages(pagesFromStorybook(book.pages));
        setRequestDirtyAfterGeneration(false);
        const hasPlanOutput = Boolean(planJob?.output);
        const hasRolesOutput = Boolean(rolesJob?.output);
        const hasPagesOutput = Boolean(pagesJob?.output);
        const restoredStep = hasPagesOutput || ["editing", "image_pending", "exportable", "submitted", "listed"].includes(book.status)
          ? 3
          : hasRolesOutput || book.status === "roles_pending"
            ? 2
            : hasPlanOutput
              ? 1
              : 0;
        goToStep(restoredStep);
        setEditingReview(
          restoredStep === 1
            ? null
            : restoredStep === 2
              ? "roles"
              : restoredStep === 3 && !["exportable", "submitted", "listed"].includes(book.status)
                ? "pages"
                : null,
        );
        setNotice({
          title: "已恢复向导进度",
          copy: hasPagesOutput || ["editing", "image_pending", "exportable", "submitted", "listed"].includes(book.status)
            ? "已载入上次的方案、角色和分页，可继续确认分页。"
            : hasRolesOutput || book.status === "roles_pending"
              ? "已载入上次的方案和角色，可继续确认角色并生成分页。"
              : hasPlanOutput
                ? "已载入上次确认的绘本方案，可继续生成角色与道具。"
                : "这本绘本还没有生成记录，请从需求开始。",
        });
        if (activeJob) {
          const activeInput = (activeJob.input || {}) as Record<string, unknown>;
          setForm((current) => ({
            ...current,
            title: typeof activeInput.title === "string" && activeInput.title ? activeInput.title : current.title,
            theme: typeof activeInput.theme === "string" && activeInput.theme ? activeInput.theme : current.theme,
            ageGroup: typeof activeInput.age_group === "string" && activeInput.age_group ? activeInput.age_group : current.ageGroup,
            pageCount: typeof activeInput.page_count === "string" && activeInput.page_count ? activeInput.page_count : current.pageCount,
            useScene: typeof activeInput.use_scene === "string" && activeInput.use_scene ? activeInput.use_scene : current.useScene,
            style: typeof activeInput.style === "string" && activeInput.style ? activeInput.style : current.style,
            pageAspectRatio: activeInput.page_aspect_ratio === "landscape_16_9" || activeInput.page_aspect_ratio === "square_1_1" || activeInput.page_aspect_ratio === "portrait_4_5" ? activeInput.page_aspect_ratio : current.pageAspectRatio,
            storyStyle: typeof activeInput.story_style === "string" ? activeInput.story_style : current.storyStyle,
            storyFramework: typeof activeInput.story_framework === "string" ? activeInput.story_framework : current.storyFramework,
          }));
          const activeStep = activeJob.jobType === "storybook_pages" ? 2 : activeJob.jobType === "storybook_roles" ? 1 : 0;
          goToStep(Math.max(restoredStep, activeStep));
          if (activeJob.status === "running" || activeJob.lockedAt) {
            setRetryJob(activeJob);
            setNotice(staleRecoveredJobNotice(activeJob));
            return;
          }
          setNotice({
            title: "已恢复排队中的生成任务",
            copy: `检测到未完成的${generationJobTypeLabel[activeJob.jobType] || "生成"}任务，任务仍在队列中；如果长时间没有变化，请重新生成。任务编号：${activeJob.id.slice(0, 8)}。`,
          });
          setGeneratingStep(activeJob.jobType);
          waitForGenerationJob(activeJob)
            .then(async (settled) => {
              if (!mounted) return;
              const ok = await handleGenerationJob(settled, "生成任务已完成");
              if (!ok) return;
              if (settled.jobType === "storybook_plan") goToStep(1);
              if (settled.jobType === "storybook_roles") goToStep(2);
              if (settled.jobType === "storybook_pages") goToStep(3);
            })
            .catch(() => {
              if (mounted) {
                setNotice({
                  title: "原生成任务已失效",
                  copy: "未完成的任务已不存在或无法读取，请直接重新生成。",
                });
              }
            })
            .finally(() => { if (mounted) setGeneratingStep(null); });
        }
      } catch {
        if (mounted) {
          resumedBookIdRef.current = null;
          setNotice({ title: "恢复向导失败", copy: "无法读取该绘本的向导进度，请重新生成。" });
        }
      }
    })();
    return () => {
      mounted = false;
      // React StrictMode 下 effect 会挂载两次：第一次的异步恢复被清理作废，
      // 必须放行第二次执行，否则恢复永远不会落地（表现为回到需求页、表单是默认值）。
      resumedBookIdRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspace.id, resumeBookId]);
  // 向导中创建绘本后把 bookId 写入地址栏：刷新页面会走上面的恢复逻辑，进度不丢失。
  useEffect(() => {
    if (!createdBookId || resumeBookId === createdBookId) return;
    if (suppressAutoRecoverRef.current) return;
    // 先把 ref 占住，避免本次写 URL 又触发一次完整恢复、覆盖正在编辑的内容。
    resumedBookIdRef.current = createdBookId;
    suppressAutoRecoverRef.current = false;
    setSearchParams({ bookId: createdBookId }, { replace: true });
  }, [createdBookId, requestDirtyAfterGeneration, resumeBookId, setSearchParams]);
  const clearGeneratedProgress = () => {
    setGenerationOutputs({});
    setPlanDraft({ summary: "", outlineText: "", roleRequirementsText: "", reviewPointsText: "" });
    setEditableRoles([]);
    setEditablePages([]);
    setCreatedBookId(null);
    setUnlockedStep(0);
    setEditingReview(null);
    setRequestDirtyAfterGeneration(false);
    setRetryJob(null);
    resumedBookIdRef.current = null;
    suppressAutoRecoverRef.current = true;
    if (searchParams.get("bookId")) setSearchParams({}, { replace: true });
  };
  const ensureStorybookCreated = async (options: { forceNew?: boolean } = {}) => {
    if (createdBookId && !options.forceNew) return createdBookId;
    setCreating(true);
    try {
      const book = await createStorybook(workspace.id, {
        title: form.title.trim() || form.theme.trim() || "新建普通绘本",
        ageGroup: form.ageGroup,
        useScene: form.useScene,
        teachingGoal: form.theme.trim() || "帮助孩子理解班级规则和生活习惯",
        coverTone: form.style.trim(),
        pageAspectRatio: form.pageAspectRatio,
      });
      suppressAutoRecoverRef.current = false;
      setCreatedBookId(book.id);
      rememberStorybookInUrl(book.id);
      return book.id;
    } finally {
      setCreating(false);
    }
  };
  const runGeneration = async (
    jobType: string,
    title: string,
    overrides?: { plan?: EditablePlan; roles?: EditableRole[] },
    options: { forceNewStorybook?: boolean } = {},
  ): Promise<GenerationJob | null> => {
    setGeneratingStep(jobType);
    setRetryJob(null);
    setNotice(null);
    try {
      // 每个向导生成任务都必须绑定到绘本草稿。
      // 否则第一步方案生成只存在前端内存里，刷新页面后无法从后端恢复。
      const bookId = await ensureStorybookCreated({ forceNew: options.forceNewStorybook });
      const job = await createGenerationJob(workspace.id, {
        jobType,
        storybookId: bookId || undefined,
        input: generationInputFor(jobType, form, overrides?.plan ?? planDraft, overrides?.roles ?? currentRoles, currentPages),
      });
      const settledJob = await waitForGenerationJob(job);
      const ok = await handleGenerationJob(settledJob, title);
      return ok ? settledJob : null;
    } catch (err) {
      setRetryJob(null);
      setNotice({ title: "生成失败", copy: err instanceof Error ? err.message : "请稍后重试" });
      return null;
    } finally {
      setGeneratingStep(null);
    }
  };

  // 方案重新生成后，下游的角色和分页仍是旧方案的产物，必须按新方案联动重生；
  // 直接用新任务的输出作为下一步输入，避免闭包里旧的 planDraft/currentRoles。
  const regeneratePlanWithCascade = async () => {
    const hadRoles = Boolean(generationOutputs.storybook_roles) || currentRoles.length > 0;
    const hadPages = Boolean(generationOutputs.storybook_pages) || currentPages.length > 0;
    const planJob = await runGeneration("storybook_plan", "已重新生成方案");
    if (!planJob?.output || !hadRoles) return;
    const freshPlan = planDraftFromOutput(planJob.output, form);
    const rolesJob = await runGeneration("storybook_roles", "角色与道具已按新方案联动更新", { plan: freshPlan });
    if (!rolesJob?.output || !hadPages) return;
    const freshRoles = rolesFromOutput(rolesJob.output);
    await runGeneration("storybook_pages", "分页已按新方案联动更新", {
      plan: freshPlan,
      roles: freshRoles.length ? freshRoles : currentRoles,
    });
  };

  const waitForGenerationJob = (initialJob: GenerationJob) =>
    pollGenerationJob(workspace.id, initialJob, { timeoutMs: 240_000 });
  const dismissRecoveredGenerationNotice = () => {
    setRetryJob(null);
    setNotice(null);
  };
  const retryFailedGeneration = async () => {
    if (!retryJob) return;
    setGeneratingStep(retryJob.jobType);
    setNotice(null);
    try {
      const settledJob = retryJob.status === "failed"
        ? await retryGenerationJob(workspace.id, retryJob.id)
          .then(waitForGenerationJob)
          .then(async (job) => (await handleGenerationJob(job, "已重新生成") ? job : null))
        : await runGeneration(retryJob.jobType, "已重新生成");
      if (settledJob?.jobType === "storybook_plan") goToStep(1);
      if (settledJob?.jobType === "storybook_roles") goToStep(2);
      if (settledJob?.jobType === "storybook_pages") goToStep(3);
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
        setRequestDirtyAfterGeneration(false);
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
      coverTone: form.style.trim(),
      pageAspectRatio: form.pageAspectRatio,
    });
  };
  const generateRoleReference = async (role: EditableRole, roleIndex: number) => {
    const bookId = createdBookId || resumeBookId || await ensureStorybookCreated();
    if (!bookId) {
      setNotice({
        title: "暂不能生成参考图",
        copy: "请先创建绘本草稿，再为需要跨页一致的角色生成参考图。",
      });
      return;
    }
    if (!role.needsConsistency) {
      setNotice({
        title: "这个角色无需参考图",
        copy: "只有主角、老师、反复出现的同伴或关键道具才需要单独生成参考图。",
      });
      return;
    }
    setRoleReferenceBusyId(roleReferenceBusyKey(role, roleIndex));
    setNotice(null);
    try {
      await persistStorybookMeta(bookId);
      if (currentRoles.length) {
        await persistRoles(bookId, currentRoles);
      }
      let savedBook = await getStorybook(workspace.id, bookId);
      let savedRole = findSavedRoleForEditable(savedBook.roles, role, roleIndex);
      if (!savedRole) {
        setNotice({
          title: "正在生成角色道具",
          copy: "当前角色还没有写入绘本，正在先生成并保存角色道具，完成后会继续生成参考图。",
        });
        const rolesJob = await runGeneration("storybook_roles", "角色与道具已生成并写入绘本");
        if (!rolesJob?.output) return;
        savedBook = await getStorybook(workspace.id, bookId);
        savedRole = findSavedRoleForEditable(savedBook.roles, role, roleIndex);
      }
      if (!savedRole) throw new Error("没有找到已保存的角色，请重新生成角色与道具后再试。");
      setRoleReferenceBusyId(roleReferenceBusyKey(roleFromStorybook(savedRole), roleIndex));
      const job = await createRoleReferenceImageTask(workspace.id, bookId, savedRole.id, {
        referenceImageUrls: [],
        imageMode: "text_to_image",
      });
      setEditableRoles((roles) => roles.map((item) => (
        item.id === savedRole.id ? { ...item, referenceStatus: "generating" } : item
      )));
      const settledJob = await waitForGenerationJob(job);
      const ok = await handleGenerationJob(settledJob, savedRole.referenceImageUrl ? "角色参考图已重新生成" : "角色参考图已生成");
      const refreshed = await getStorybook(workspace.id, bookId);
      setEditableRoles(rolesFromStorybook(refreshed.roles));
      setRoleVariantRefreshKey((value) => value + 1);
      if (!ok) return;
      setNotice({
        title: "角色参考图已生成",
        copy: `「${savedRole.name}」的参考图已写入绘本，后续分页插图会用它保持跨页形象一致。`,
      });
    } catch (err) {
      setNotice({
        title: "角色参考图生成失败",
        copy: err instanceof Error ? err.message : "请稍后重试。",
      });
    } finally {
      setRoleReferenceBusyId(null);
    }
  };
  // 分页生成后自动为跨页出现的角色排队生成参考图，避免故事里只有角色文字描述、没有角色图片。
  const autoGenerateRoleReferences = async (bookId: string) => {
    let book: Storybook;
    let recentJobs: GenerationJob[] = [];
    try {
      book = await getStorybook(workspace.id, bookId);
      recentJobs = await listGenerationJobsPage(workspace.id, { storybookId: bookId, limit: 50 })
        .then((page) => page.data)
        .catch(() => [] as GenerationJob[]);
    } catch {
      return;
    }
    const activeRoleIds = new Set(
      recentJobs
        .filter((job) => job.jobType === "storybook_role_reference_image" && isActiveJobStatus(job.status))
        .map((job) => ((job.input || {}) as { role_id?: unknown }).role_id)
        .filter((value): value is string => typeof value === "string"),
    );
    const pendingRoles = book.roles.filter((role) => {
      if (!role.needsConsistency) return false;
      if (role.referenceStatus === "ready" && role.referenceImageUrl) return false;
      if (activeRoleIds.has(role.id)) return false;
      const usage = book.pages.filter((page) => `${page.title} ${page.body} ${page.illustrationPrompt}`.includes(role.name)).length;
      return usage >= 2;
    });
    if (!pendingRoles.length) return;
    let queued = 0;
    let failed = 0;
    for (const role of pendingRoles) {
      try {
        // 不传 prompt，后端会根据角色最新的名称、类型和外观设定组合标准参考图提示词。
        await createRoleReferenceImageTask(workspace.id, bookId, role.id, {
          referenceImageUrls: [],
          imageMode: "text_to_image",
        });
        queued += 1;
      } catch {
        failed += 1;
      }
    }
    if (queued > 0) {
      setNotice({
        title: "角色参考图已自动加入生成队列",
        copy: `分页已生成，同时为 ${queued} 个跨页角色自动排队生成参考图${failed ? `（${failed} 个入队失败，可在详情页手动重试）` : ""}。进入详情页可查看进度，参考图会让后续插图保持同一形象。`,
      });
    } else if (failed > 0) {
      setNotice({
        title: "角色参考图自动入队失败",
        copy: "分页已生成，但角色参考图未能自动加入生成队列，可在详情页角色管理中手动生成。",
      });
    }
  };
  const handlePrimary = async () => {
    setNotice(null);
    if (step === 0) {
      const shouldResetGenerated = requestDirtyAfterGeneration && Boolean(
        createdBookId
        || generationOutputs.storybook_plan
        || editableRoles.length
        || editablePages.length
        || planDraft.summary
        || planDraft.outlineText,
      );
      if (shouldResetGenerated) {
        if (!window.confirm("重新生成方案会清空已生成的方案、角色和分页内容，确定继续吗？")) {
          return;
        }
        clearGeneratedProgress();
      }
      if (await runGeneration("storybook_plan", "绘本方案已生成", undefined, { forceNewStorybook: shouldResetGenerated })) {
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
      try {
        await persistStorybookMeta(createdBookId);
      } catch (err) {
        setNotice({ title: "保存绘本信息失败", copy: err instanceof Error ? err.message : "请稍后重试" });
        return;
      }
    }
    if (step === 2) {
      const bookId = await ensureStorybookCreated();
      if (!hasRoles) {
        await runGeneration("storybook_roles", "角色与道具已生成并写入绘本");
        setEditingReview("roles");
        return;
      }
      try {
        if (bookId) {
          await persistRoles(bookId, currentRoles);
        }
        if (await runGeneration("storybook_pages", "分页图文已生成并写入绘本")) {
          if (bookId) {
            await autoGenerateRoleReferences(bookId);
          }
          goToStep(3);
          setEditingReview("pages");
        }
      } catch (err) {
        setNotice({ title: "保存角色失败", copy: err instanceof Error ? err.message : "请稍后重试" });
      }
      return;
    }
    if (step === 3) {
      if (!hasPages) {
        if (await runGeneration("storybook_pages", "分页图文已生成并写入绘本")) {
          if (createdBookId) {
            await autoGenerateRoleReferences(createdBookId);
          }
          setEditingReview("pages");
        }
        return;
      }
      try {
        const bookId = createdBookId;
        if (bookId) {
          await persistPages(bookId);
          // 向导完成只推进到 editing，交付需在详情页完成老师复核后再标记可交付。
          await updateStorybook(workspace.id, bookId, { status: "editing" });
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
      <header className="wizard-header">
        <h1>新建普通绘本</h1>
        <span>创建在 {workspace.name}</span>
      </header>
      {provider && !provider.realTextReady && (
        <Notice
          title="真实文本生成暂不可用"
          copy={`${provider.diagnostic}${provider.missingConfiguration.length ? ` 缺少：${provider.missingConfiguration.join("、")}` : ""}`}
          tone="warn"
        />
      )}
      <WizardTopNav
        steps={steps}
        active={step}
        maxUnlockedStep={unlockedStep}
        disabled={Boolean(generatingStep)}
        status={generatingStepLabel || "生成完成后可继续编辑"}
        onSelect={(next) => { if (!generatingStep) goToStep(next); }}
      />
      <div className="wizard-shell wizard-shell-single">
        <Card className="wizard-card">
          {notice && (
            <Notice
              title={notice.title}
              copy={notice.copy}
              tone={retryJob ? "danger" : "info"}
              action={retryJob ? (
                <div className="inline-actions">
                  <button className="button secondary" type="button" disabled={generatingStep === retryJob.jobType} onClick={retryFailedGeneration}>重新生成</button>
                  {isActiveJobStatus(retryJob.status) && (
                    <button className="button ghost" type="button" disabled={Boolean(generatingStep)} onClick={dismissRecoveredGenerationNotice}>忽略</button>
                  )}
                </div>
              ) : undefined}
            />
          )}
          {step === 0 && (
            <RequestStepForm
              form={form}
              disabled={Boolean(generatingStep)}
              styleCardsExpanded={styleCardsExpanded}
              onChange={updateRequestForm}
              onToggleStyleCards={() => setStyleCardsExpanded((value) => !value)}
            />
          )}
          {step === 1 && <GenerationReviewBlock showMeta title="绘本方案" output={generationOutputs.storybook_plan} items={storybookPlanItems(generationOutputs.storybook_plan, form, planDraft)} regenerating={generatingStep === "storybook_plan"} onRegenerate={() => void regeneratePlanWithCascade()} onEdit={() => setEditingReview(editingReview === "plan" ? null : "plan")} editing={editingReview === "plan"} reviewContent={<PlanReviewSummary form={form} plan={planDraft} />} editor={<><PlanEditor form={form} plan={planDraft} onFormChange={setForm} onPlanChange={setPlanDraft} /><p className="form-hint">修改在重新生成时生效；离开页面前请点「重新生成」。</p></>} />}
          {step === 2 && <GenerationReviewBlock showMeta title="角色与关键道具" output={generationOutputs.storybook_roles} items={storybookRoleItems(generationOutputs.storybook_roles, currentRoles, planDraft, form)} regenerating={generatingStep === "storybook_roles"} onRegenerate={() => runGeneration("storybook_roles", "已重新生成角色")} onEdit={() => setEditingReview(editingReview === "roles" ? null : "roles")} editing={editingReview === "roles"} editor={<><RoleEditor workspaceId={workspace.id} storybookId={createdBookId || resumeBookId || undefined} roles={currentRoles.length ? currentRoles : roleDraftsFromPlan(planDraft, form)} onChange={setEditableRoles} onGenerateReference={generateRoleReference} onRolesRefresh={setEditableRoles} roleReferenceBusyId={roleReferenceBusyId} variantRefreshKey={roleVariantRefreshKey} /><p className="form-hint">修改会先保存到绘本；需要跨页一致的角色可在本页生成参考图。</p></>} />}
          {step === 3 && <GenerationReviewBlock showMeta title="分页图文" output={generationOutputs.storybook_pages} items={storybookPageItems(generationOutputs.storybook_pages, currentPages, planDraft, form)} regenerating={generatingStep === "storybook_pages"} onRegenerate={() => runGeneration("storybook_pages", "已重新生成分页")} onEdit={() => setEditingReview(editingReview === "pages" ? null : "pages")} editing={editingReview === "pages"} editor={<><PageEditor pages={currentPages.length ? currentPages : pageDraftsFromPlan(planDraft, form)} onChange={setEditablePages} roles={currentRoles} /><p className="form-hint">修改在重新生成时生效；离开页面前请点「重新生成」。</p></>} />}
          {step === 4 && (
            <div className="preview-complete">
              <Badge tone="info">编辑中</Badge>
              <h2>《{form.title || "一起玩小汽车"}》分页已就绪</h2>
              <p>请进入详情页生成插图、完成老师复核，再标记可交付后导出 PDF 或派生定制绘本。</p>
              {targetBook ? (
                <Link className="button primary" to={`/app/${workspace.id}/storybooks/${targetBook}`}>进入绘本详情</Link>
              ) : (
                <ActionButton className="button primary" disabled disabledHint="需要先成功创建绘本">等待绘本创建完成</ActionButton>
              )}
            </div>
          )}
          <div className="wizard-actions">
            <ActionButton className="button secondary" disabled={step === 0 || Boolean(generatingStep)} disabledHint={step === 0 ? "当前已经是第一步" : "生成进行中，请稍候"} onClick={() => { setNotice(null); goToStep(Math.max(0, step - 1)); }}>上一步</ActionButton>
            <ActionButton className="button primary" disabled={step === steps.length - 1 || creating || Boolean(generatingStep)} disabledHint={step === steps.length - 1 ? "绘本已生成，请进入详情继续编辑或导出" : "生成进行中，请稍候"} onClick={handlePrimary}>{creating ? "正在创建..." : generatingStep ? "生成中..." : primaryLabels[step]}</ActionButton>
          </div>
        </Card>
      </div>
    </div>
  );
}

function roleReferenceBusyKey(role: EditableRole, index: number) {
  return role.id || `${index}:${role.roleType}:${role.name || "未命名角色"}`;
}

function findSavedRoleForEditable(savedRoles: StorybookRole[], role: EditableRole, roleIndex: number) {
  if (role.id) {
    const byId = savedRoles.find((item) => item.id === role.id);
    if (byId) return byId;
  }
  const byName = savedRoles.find((item) => item.name === role.name);
  if (byName) return byName;
  const byType = savedRoles.filter((item) => item.roleType === role.roleType);
  if (byType.length) return byType[Math.min(roleIndex, byType.length - 1)] || byType[0];
  return savedRoles.filter((item) => item.needsConsistency)[Math.min(roleIndex, savedRoles.length - 1)] || savedRoles[0];
}
