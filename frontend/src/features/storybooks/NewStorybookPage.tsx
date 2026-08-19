import { useEffect, useRef, useState } from "react";
import { useNavigate, useOutletContext, useSearchParams } from "react-router-dom";
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
import { PlanReviewSummary } from "./new/components/PlanReviewSummary";
import { RequestStepForm } from "./new/components/RequestStepForm";
import { RoleEditor } from "./new/components/RoleEditor";
import { StorybookGenerationProgress, type GenerationPhase } from "./new/components/StorybookGenerationProgress";
import { WizardTopNav } from "./new/components/WizardTopNav";
import {
  generationInputFor,
  pageFromStorybook,
  pagesFromOutput,
  pagesFromStorybook,
  planDraftFromOutput,
  roleFromStorybook,
  rolesFromOutput,
  rolesFromStorybook,
} from "./new/helpers";
import { STORY_STYLE_PRESETS, STYLE_PRESETS } from "./new/presets";
import { storybookPlanItems } from "./new/reviewItems";
import type { EditablePage, EditablePlan, EditableRole, StorybookRequestForm } from "./new/types";

const steps = ["想法", "方向", "大纲", "生成"];

type StoryDirection = {
  id: string;
  title: string;
  summary: string;
  fitReason: string;
  personalHook: string;
  materialLabels: string[];
};
type StoredCreationSession = {
  workspaceId: string;
  step: number;
  unlockedStep: number;
  form: StorybookRequestForm;
  planDraft: EditablePlan;
  selectedDirectionId: string | null;
  directionBatch: number;
  customMaterials: string[];
  visualComplexity: string;
  characterConsistency: string;
  useSceneExplicit: boolean;
  createdBookId: string | null;
  updatedAt: string;
};

const storyStartExamples = [
  {
    title: "孩子成长",
    copy: "帮孩子理解分享、等待、情绪表达",
    value: "给一个 4 岁孩子做一本关于分享和轮流的温柔故事。",
    defaults: { title: "成长小练习", theme: "理解分享、等待和情绪表达", useScene: "规则引导" },
  },
  {
    title: "班级教育",
    copy: "做一本文明排队、午睡、活动规则故事",
    value: "给一个幼儿园班级做一本关于排队等待的规则故事。",
    defaults: { title: "班级规则故事", theme: "理解并练习班级规则", useScene: "规则引导" },
  },
  {
    title: "生日纪念",
    copy: "记录一个人的成长、回忆和祝福",
    value: "给一个孩子做一本生日纪念故事，记录成长和祝福。",
    defaults: { title: "特别的生日故事", theme: "记录成长、回忆和祝福", useScene: "家园沟通" },
  },
  {
    title: "课程故事",
    copy: "为课程主题生成一个配套故事",
    value: "做一本适合课堂导入的环保主题故事。",
    defaults: { title: "课程主题故事", theme: "围绕课程主题展开共读", useScene: "课堂共读" },
  },
  {
    title: "自由创作",
    copy: "直接描述你想要的故事",
    value: "我想自由创作一本温暖、有想象力的故事。",
    defaults: { title: "", theme: "", useScene: "" },
  },
];
const defaultStorybookRequestForm: StorybookRequestForm = {
  title: "",
  theme: "",
  ageGroup: "4-5 岁",
  pageCount: "6",
  useScene: "",
  style: STYLE_PRESETS[0].value,
  pageAspectRatio: "portrait_4_5",
  storyStyle: STORY_STYLE_PRESETS[0].value,
  storyFramework: "",
  quickIdea: "",
  visualComplexity: "simple",
  characterConsistency: "auto",
};
const defaultPlanDraft: EditablePlan = {
  summary: "",
  outlineText: "",
  roleRequirementsText: "",
  reviewPointsText: "",
};

export function NewStorybookPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const navigate = useNavigate();
  const [step, setStep] = useState(0);
  const [unlockedStep, setUnlockedStep] = useState(0);
  const [notice, setNotice] = useState<{ title: string; copy: string } | null>(null);
  const [creating, setCreating] = useState(false);
  const [styleCardsExpanded, setStyleCardsExpanded] = useState(false);
  const [customStyleOpen, setCustomStyleOpen] = useState(false);
  const [generatingStep, setGeneratingStep] = useState<string | null>(null);
  const [generationPhase, setGenerationPhase] = useState<GenerationPhase>("idle");
  const [fullDraftGenerating, setFullDraftGenerating] = useState(false);
  const [createdBookId, setCreatedBookId] = useState<string | null>(null);
  const [retryJob, setRetryJob] = useState<GenerationJob | null>(null);
  const [generationOutputs, setGenerationOutputs] = useState<Record<string, unknown>>({});
  const [provider, setProvider] = useState<GenerationProviderStatus | null>(null);
  const [editingReview, setEditingReview] = useState<null | "plan" | "roles" | "pages">(null);
  const [roleReferenceBusyId, setRoleReferenceBusyId] = useState<string | null>(null);
  const [roleVariantRefreshKey, setRoleVariantRefreshKey] = useState(0);
  const [requestDirtyAfterGeneration, setRequestDirtyAfterGeneration] = useState(false);
  const [selectedDirectionId, setSelectedDirectionId] = useState<string | null>(null);
  const [imagePreferenceOpen, setImagePreferenceOpen] = useState(false);
  const [directionSupplement, setDirectionSupplement] = useState("");
  const [customMaterials, setCustomMaterials] = useState<string[]>([]);
  const [outlineAdjustPage, setOutlineAdjustPage] = useState<number | null>(null);
  const [visualComplexity, setVisualComplexity] = useState("simple");
  const [characterConsistency, setCharacterConsistency] = useState("auto");
  const [useSceneExplicit, setUseSceneExplicit] = useState(false);
  const [directionBatch, setDirectionBatch] = useState(0);
  const [sessionHydrated, setSessionHydrated] = useState(false);
  const [restoredSessionAt, setRestoredSessionAt] = useState<string | null>(null);
  const [planDraft, setPlanDraft] = useState<EditablePlan>(defaultPlanDraft);
  const [editableRoles, setEditableRoles] = useState<EditableRole[]>([]);
  const [editablePages, setEditablePages] = useState<EditablePage[]>([]);
  const [form, setForm] = useState<StorybookRequestForm>(defaultStorybookRequestForm);
  const [searchParams, setSearchParams] = useSearchParams();
  const resumeBookId = searchParams.get("bookId");
  const resumedBookIdRef = useRef<string | null>(null);
  const creationSessionStorageKey = `kindleaf.creation-session.${workspace.id}`;
  const suppressAutoRecoverRef = useRef(false);
  // A new generation must be isolated from every previous poll/recovery callback.
  // React state alone cannot do this because an older async request can settle later.
  const generationRunRef = useRef(0);
  const beginGenerationRun = () => {
    generationRunRef.current += 1;
    return generationRunRef.current;
  };
  const isCurrentGenerationRun = (runId: number) => generationRunRef.current === runId;
  const generatedRoles = rolesFromOutput(generationOutputs.storybook_roles);
  const generatedPages = pagesFromOutput(generationOutputs.storybook_pages);
  const currentRoles = editableRoles.length ? editableRoles : generatedRoles;
  const currentPages = editablePages.length ? editablePages : generatedPages;
  const hasPlan = Boolean(generationOutputs.storybook_plan || planDraft.summary || planDraft.outlineText);
  const materials = materialLabelsFor(form, planDraft, customMaterials);
  const storyDirections = directionsFor(form, planDraft, materials, directionBatch);
  const selectedDirection = storyDirections.find((direction) => direction.id === selectedDirectionId) || null;
  const outlineItems = outlineItemsFor(planDraft, form);
  const intentReady = isStoryIdeaReady(form.quickIdea);
  const effectiveForm = { ...form, personalMaterials: customMaterials, visualComplexity, characterConsistency };
  const visualSummary = visualPreferenceSummary(form, visualComplexity, characterConsistency);
  const primaryLabels = [
    "开始生成方向",
    selectedDirection ? "按这个方向继续" : "选择一个方向",
    "就按这个生成",
    retryJob ? "重试生成" : "开始生成整本绘本",
  ];
  const flowBusy = fullDraftGenerating || Boolean(generatingStep) || isBlockingGenerationPhase(generationPhase);
  const showNotice = (title: string, copy: string) => {
    setRetryJob(null);
    setNotice({ title, copy });
  };
  const goToStep = (nextStep: number) => {
    setUnlockedStep((value) => Math.max(value, nextStep));
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
    const directionSensitiveFields: Array<keyof StorybookRequestForm> = ["quickIdea", "title", "theme", "ageGroup", "pageCount", "useScene"];
    const changedEntries = Object.entries(patch).filter(([key, value]) => form[key as keyof typeof form] !== value);
    if (hasGenerated && changedEntries.length) {
      setRequestDirtyAfterGeneration(true);
    }
    if (changedEntries.some(([key]) => directionSensitiveFields.includes(key as keyof StorybookRequestForm))) {
      setSelectedDirectionId(null);
      setEditingReview(null);
      setOutlineAdjustPage(null);
      setImagePreferenceOpen(false);
    }
    if ("useScene" in patch) {
      setUseSceneExplicit(Boolean(patch.useScene?.trim()));
    } else if (typeof patch.quickIdea === "string" && patch.quickIdea.trim() !== form.quickIdea.trim()) {
      setUseSceneExplicit(false);
    }
    setForm((current) => {
      const next = { ...current, ...patch };
      if (typeof patch.quickIdea === "string") {
        // A different story idea starts a new context. Keep explicit fields only when
        // the user changes them in the same interaction; inferred scene data must not
        // silently describe the previous idea.
        if (patch.quickIdea.trim() !== current.quickIdea.trim() && !("useScene" in patch)) {
          next.useScene = "";
        }
        return normalizeRecoveredForm(next) as StorybookRequestForm;
      }
      return next;
    });
  };
  const staleRecoveredJobNotice = (job: GenerationJob) => ({
    title: "上次作品生成已中断",
    copy: `检测到未完成的${generationJobTypeLabel[job.jobType] || "生成"}，但它是在服务重启或页面离开前开始的，无法继续执行。请点击重新生成。`,
  });

  useEffect(() => {
    try {
      const raw = window.localStorage.getItem(creationSessionStorageKey);
      if (!raw) {
        setSessionHydrated(true);
        return;
      }
      const saved = JSON.parse(raw) as Partial<StoredCreationSession>;
      if (saved.workspaceId !== workspace.id) {
        setSessionHydrated(true);
        return;
      }
      if (resumeBookId && saved.createdBookId && saved.createdBookId !== resumeBookId) {
        setSessionHydrated(true);
        return;
      }
      const recoveredForm = saved.form;
      if (recoveredForm) {
        const normalizedForm = normalizeRecoveredForm(recoveredForm);
        // Sessions created before this marker existed cannot tell whether their
        // scene came from the user or from an earlier story. Treat it as inferred.
        if (!saved.useSceneExplicit) normalizedForm.useScene = "";
        setForm((current) => ({ ...current, ...normalizedForm }));
      }
      if (saved.planDraft) setPlanDraft((current) => ({ ...current, ...saved.planDraft }));
      if (typeof saved.selectedDirectionId === "string") setSelectedDirectionId(saved.selectedDirectionId);
      if (typeof saved.directionBatch === "number") setDirectionBatch(saved.directionBatch);
      if (Array.isArray(saved.customMaterials)) setCustomMaterials(saved.customMaterials.filter((item) => typeof item === "string"));
      if (typeof saved.visualComplexity === "string") setVisualComplexity(saved.visualComplexity);
      if (typeof saved.characterConsistency === "string") setCharacterConsistency(saved.characterConsistency);
      if (saved.useSceneExplicit === true) setUseSceneExplicit(true);
      if (typeof saved.createdBookId === "string") setCreatedBookId(saved.createdBookId);
      if (typeof saved.updatedAt === "string") setRestoredSessionAt(saved.updatedAt);
      if (typeof saved.step === "number") {
        const restoredStep = Math.min(Math.max(saved.step, 0), steps.length - 1);
        setStep(restoredStep);
        setUnlockedStep(Math.max(restoredStep, typeof saved.unlockedStep === "number" ? saved.unlockedStep : restoredStep));
      }
    } catch {
      window.localStorage.removeItem(creationSessionStorageKey);
    } finally {
      setSessionHydrated(true);
    }
  }, [creationSessionStorageKey, resumeBookId, workspace.id]);

  useEffect(() => {
    if (!sessionHydrated) return;
    const payload: StoredCreationSession = {
      workspaceId: workspace.id,
      step,
      unlockedStep,
      form,
      planDraft,
      selectedDirectionId,
      directionBatch,
      customMaterials,
      visualComplexity,
      characterConsistency,
      useSceneExplicit,
      createdBookId,
      updatedAt: new Date().toISOString(),
    };
    window.localStorage.setItem(creationSessionStorageKey, JSON.stringify(payload));
  }, [
    characterConsistency,
    createdBookId,
    creationSessionStorageKey,
    customMaterials,
    directionBatch,
    form,
    planDraft,
    selectedDirectionId,
    sessionHydrated,
    step,
    unlockedStep,
    useSceneExplicit,
    visualComplexity,
    workspace.id,
  ]);

  // Repair drafts created before quick adjustments were made idempotent.
  // Only known generated suffixes are normalized; user-authored prose is untouched.
  useEffect(() => {
    setPlanDraft((current) => {
      const outlineText = normalizeOutlineQuickAdjustmentText(current.outlineText);
      return outlineText === current.outlineText ? current : { ...current, outlineText };
    });
  }, [planDraft.outlineText]);

  useEffect(() => {
    getWorkspaceGenerationProvider(workspace.id).then(setProvider).catch(() => setProvider(null));
  }, [workspace.id]);

  // 断线恢复：刷新后如果还有向导类生成任务在跑，恢复表单上下文并继续等待结果。
  useEffect(() => {
    if (resumeBookId) return;
    if (suppressAutoRecoverRef.current) return;
    let mounted = true;
    const recoveryRunId = generationRunRef.current;
    listGenerationJobsPage(workspace.id, { limit: 10 })
      .then((page) => {
        if (!mounted || !isCurrentGenerationRun(recoveryRunId)) return;
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
          quickIdea: typeof input.quick_idea === "string" ? input.quick_idea : current.quickIdea,
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
          title: "已恢复排队中的作品生成",
          copy: "检测到未完成的作品生成，系统会继续等待结果；如果长时间没有变化，请重新生成。",
        });
        setGeneratingStep(active.jobType);
        waitForGenerationJob(active)
          .then((settled) => {
            if (mounted && isCurrentGenerationRun(recoveryRunId)) {
              void handleGenerationJob(settled, "作品生成已完成", recoveryRunId);
            }
          })
          .catch(() => {
            if (mounted && isCurrentGenerationRun(recoveryRunId)) {
              setNotice({
                title: "原作品生成已失效",
                copy: "未完成的作品生成已不存在或无法读取，请直接重新生成。",
              });
            }
          })
          .finally(() => { if (mounted && isCurrentGenerationRun(recoveryRunId)) setGeneratingStep(null); });
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
    const recoveryRunId = generationRunRef.current;
    void (async () => {
      try {
        const book = await getStorybook(workspace.id, resumeBookId);
        if (!mounted || !isCurrentGenerationRun(recoveryRunId)) return;
        setCreatedBookId(book.id);
        setGenerationOutputs({});
        setEditableRoles([]);
        setEditablePages([]);
        setPlanDraft({ summary: "", outlineText: "", roleRequirementsText: "", reviewPointsText: "" });
        const jobsPage = await listGenerationJobsPage(workspace.id, { storybookId: book.id, limit: 50 });
        if (!mounted || !isCurrentGenerationRun(recoveryRunId)) return;
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
        const failedJob = wizardJobs.find((job) => {
          if (job.status !== "failed") return false;
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
          quickIdea: typeof planInput.quick_idea === "string" ? planInput.quick_idea : current.quickIdea,
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
        if (
          (hasPagesOutput || ["editing", "image_pending", "exportable", "submitted", "listed"].includes(book.status))
          && !failedJob
          && !activeJob
        ) {
          navigate(`/app/${workspace.id}/storybooks/${book.id}?result=plain&from=new`, { replace: true });
          return;
        }
        const restoredStep = hasPlanOutput ? 1 : 0;
        goToStep(restoredStep);
        setEditingReview(null);
        setNotice({
          title: "已恢复向导进度",
          copy: hasPagesOutput || ["editing", "image_pending", "exportable", "submitted", "listed"].includes(book.status)
            ? "已载入上次的故事草稿和图文内容，可继续生成或进入详情精修。"
            : hasRolesOutput || book.status === "roles_pending"
              ? "已载入上次的故事草稿和角色内容，可继续生成图文。"
              : hasPlanOutput
                ? "已载入上次的故事草稿，可继续生成图文。"
                : "这本绘本还没有生成记录，请从需求开始。",
        });
        if (failedJob) {
          const failedStep = failedJob.jobType === "storybook_plan" ? 0 : 2;
          goToStep(Math.max(restoredStep, failedStep));
          setGenerationPhase("failed");
          setRetryJob(failedJob);
          setNotice({
            title: `${generationJobTypeLabel[failedJob.jobType] || "生成"}失败`,
            copy: `${generationErrorMessage(failedJob)}。可以重试，或返回前一步调整后再生成。`,
          });
          return;
        }
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
            quickIdea: typeof activeInput.quick_idea === "string" ? activeInput.quick_idea : current.quickIdea,
          }));
          const activeStep = activeJob.jobType === "storybook_plan" ? 0 : 2;
          goToStep(Math.max(restoredStep, activeStep));
          if (activeJob.status === "running" || activeJob.lockedAt) {
            setRetryJob(activeJob);
            setNotice(staleRecoveredJobNotice(activeJob));
            return;
          }
          setNotice({
            title: "已恢复排队中的作品生成",
            copy: "检测到未完成的作品生成，系统会继续等待结果；如果长时间没有变化，请重新生成。",
          });
          setGeneratingStep(activeJob.jobType);
          waitForGenerationJob(activeJob)
            .then(async (settled) => {
              if (!mounted || !isCurrentGenerationRun(recoveryRunId)) return;
              const ok = await handleGenerationJob(settled, "作品生成已完成", recoveryRunId);
              if (!ok) return;
              if (settled.jobType === "storybook_plan") goToStep(1);
              if (settled.jobType !== "storybook_plan") goToStep(2);
            })
            .catch(() => {
              if (mounted && isCurrentGenerationRun(recoveryRunId)) {
                setNotice({
                  title: "原作品生成已失效",
                  copy: "未完成的作品生成已不存在或无法读取，请直接重新生成。",
                });
              }
            })
            .finally(() => { if (mounted && isCurrentGenerationRun(recoveryRunId)) setGeneratingStep(null); });
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
    beginGenerationRun();
    setGenerationOutputs({});
    setPlanDraft({ summary: "", outlineText: "", roleRequirementsText: "", reviewPointsText: "" });
    setEditableRoles([]);
    setEditablePages([]);
    setCreatedBookId(null);
    setGeneratingStep(null);
    setGenerationPhase("idle");
    setFullDraftGenerating(false);
    setUnlockedStep(0);
    setEditingReview(null);
    setRequestDirtyAfterGeneration(false);
    setRetryJob(null);
    setSelectedDirectionId(null);
    setDirectionSupplement("");
    setOutlineAdjustPage(null);
    setImagePreferenceOpen(false);
    setDirectionBatch((value) => value + 1);
    resumedBookIdRef.current = null;
    suppressAutoRecoverRef.current = true;
    if (searchParams.get("bookId")) setSearchParams({}, { replace: true });
  };
  const clearGeneratedStoryContent = () => {
    setGenerationOutputs((outputs) => {
      const { storybook_roles: _roles, storybook_pages: _pages, ...remainingOutputs } = outputs;
      return remainingOutputs;
    });
    setEditableRoles([]);
    setEditablePages([]);
    setRetryJob(null);
  };
  const ensureStorybookCreated = async (options: { forceNew?: boolean } = {}) => {
    if (createdBookId && !options.forceNew) {
      try {
        await getStorybook(workspace.id, createdBookId);
        return createdBookId;
      } catch {
        // A recovered browser draft can outlive a deleted or reset backend record.
        // Do not enqueue work against that dead id; recreate the draft below.
        setCreatedBookId(null);
        resumedBookIdRef.current = null;
        if (searchParams.get("bookId")) setSearchParams({}, { replace: true });
      }
    }
    setCreating(true);
    try {
      const book = await createStorybook(workspace.id, {
        title: storybookTitleFor(form),
        ageGroup: form.ageGroup,
        useScene: storybookUseSceneFor(form),
        teachingGoal: storybookThemeFor(form),
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
    overrides?: { plan?: EditablePlan; roles?: EditableRole[]; pages?: EditablePage[]; form?: StorybookRequestForm },
    options: { forceNewStorybook?: boolean; runId?: number } = {},
  ): Promise<GenerationJob | null> => {
    const runId = options.runId ?? beginGenerationRun();
    setGeneratingStep(jobType);
    setRetryJob(null);
    setNotice({
      title: jobType === "storybook_plan" ? "正在生成故事方向" : "正在生成绘本内容",
      copy: jobType === "storybook_plan" ? "正在整理你的故事想法，通常需要一点时间。" : "正在整理故事内容，请稍候。",
    });
    try {
      // 每个向导生成任务都必须绑定到绘本草稿。
      // 否则第一步方案生成只存在前端内存里，刷新页面后无法从后端恢复。
      const bookId = await ensureStorybookCreated({ forceNew: options.forceNewStorybook });
      if (!isCurrentGenerationRun(runId)) return null;
      const job = await createGenerationJob(workspace.id, {
        jobType,
        storybookId: bookId || undefined,
        input: generationInputFor(
          jobType,
          overrides?.form ?? effectiveForm,
          overrides?.plan ?? planDraft,
          overrides?.roles ?? currentRoles,
          overrides?.pages ?? currentPages,
        ),
      });
      const settledJob = await waitForGenerationJob(job);
      if (!isCurrentGenerationRun(runId)) return null;
      const ok = await handleGenerationJob(settledJob, title, runId);
      return ok ? settledJob : null;
    } catch (err) {
      if (!isCurrentGenerationRun(runId)) return null;
      setRetryJob(null);
      setNotice({ title: "生成失败", copy: err instanceof Error ? err.message : "请稍后重试" });
      return null;
    } finally {
      if (isCurrentGenerationRun(runId)) setGeneratingStep(null);
    }
  };

  const generateFullDraft = async () => {
    const runId = beginGenerationRun();
    setRetryJob(null);
    setNotice(null);
    // "重新生成整本绘本" means a new set of generated content. Keep the confirmed
    // plan, but never treat the prior roles/pages as reusable completion.
    clearGeneratedStoryContent();
    setFullDraftGenerating(true);
    setGenerationPhase("roles");
    try {
      const bookId = await ensureStorybookCreated();
      if (!isCurrentGenerationRun(runId)) return;
      if (!bookId) throw new Error("需要先创建绘本草稿。");
      await persistStorybookMeta(bookId);
      if (!isCurrentGenerationRun(runId)) return;

      const rolesJob = await runGeneration("storybook_roles", "角色和道具已整理", undefined, { runId });
      if (!isCurrentGenerationRun(runId)) return;
      if (!rolesJob?.output) {
        setGenerationPhase("failed");
        return;
      }
      const rolesForPages = rolesFromOutput(rolesJob.output);

      setGenerationPhase("pages");
      const pagesJob = await runGeneration("storybook_pages", "图文草稿已生成", {
        plan: planDraft,
        roles: rolesForPages,
        pages: [],
      }, { runId });
      if (!isCurrentGenerationRun(runId)) return;
      if (!pagesJob?.output) {
        setGenerationPhase("failed");
        return;
      }

      setGenerationPhase("references");
      await autoGenerateRoleReferences(bookId);
      if (!isCurrentGenerationRun(runId)) return;
      await updateStorybook(workspace.id, bookId, { status: "editing" });
      if (!isCurrentGenerationRun(runId)) return;
      setGenerationPhase("done");
      window.localStorage.removeItem(creationSessionStorageKey);
      navigate(`/app/${workspace.id}/storybooks/${bookId}?result=plain&from=new`);
    } catch (err) {
      if (!isCurrentGenerationRun(runId)) return;
      setGenerationPhase("failed");
      setNotice({ title: "生成图文失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      if (isCurrentGenerationRun(runId)) {
        setFullDraftGenerating(false);
        setGeneratingStep(null);
      }
    }
  };

  // 方案重新生成后，下游的角色和分页仍是旧方案的产物，必须按新方案联动重生；
  // 直接用新任务的输出作为下一步输入，避免闭包里旧的 planDraft/currentRoles。
  const regeneratePlanWithCascade = async (formOverride?: StorybookRequestForm) => {
    const runId = beginGenerationRun();
    const hadRoles = Boolean(generationOutputs.storybook_roles) || currentRoles.length > 0;
    const hadPages = Boolean(generationOutputs.storybook_pages) || currentPages.length > 0;
    clearGeneratedStoryContent();
    setSelectedDirectionId(null);
    setDirectionBatch((value) => value + 1);
    const sourceForm = formOverride ?? effectiveForm;
    setGenerationPhase("plan");
    const planJob = await runGeneration("storybook_plan", "故事草稿已重新生成", { form: sourceForm }, { runId });
    if (!isCurrentGenerationRun(runId)) return;
    if (!planJob?.output) {
      setGenerationPhase("failed");
      return;
    }
    if (!hadRoles) {
      setGenerationPhase("idle");
      return;
    }
    const freshPlan = planDraftFromOutput(planJob.output, sourceForm);
    setGenerationPhase("roles");
    const rolesJob = await runGeneration("storybook_roles", "角色和道具已按新草稿更新", { plan: freshPlan, form: sourceForm }, { runId });
    if (!isCurrentGenerationRun(runId)) return;
    if (!rolesJob?.output) {
      setGenerationPhase("failed");
      return;
    }
    if (!hadPages) {
      setGenerationPhase("idle");
      return;
    }
    const freshRoles = rolesFromOutput(rolesJob.output);
    setGenerationPhase("pages");
    const pagesJob = await runGeneration("storybook_pages", "图文草稿已按新方向更新", {
      plan: freshPlan,
      roles: freshRoles.length ? freshRoles : currentRoles,
      pages: [],
      form: sourceForm,
    }, { runId });
    if (!isCurrentGenerationRun(runId)) return;
    setGenerationPhase(pagesJob?.output ? "idle" : "failed");
  };

  const waitForGenerationJob = (initialJob: GenerationJob) =>
    pollGenerationJob(workspace.id, initialJob, { timeoutMs: 240_000 });
  const dismissRecoveredGenerationNotice = () => {
    setRetryJob(null);
    setNotice(null);
    setGeneratingStep(null);
    setGenerationPhase("idle");
  };
  const retryFailedGeneration = async () => {
    if (!retryJob) return;
    const runId = beginGenerationRun();
    setGeneratingStep(retryJob.jobType);
    setGenerationPhase(generationPhaseForJobType(retryJob.jobType));
    setNotice(null);
    try {
      const settledJob = retryJob.status === "failed"
        ? await retryGenerationJob(workspace.id, retryJob.id)
          .then(waitForGenerationJob)
          .then(async (job) => (await handleGenerationJob(job, "已重新生成", runId) ? job : null))
        : await runGeneration(retryJob.jobType, "已重新生成", undefined, { runId });
      if (!isCurrentGenerationRun(runId)) return;
      if (settledJob?.jobType === "storybook_plan") goToStep(1);
      if (settledJob && settledJob.jobType !== "storybook_plan") goToStep(2);
      if (settledJob) setGenerationPhase("idle");
    } catch (err) {
      if (!isCurrentGenerationRun(runId)) return;
      setGenerationPhase("failed");
      setNotice({ title: "重试失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      if (isCurrentGenerationRun(runId)) setGeneratingStep(null);
    }
  };
  const handleGenerationJob = async (job: GenerationJob, title: string, runId?: number) => {
    if (runId !== undefined && !isCurrentGenerationRun(runId)) return false;
    if (job.status === "failed") {
      setGenerationPhase("failed");
      setRetryJob(job);
      setNotice({
        title: "生成失败",
        copy: `${generationErrorMessage(job)}。可以重试，或返回前一步调整后再生成。`,
      });
      return false;
    }
    if (["queued", "running"].includes(job.status)) {
      setRetryJob(null);
      setGenerationPhase("idle");
      setNotice({
        title: "作品还在生成",
        copy: `当前状态：${generationStatusLabel(job.status)}。稍后可重新点击继续。`,
      });
      return false;
    }
    setRetryJob(null);
    if (job.output) {
      let generatedRoles: EditableRole[] | null = null;
      let generatedPages: EditablePage[] | null = null;
      if (job.jobType === "storybook_roles") {
        generatedRoles = job.storybookId
          ? rolesFromStorybook((await getStorybook(workspace.id, job.storybookId)).roles)
          : rolesFromOutput(job.output);
      }
      if (job.jobType === "storybook_pages") {
        generatedPages = job.storybookId
          ? pagesFromStorybook((await getStorybook(workspace.id, job.storybookId)).pages)
          : pagesFromOutput(job.output);
      }
      if (runId !== undefined && !isCurrentGenerationRun(runId)) return false;
      setGenerationOutputs((outputs) => ({ ...outputs, [job.jobType]: job.output }));
      if (job.jobType === "storybook_plan") {
        setPlanDraft(planDraftFromOutput(job.output, form));
        setRequestDirtyAfterGeneration(false);
      }
      if (job.jobType === "storybook_roles" && generatedRoles) {
        setEditableRoles(generatedRoles.length ? generatedRoles : rolesFromOutput(job.output));
      }
      if (job.jobType === "storybook_pages" && generatedPages) {
        setEditablePages(generatedPages.length ? generatedPages : pagesFromOutput(job.output));
      }
    }
    if (runId !== undefined && !isCurrentGenerationRun(runId)) return false;
    setNotice({ title, copy: successNoticeCopy(job.jobType) });
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
      title: storybookTitleFor(form),
      ageGroup: form.ageGroup,
      useScene: storybookUseSceneFor(form),
      teachingGoal: storybookThemeFor(form),
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
          title: "正在整理角色",
          copy: "当前角色还没有写入绘本，正在先整理并保存角色，完成后会继续生成参考图。",
        });
        const rolesJob = await runGeneration("storybook_roles", "角色和道具已写入绘本");
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
        title: "角色形象已开始准备",
        copy: `分页已生成，同时为 ${queued} 个跨页角色准备参考图${failed ? `（${failed} 个没有开始，可在验收工作台手动重试）` : ""}。进入验收工作台可查看进度，参考图会让后续插图保持同一形象。`,
      });
    } else if (failed > 0) {
      setNotice({
        title: "角色形象准备失败",
        copy: "分页已生成，但有角色形象没有开始准备，可在验收工作台的角色管理中手动生成。",
      });
    }
  };
  const handlePrimary = async () => {
    setNotice(null);
    if (step === 0) {
      if (!intentReady) {
        setNotice({ title: "先补充一句故事想法", copy: "可以写得很简单，比如：给乐乐做一本关于分享小汽车的温柔故事。" });
        return;
      }
      const shouldResetGenerated = requestDirtyAfterGeneration && Boolean(
        createdBookId
        || generationOutputs.storybook_plan
        || editableRoles.length
        || editablePages.length
        || planDraft.summary
        || planDraft.outlineText,
      );
      if (shouldResetGenerated) {
        if (!window.confirm("重新生成会替换现在的故事草稿和后续内容，确定继续吗？")) {
          return;
        }
        clearGeneratedProgress();
      }
      setGenerationPhase("plan");
      if (await runGeneration("storybook_plan", "故事草稿已生成", undefined, { forceNewStorybook: shouldResetGenerated })) {
        setGenerationPhase("idle");
        goToStep(1);
      } else {
        setGenerationPhase("failed");
      }
      return;
    }
    if (step === 1) {
      if (!selectedDirection) {
        setNotice({ title: "请选择一个故事方向", copy: "方向选择是共创流程里的关键一步，选中后再继续生成大纲。" });
        return;
      }
      updateRequestForm({ storyFramework: `${selectedDirection.title}：${selectedDirection.summary}\n${selectedDirection.personalHook}` });
      goToStep(2);
      return;
    }
    if (step === 2) {
      goToStep(3);
      return;
    }
    if (step === 3) {
      if (retryJob) {
        setRetryJob(null);
        await generateFullDraft();
        return;
      }
      await generateFullDraft();
      return;
    }
    goToStep(Math.min(steps.length - 1, step + 1));
  };
  const submitDirectionSupplement = () => {
    const supplement = directionSupplement.trim();
    if (!supplement) return;
    const nextForm = { ...effectiveForm, quickIdea: `${form.quickIdea.trim()} ${supplement}`.trim() };
    updateRequestForm({ quickIdea: nextForm.quickIdea });
    setDirectionSupplement("");
    setEditingReview(null);
    void regeneratePlanWithCascade(nextForm);
  };
  const applyOutlineAdjustment = (pageNumber: number, action: string) => {
    const currentItems = outlineItemsFor(planDraft, form);
    const nextItems = currentItems.map((item) => {
      if (item.pageNumber !== pageNumber) return item;
      return { ...item, summary: adjustedOutlineSummary(item.summary, action) };
    });
    setPlanDraft((current) => ({
      ...current,
      outlineText: nextItems.map((item) => `${item.pageNumber}. ${item.summary}`).join("\n"),
    }));
    setOutlineAdjustPage(null);
    showNotice("大纲已调整", `第 ${pageNumber} 页已按「${action}」更新，可以继续生成。`);
  };
  return (
    <div className="page-stack">
      <header className="wizard-header">
        <h1>创建专属故事</h1>
        <span>创建在 {workspace.name}</span>
      </header>
      {provider && !provider.realTextReady && (retryJob || step === 3) && (
        <Notice
          title="当前使用演示生成"
          copy="真实生成服务还没有完全配置好，当前会先用演示内容跑通创作流程。"
          tone="warn"
        />
      )}
      <WizardTopNav
        steps={steps}
        active={step}
        maxUnlockedStep={unlockedStep}
        disabled={flowBusy}
        onSelect={(next) => { if (!flowBusy) goToStep(next); }}
      />
      <div className="wizard-shell wizard-shell-single">
        <Card className="wizard-card">
          {notice && (
            <Notice
              title={notice.title}
              copy={notice.copy}
              tone={retryJob ? "danger" : "info"}
              action={retryJob && step !== 3 ? (
                <div className="inline-actions">
                  <button className="button secondary" type="button" disabled={generatingStep === retryJob.jobType} onClick={retryFailedGeneration}>重新生成</button>
                  {isActiveJobStatus(retryJob.status) && (
                    <button className="button ghost" type="button" disabled={Boolean(generatingStep)} onClick={dismissRecoveredGenerationNotice}>忽略</button>
                  )}
                </div>
              ) : undefined}
            />
          )}
          {restoredSessionAt && !notice && (
            <div className="session-recovery-strip" role="status">
              <span>已恢复上次编辑</span>
              <button type="button" onClick={() => setRestoredSessionAt(null)}>知道了</button>
            </div>
          )}
          {step === 0 && (
            <section className="co-creation-step intent-step">
              <div className="co-creation-heading">
                <Badge tone="info">想法</Badge>
                <h2>想做一本什么故事？</h2>
                <p>说一句也可以，我们会帮你整理成故事方向。</p>
              </div>
              <label className="intent-input">
                <span>故事想法</span>
                <textarea
                  value={form.quickIdea}
                  disabled={Boolean(generatingStep)}
                  onChange={(event) => updateRequestForm({ quickIdea: event.target.value })}
                  placeholder="给 4 岁的乐乐，他最近不太愿意分享红色小汽车，想做一本温柔的小故事。"
                />
                {!intentReady && <small>{form.quickIdea.trim() ? "可以再写几个字，说明想解决的问题或想讲的事。" : "先写一句想法，或选择下面的示例开始。"}</small>}
              </label>
              <div className="example-start-grid" aria-label="示例起点">
                {storyStartExamples.map((example) => (
                  <button
                    key={example.title}
                    type="button"
                    className="example-start-card"
                    disabled={Boolean(generatingStep)}
                    onClick={() => updateRequestForm({ quickIdea: example.value, ...example.defaults })}
                  >
                    <strong>{example.title}</strong>
                    <span>{example.copy}</span>
                  </button>
                ))}
              </div>
              <details className="compact-disclosure">
                <summary>更多故事设置</summary>
                <RequestStepForm
                  form={form}
                  disabled={Boolean(generatingStep)}
                  styleCardsExpanded={styleCardsExpanded}
                  customStyleOpen={customStyleOpen}
                  onChange={updateRequestForm}
                  onToggleStyleCards={() => setStyleCardsExpanded((value) => !value)}
                  onToggleCustomStyle={() => setCustomStyleOpen((value) => !value)}
                />
              </details>
            </section>
          )}
          {step === 1 && (
            <section className="co-creation-step direction-step">
              <div className="understanding-strip">
                <strong>我理解：</strong>
                <span>{understandingFor(form, planDraft)}</span>
              </div>
              <MaterialChips labels={materials} compact prefix="关键细节" />
              <div className="co-creation-heading">
                <Badge tone="info">方向</Badge>
                <h2>想从哪个角度讲这个故事？</h2>
                <p>选择一个方向后，系统会继续整理故事大纲。</p>
              </div>
              <div className="story-direction-grid">
                {storyDirections.map((direction) => (
                  <button
                    key={direction.id}
                    type="button"
                    className={`story-direction-card${selectedDirectionId === direction.id ? " selected" : ""}`}
                    onClick={() => setSelectedDirectionId(direction.id)}
                    >
                      <strong>{direction.title}</strong>
                      <span>{direction.summary}</span>
                      <i>{direction.personalHook}</i>
                    <b>{selectedDirectionId === direction.id ? "已选择" : "选这个"}</b>
                  </button>
                ))}
              </div>
              <div className="inline-actions">
                <button className="button secondary" type="button" disabled={Boolean(generatingStep)} onClick={() => void regeneratePlanWithCascade()}>换一批</button>
                <button className="button ghost" type="button" onClick={() => setEditingReview(editingReview === "plan" ? null : "plan")}>补充细节</button>
              </div>
              {editingReview === "plan" && (
                <div className="inline-editor-panel direction-refinement-panel">
                  <label>
                    添加一句希望保留的真实细节（可选）
                    <textarea
                      rows={1}
                      value={directionSupplement}
                      disabled={Boolean(generatingStep)}
                      placeholder="例如：发生在星星班午睡室，主角带着蓝色雨靴。"
                      onChange={(event) => setDirectionSupplement(event.target.value)}
                    />
                  </label>
                  {selectedDirectionId && (
                    <p className="direction-refinement-note">更新后会生成新的方向，需要重新选择。</p>
                  )}
                  <div className="inline-actions">
                    <button className="button secondary" type="button" onClick={() => setEditingReview(null)}>取消</button>
                    <button className="button primary" type="button" disabled={!directionSupplement.trim() || Boolean(generatingStep)} onClick={submitDirectionSupplement}>更新并重新选择方向</button>
                  </div>
                </div>
              )}
            </section>
          )}
          {step === 2 && (
            <section className="co-creation-step outline-step">
              <div className="co-creation-heading">
                <Badge tone="info">大纲</Badge>
                <h2>故事会这样展开</h2>
                <p>先看走向，满意就直接生成完整故事。</p>
              </div>
              <ol className="outline-review-list">
                {outlineItems.map((item) => (
                  <li key={item.pageNumber}>
                    <span>{item.pageNumber}</span>
                    <div>
                      <strong>{item.summary}</strong>
                      <small>{matchingMaterials(item.summary, materials).slice(0, 2).join("、") || "故事推进"}</small>
                    </div>
                    <button type="button" className="button ghost" onClick={() => setOutlineAdjustPage(outlineAdjustPage === item.pageNumber ? null : item.pageNumber)}>调整这一页</button>
                    {outlineAdjustPage === item.pageNumber && (
                      <div className="outline-quick-actions">
                        {["更短一点", "更温柔", "更有趣", "换个情节", "补充要求"].map((action) => (
                          <button key={action} type="button" onClick={() => applyOutlineAdjustment(item.pageNumber, action)}>{action}</button>
                        ))}
                      </div>
                    )}
                  </li>
                ))}
              </ol>
            </section>
          )}
          {step === 3 && (
            <section className="co-creation-step generation-step">
              {!flowBusy && generationPhase !== "failed" ? (
                <>
                  <div className="co-creation-heading">
                    <Badge tone="good">生成</Badge>
                    <h2>准备生成这本专属绘本</h2>
                    <p>故事和画面会按当前方向生成，技术细节会自动处理。</p>
                  </div>
                  <div className="generation-composer">
                    <div><span>故事</span><strong>{selectedDirection?.title || form.title || "专属故事"}</strong></div>
                    <div><span>页数</span><strong>{form.pageCount || outlineItems.length} 页 + 封面</strong></div>
                    <div><span>画面设置会参与生成</span><strong>{visualSummary}</strong></div>
                  </div>
                  <button className="button secondary" type="button" onClick={() => setImagePreferenceOpen((value) => !value)}>调整画面</button>
                  {imagePreferenceOpen && (
                    <div className="image-preference-drawer">
                      <ImagePreferenceDrawer
                        form={form}
                        disabled={Boolean(generatingStep)}
                        onChange={updateRequestForm}
                        complexity={visualComplexity}
                        onComplexityChange={setVisualComplexity}
                        consistency={characterConsistency}
                        onConsistencyChange={setCharacterConsistency}
                        summary={visualSummary}
                        onClose={() => setImagePreferenceOpen(false)}
                      />
                    </div>
                  )}
                </>
              ) : (
                <StorybookGenerationProgress
                  phase={generationPhase}
                  generatingStep={generatingStep}
                  failedJob={retryJob}
                  materialLabels={materials}
                  onBackToDraft={() => { setNotice(null); setGenerationPhase("idle"); goToStep(2); }}
                  onRetry={() => void handlePrimary()}
                />
              )}
            </section>
          )}
          <div className="wizard-actions">
            {step > 0 && (
              <ActionButton className="button secondary" disabled={flowBusy} disabledHint="生成进行中，请稍候" onClick={() => { setNotice(null); goToStep(Math.max(0, step - 1)); }}>
                {step === 1 ? "调整想法" : step === 2 ? "返回方向" : "返回大纲"}
              </ActionButton>
            )}
            <ActionButton
              className="button primary"
              disabled={creating || flowBusy || (step === 0 && !intentReady) || (step === 1 && !selectedDirection)}
              disabledHint={flowBusy ? "生成进行中，请稍候" : step === 0 && !intentReady ? "先写一句想法，或选择下面的示例开始" : step === 1 && !selectedDirection ? "请先选择一个故事方向" : undefined}
              onClick={handlePrimary}
            >
              {creating ? "正在创建..." : flowBusy ? "生成中..." : primaryLabels[step]}
            </ActionButton>
          </div>
        </Card>
      </div>
    </div>
  );
}

function roleReferenceBusyKey(role: EditableRole, index: number) {
  return role.id || `${index}:${role.roleType}:${role.name || "未命名角色"}`;
}

function isStoryIdeaReady(value: string) {
  const compact = value.replace(/\s+/g, "");
  if (compact.length >= 4 && /[\u4e00-\u9fa5]/.test(compact)) return true;
  return compact.length >= 10;
}

function storybookTitleFor(form: StorybookRequestForm) {
  const title = form.title.trim();
  if (title) return title;
  const idea = form.quickIdea.trim();
  if (idea) return titleFromIdea(idea);
  return form.theme.trim() || "新建专属故事";
}

function normalizeRecoveredForm(form: Partial<StorybookRequestForm>) {
  const next = { ...form };
  const idea = typeof next.quickIdea === "string" ? next.quickIdea : "";
  const hasLegacyTitle = isLegacyDefaultConflict(idea, next.title, "一起玩小汽车");
  const hasLegacyTheme = isLegacyDefaultConflict(idea, next.theme, "学会分享和轮流");
  const hasLegacyFramework = isLegacyFrameworkConflict(idea, next.storyFramework);
  if (hasLegacyTitle) {
    next.title = "";
  }
  if (hasLegacyTheme) {
    next.theme = "";
  }
  if (hasLegacyFramework) {
    next.storyFramework = "";
  }
  // Earlier drafts did not record whether a scene was manually selected or
  // auto-filled. When their other fields clearly belong to another story, do
  // not leave a misleading scene selection behind.
  if (hasLegacyTitle || hasLegacyTheme || hasLegacyFramework) next.useScene = "";
  return next;
}

function isLegacyDefaultConflict(idea: string, value: unknown, legacyDefault: string) {
  if (value !== legacyDefault) return false;
  return !/(分享|轮流|小汽车|汽车|玩具)/.test(idea);
}

function isLegacyFrameworkConflict(idea: string, value: unknown) {
  if (typeof value !== "string" || !value.trim()) return false;
  const frameworkMentionsLegacyToy = /(一起玩小汽车|小汽车|分享|轮流)/.test(value);
  if (!frameworkMentionsLegacyToy) return false;
  return !/(分享|轮流|小汽车|汽车|玩具)/.test(idea);
}

function storybookThemeFor(form: StorybookRequestForm) {
  const theme = form.theme.trim();
  if (theme) return theme;
  const idea = form.quickIdea.trim();
  if (idea) return themeFromIdea(idea);
  return "围绕孩子成长需要生成一个温柔、清楚的故事";
}

function storybookUseSceneFor(form: StorybookRequestForm) {
  return form.useScene.trim() || useSceneFromIdea(form.quickIdea);
}

function titleFromIdea(idea: string) {
  const compact = idea.replace(/[。.!！?？]+$/, "").replace(/^我想(要)?/, "").trim();
  if (/不打人|打人|动手|抢|推|咬|踢/.test(compact)) return "好好说的小练习";
  if (/分享|轮流|等待/.test(compact)) return "一起轮流玩";
  if (/排队|午睡|规则/.test(compact)) return "班级里的小约定";
  if (/生日|纪念|成长/.test(compact)) return "特别的成长故事";
  return compact.length > 12 ? `${compact.slice(0, 12)}的小故事` : `${compact || "专属"}小故事`;
}

function themeFromIdea(idea: string) {
  if (/不打人|打人|动手|抢|推|咬|踢/.test(idea)) return "学习用语言表达需求，不用动手伤害别人";
  if (/分享|轮流/.test(idea)) return "学习分享和轮流";
  if (/等待|排队/.test(idea)) return "练习等待和遵守规则";
  if (/情绪|生气|难过|害怕/.test(idea)) return "识别情绪并学习表达";
  return idea.replace(/[。.!！?？]+$/, "").slice(0, 24) || "孩子成长";
}

function useSceneFromIdea(idea: string) {
  if (/不打人|打人|动手|抢|推|咬|踢|排队|午睡|规则/.test(idea)) return "规则引导";
  if (/课堂|课程|主题/.test(idea)) return "课堂共读";
  if (/家|家长|睡前/.test(idea)) return "家园沟通";
  return "课堂共读";
}

function isBlockingGenerationPhase(phase: GenerationPhase) {
  return ["plan", "roles", "pages", "references"].includes(phase);
}

function generationPhaseForJobType(jobType: string): GenerationPhase {
  if (jobType === "storybook_plan") return "plan";
  if (jobType === "storybook_roles") return "roles";
  if (jobType === "storybook_pages") return "pages";
  if (jobType === "storybook_role_reference_image") return "references";
  return "idle";
}

function understandingFor(form: StorybookRequestForm, plan: EditablePlan) {
  if (plan.summary.trim()) return plan.summary.trim();
  const idea = form.quickIdea.trim();
  if (idea) return idea.replace(/[。.!！?？]+$/, "");
  return `围绕「${form.theme || "一个成长主题"}」做一本${form.useScene || "适合共读"}的故事`;
}

function materialLabelsFor(form: StorybookRequestForm, plan: EditablePlan, customMaterials: string[] = []) {
  const source = [
    form.quickIdea,
    form.title,
    form.theme,
    form.useScene,
    form.ageGroup,
    plan.summary,
    plan.outlineText,
  ].join(" ");
  const quoted = Array.from(source.matchAll(/[「“"]([^」”"]{2,16})[」”"]/g)).map((match) => match[1]);
  const namedPlaces = Array.from(source.matchAll(/([\u4e00-\u9fa5A-Za-z0-9]{2,12}(?:班|园|幼儿园|学校|家|教室|操场|午睡室|图书角|美工区))/g)).map((match) => match[1]);
  const childNames = Array.from(source.matchAll(/(?:给|为|关于|叫)([\u4e00-\u9fa5A-Za-z0-9]{2,6})(?:，|,|。|的|做|他|她|小朋友|孩子)/g)).map((match) => match[1]);
  const detailPhrases = source
    .split(/[。！？.!?\n]/)
    .map((item) => item.trim())
    .filter((item) => item.length >= 4 && item.length <= 18 && /(生日|分享|轮流|等待|排队|午睡|分离|害怕|勇敢|朋友|规则|纪念|课程|环保|情绪|习惯|小汽车|玩具|雨靴|书包)/.test(item));
  const candidates = [
    ...customMaterials,
    form.title,
    form.theme,
    form.useScene,
    form.ageGroup,
    ...quoted,
    ...childNames,
    ...namedPlaces,
    ...detailPhrases,
    ...["乐乐", "朵朵", "小汽车", "红色小汽车", "分享", "轮流", "等待", "排队", "午睡", "生日", "环保", "温柔", "有趣", "星星班", "操场", "教室"].filter((item) => source.includes(item)),
  ];
  return Array.from(new Set(candidates.map((item) => item.trim()).filter((item) => item && !["孩子", "小朋友", "一个"].includes(item)))).slice(0, 10);
}

function directionsFor(form: StorybookRequestForm, plan: EditablePlan, materials: string[], batch = 0): StoryDirection[] {
  const idea = form.quickIdea.trim() || plan.summary.trim() || form.theme || "一个温暖的小故事";
  const titleSeed = form.title.trim() || firstMaterial(materials) || "专属故事";
  const theme = form.theme.trim() || "成长";
  const useScene = form.useScene.trim() || "共读";
  const hook = personalHookFor(materials, idea);
  const variants = [
    [
      {
        id: "gentle",
        title: `${titleSeed}的小小练习`,
        summary: `${idea.replace(/[。.!！?？]+$/, "")}，用温柔的方式呈现一次尝试和改变。`,
        fitReason: `${theme} / 温柔引导`,
        personalHook: `专属落点：把${hook}放在主角第一次愿意尝试的时刻。`,
      },
      {
        id: "playful",
        title: `${titleSeed}的有趣任务`,
        summary: `把${theme}变成一个轻松的小任务，让主角在${useScene}里自然完成一次选择。`,
        fitReason: "轻松有趣 / 行动感",
        personalHook: `专属落点：用${hook}作为推动情节的小任务。`,
      },
      {
        id: "warm",
        title: `${titleSeed}的暖心时刻`,
        summary: `从一个真实的小冲突开始，让关系、情绪和一句想传达的话成为故事收束。`,
        fitReason: "情绪安抚 / 纪念感",
        personalHook: `专属落点：让${hook}成为故事最后被记住的细节。`,
      },
    ],
    [
      {
        id: "daily",
        title: `${titleSeed}的一天`,
        summary: `把${idea.replace(/[。.!！?？]+$/, "")}放进一个日常小场景，让改变自然发生。`,
        fitReason: "真实日常 / 陪伴感",
        personalHook: `专属落点：从${hook}开始，减少说教感。`,
      },
      {
        id: "choice",
        title: `${titleSeed}的两个选择`,
        summary: `主角遇到两个选择，在试一试和被鼓励中找到适合自己的办法。`,
        fitReason: `${theme} / 主动选择`,
        personalHook: `专属落点：围绕${hook}设计两个可选择的行动。`,
      },
      {
        id: "memory",
        title: `${titleSeed}的特别回忆`,
        summary: `把${useScene}里的真实细节串成一段值得保存的故事。`,
        fitReason: "纪念感 / 私人定制",
        personalHook: `专属落点：把${hook}做成反复出现的记忆符号。`,
      },
    ],
    [
      {
        id: "helper",
        title: `${titleSeed}来帮忙`,
        summary: `让主角先帮助别人，再慢慢理解${theme}对自己的意义。`,
        fitReason: "关系互动 / 正向行动",
        personalHook: `专属落点：让${hook}成为主角帮忙的契机。`,
      },
      {
        id: "secret",
        title: `${titleSeed}的小秘密`,
        summary: `从一个小小秘密或愿望开始，把真实素材变成故事里的惊喜。`,
        fitReason: "想象力 / 情绪表达",
        personalHook: `专属落点：把${hook}藏进一个小惊喜里。`,
      },
      {
        id: "promise",
        title: `${titleSeed}的小约定`,
        summary: `围绕一个清楚的小约定展开，让故事最后落在可执行的行动上。`,
        fitReason: `${theme} / 规则内化`,
        personalHook: `专属落点：用${hook}承接最后的小约定。`,
      },
    ],
  ];
  return variants[batch % variants.length].map((direction) => ({
    ...direction,
    id: `${direction.id}-${batch}`,
    materialLabels: materials,
  }));
}

function firstMaterial(materials: string[]) {
  return materials.find((item) => item.length <= 8) || materials[0];
}

function personalHookFor(materials: string[], idea: string) {
  const material = materials.find((item) => item.length <= 12 && !/岁|规则引导|共读/.test(item)) || materials[0];
  if (material) return `「${material}」`;
  const shortIdea = idea.replace(/[。.!！?？]+$/, "").slice(0, 12);
  return shortIdea ? `「${shortIdea}」` : "一个真实细节";
}

function outlineItemsFor(plan: EditablePlan, form: StorybookRequestForm) {
  const lines = plan.outlineText.split(/\n+/).map((item) => item.trim()).filter(Boolean);
  const fallback = [
    `${form.title || "主角"}进入熟悉场景`,
    `出现和「${form.theme || "成长目标"}」有关的小挑战`,
    "主角表达自己的想法或情绪",
    "身边的人给出清楚、温柔的办法",
    "主角尝试新的做法",
    "故事在被理解和鼓励中收束",
  ];
  return (lines.length ? lines : fallback).slice(0, 8).map((line, index) => ({
    pageNumber: index + 1,
    summary: line.replace(/^第\s*[^：:]+[：:]\s*/, "").replace(/^\d+[.、]\s*/, ""),
  }));
}

function matchingMaterials(text: string, materials: string[]) {
  return materials.filter((item) => text.includes(item));
}

function visualPreferenceSummary(form: StorybookRequestForm, complexity = "simple", consistency = "auto") {
  const style = STYLE_PRESETS.find((preset) => preset.value === form.style)?.label || form.style.trim().replace(/^画面风格：/, "").split(/[。，.]/)[0] || "自动画风";
  return `${style} · ${visualComplexityLabel(complexity)} · ${characterConsistencyLabel(consistency)}`;
}

function MaterialChips({ labels, compact = false, prefix = "已识别" }: { labels: string[]; compact?: boolean; prefix?: string }) {
  const [expanded, setExpanded] = useState(false);
  const visible = labels.slice(0, compact ? 4 : 5);
  const overflow = labels.length - visible.length;
  const displayed = expanded ? labels : visible;
  if (!labels.length) {
    return <p className="material-chip-hint">可以补充名字、地点或真实细节，让故事更像你们。</p>;
  }
  return (
    <div className={`material-chip-row${compact ? " compact" : ""}`}>
      <span>{prefix}：</span>
      {displayed.map((label) => (
        <em key={label} title={label}>{truncateChip(label)}</em>
      ))}
      {overflow > 0 && (
        <button className="material-chip-more" type="button" title={labels.slice(visible.length).join("、")} onClick={() => setExpanded((value) => !value)}>
          {expanded ? "收起" : `+${overflow}`}
        </button>
      )}
    </div>
  );
}

function truncateChip(label: string) {
  return label.length > 10 ? `${label.slice(0, 10)}...` : label;
}

function adjustedOutlineSummary(summary: string, action: string) {
  // Quick actions are alternatives, not accumulative edits. Strip a previous
  // generated suffix first so rapid/repeated clicks cannot duplicate it.
  const clean = summary
    .replace(/(?:，(?:用更温柔的方式被理解|加入一个轻松的小任务|换成一次新的尝试|补充一个真实细节))+$/, "")
    .replace(/[。.!！?？]+$/, "");
  if (action === "更短一点") return clean.length > 22 ? `${clean.slice(0, 22)}...` : clean;
  if (action === "更温柔") return `${clean}，用更温柔的方式被理解`;
  if (action === "更有趣") return `${clean}，加入一个轻松的小任务`;
  if (action === "换个情节") return `${clean}，换成一次新的尝试`;
  return `${clean}，补充一个真实细节`;
}

const outlineQuickAdjustmentSuffix = "用更温柔的方式被理解|加入一个轻松的小任务|换成一次新的尝试|补充一个真实细节";

function normalizeOutlineQuickAdjustmentText(outlineText: string) {
  return outlineText
    .split("\n")
    .map((line) => normalizeOutlineQuickAdjustment(line))
    .join("\n");
}

function normalizeOutlineQuickAdjustment(summary: string) {
  const suffixPattern = new RegExp(`((?:，(?:${outlineQuickAdjustmentSuffix}))+)$`);
  const match = summary.match(suffixPattern);
  if (!match) return summary;
  const suffixes = match[1].match(new RegExp(`，(?:${outlineQuickAdjustmentSuffix})`, "g")) || [];
  if (suffixes.length <= 1) return summary;
  return `${summary.slice(0, -match[1].length)}${suffixes[suffixes.length - 1]}`;
}

function ImagePreferenceDrawer({
  form,
  disabled,
  onChange,
  complexity,
  onComplexityChange,
  consistency,
  onConsistencyChange,
  summary,
  onClose,
}: {
  form: StorybookRequestForm;
  disabled: boolean;
  onChange: (patch: Partial<StorybookRequestForm>) => void;
  complexity: string;
  onComplexityChange: (value: string) => void;
  consistency: string;
  onConsistencyChange: (value: string) => void;
  summary: string;
  onClose: () => void;
}) {
  const styleOptions = STYLE_PRESETS.filter((preset) => ["水彩", "蜡笔", "扁平", "黏土", "国风"].some((keyword) => preset.label.includes(keyword) || preset.tag.includes(keyword))).slice(0, 5);
  return (
    <div className="image-preference-content">
      <fieldset>
        <legend>画风</legend>
        <div className="segmented-wrap">
          {styleOptions.map((preset) => (
            <button key={preset.value} type="button" className={form.style === preset.value ? "active" : ""} disabled={disabled} onClick={() => onChange({ style: preset.value })}>
              {preset.label}
            </button>
          ))}
        </div>
      </fieldset>
      <fieldset>
        <legend>画面复杂度</legend>
        <div className="segmented-wrap">
          {["simple", "standard", "rich"].map((value) => (
            <button key={value} type="button" className={complexity === value ? "active" : ""} disabled={disabled} onClick={() => onComplexityChange(value)}>
              {visualComplexityLabel(value)}
            </button>
          ))}
        </div>
      </fieldset>
      <details>
        <summary>角色一致性</summary>
        <div className="segmented-wrap">
          {["auto", "speed", "confirm_character"].map((value) => (
            <button key={value} type="button" className={consistency === value ? "active" : ""} disabled={disabled} onClick={() => onConsistencyChange(value)}>
              {characterConsistencyLabel(value)}
            </button>
          ))}
        </div>
      </details>
      <div className="image-preference-summary">
        <span>当前：{summary}</span>
        <button className="button primary" type="button" onClick={onClose}>保存设置</button>
      </div>
    </div>
  );
}

function visualComplexityLabel(value: string) {
  return { simple: "简单清楚", standard: "标准", rich: "细节丰富" }[value] || "简单清楚";
}

function characterConsistencyLabel(value: string) {
  return { auto: "自动保持角色一致", speed: "优先速度", confirm_character: "先让我确认主角形象" }[value] || "自动保持角色一致";
}

function successNoticeCopy(jobType: string) {
  if (jobType === "storybook_plan") return "看一眼故事方向，合适就继续生成图文。";
  if (jobType === "storybook_roles") return "角色和关键道具已经整理好，系统会继续生成分页。";
  if (jobType === "storybook_pages") return "分页图文已经准备好，接下来会进入绘本详情继续检查。";
  if (jobType === "storybook_role_reference_image") return "参考图已经更新，后续插图会更容易保持同一形象。";
  return "生成内容已更新，可以继续下一步。";
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
