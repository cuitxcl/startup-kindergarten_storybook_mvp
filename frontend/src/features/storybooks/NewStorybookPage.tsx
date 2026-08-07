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
import { ActionButton, Badge, Card, Notice, WizardSideNav } from "../../components/ui";
import { GenerationReviewBlock } from "../../components/GenerationReviewBlock";
import type { Storybook, StorybookPage, StorybookRole, Workspace } from "../../types/domain";
import {
  generationErrorMessage,
  generationStatusLabel,
  isActiveJobStatus,
  pollGenerationJob,
} from "../../utils/generation";
import { generationJobTypeLabel } from "../../utils/labels";

const steps = ["需求", "绘本方案", "角色道具", "分页编辑", "预览导出"];

// 画面风格预设：每种画风配一张代表性预览图，选中后生成插图时会严格按该画风绘制。
import imgPixar from "../../assets/style-presets/pixar.jpg";
import imgGhibli from "../../assets/style-presets/ghibli.jpg";
import imgWatercolor from "../../assets/style-presets/watercolor.jpg";
import imgPencil from "../../assets/style-presets/pencil.jpg";
import imgInk from "../../assets/style-presets/ink.jpg";
import imgCartoon from "../../assets/style-presets/cartoon.jpg";
import imgGongbi from "../../assets/style-presets/gongbi.jpg";
import imgFlat from "../../assets/style-presets/flat.jpg";
import imgCrayon from "../../assets/style-presets/crayon.jpg";
import imgPapercut from "../../assets/style-presets/papercut.jpg";
import imgClay from "../../assets/style-presets/clay.jpg";
import imgOilpaint from "../../assets/style-presets/oilpaint.jpg";

const STYLE_PRESETS = [
  { label: "皮克斯3D", tag: "立体动画感", image: imgPixar, value: "画面风格：皮克斯3D动画风。高质量3D渲染，角色立体圆润、毛发细腻、大眼睛表情生动，柔和影棚光，色彩鲜艳明快，像动画电影截图。" },
  { label: "宫崎骏", tag: "吉卜力手绘", image: imgGhibli, value: "画面风格：宫崎骏/吉卜力手绘风。手绘水彩质感，背景细腻丰富，天空云朵柔和，光影温暖治愈，带淡淡怀旧气息。" },
  { label: "水彩绘本", tag: "清新通透", image: imgWatercolor, value: "画面风格：水彩绘本风。半透明水彩晕染，边缘柔和，留白自然，可见纸张纹理，色调清淡通透，经典童书插画感。" },
  { label: "手绘彩铅", tag: "细腻笔触", image: imgPencil, value: "画面风格：手绘彩铅风。可见铅笔排线和叠色笔触，质感温暖手工感强，色调柔和，像老师亲手绘制的插画。" },
  { label: "水墨画", tag: "东方韵味", image: imgInk, value: "画面风格：中国水墨画风。毛笔笔触灵动，墨色浓淡变化，点缀淡雅色彩，留白有意境，宣纸质感，东方美学。" },
  { label: "卡通", tag: "简洁明快", image: imgCartoon, value: "画面风格：卡通插画风。粗描边、平涂鲜艳色块，造型简洁夸张，表情生动可爱，现代儿童动画感，画面干净明快。" },
  { label: "国风工笔画", tag: "精致典雅", image: imgGongbi, value: "画面风格：国风工笔画风。线条细腻工整，层层晕染的矿物色彩，构图典雅精致，传统国画质感，适合文化主题。" },
  { label: "扁平插画", tag: "现代简约", image: imgFlat, value: "画面风格：扁平插画风。极简几何造型，大面积纯色块，无渐变无阴影，配色时尚，现代儿童读物设计感。" },
  { label: "蜡笔油画棒", tag: "童趣涂鸦", image: imgCrayon, value: "画面风格：蜡笔油画棒风。厚重蜡笔质感，色彩浓艳大胆，笔触稚拙童趣，像孩子自己的涂鸦作品，纸纹明显。" },
  { label: "剪纸拼贴", tag: "手工纸艺", image: imgPapercut, value: "画面风格：剪纸拼贴风。层叠剪纸造型，可见纸张边缘和投影，手工纸艺质感，色彩对比鲜明，民间艺术趣味。" },
  { label: "黏土定格", tag: "手作立体", image: imgClay, value: "画面风格：黏土定格动画风。角色像手工捏制的黏土，带指纹纹理，材质柔软立体，柔和布光，阿德曼动画式手作魅力。" },
  { label: "厚涂油画", tag: "艺术质感", image: imgOilpaint, value: "画面风格：厚涂油画风。可见丰富笔触和颜料堆叠质感，色彩浓郁温暖，光影细腻，美术馆级绘本封面感。" },
];

// 故事风格预设：情节基调与叙事类型，决定故事怎么走；与画面风格（怎么画）互相独立，可自由组合。
const STORY_STYLE_PRESETS = [
  { label: "日常温情型", tag: "治愈系", value: "日常温情治愈系：以幼儿园真实生活为底色，节奏舒缓，情节小而暖，老师温柔引导，结尾有拥抱感的小收获。" },
  { label: "冒险奇幻型", tag: "想象力爆棚", value: "冒险奇幻型：魔法森林、发光生物、会说话的伙伴，画面瑰丽夸张，情节一波三折，鼓励孩子勇敢探索。" },
  { label: "幽默搞笑型", tag: "笑点不断", value: "幽默搞笑型：夸张的误会和反转情节，角色表情动作喜剧化，让孩子在笑声中明白一个小道理。" },
  { label: "科学探索型", tag: "满足好奇心", value: "科学探索型：从孩子的一个小疑问出发，经历观察、猜想、验证的过程，认识自然和生活中的科学现象。" },
  { label: "成长引导型", tag: "规则与习惯", value: "成长引导型：聚焦规则、习惯和社交情景，冲突真实、解决办法具体，老师示范清晰，适合班级共读讨论。" },
  { label: "自然认知型", tag: "亲近自然", value: "自然认知型：以四季、动植物、天气变化为线索，画面清新细腻，在观察自然的过程中渗透认知目标。" },
  { label: "睡前安恬型", tag: "温柔入眠", value: "睡前安恬型：节奏轻缓、画面柔和，情节安稳温暖，以晚安和拥抱收尾，帮助孩子平静下来进入睡眠。" },
  { label: "节日民俗型", tag: "传统节庆", value: "节日民俗型：围绕传统节日与民俗活动展开，融入灯笼、舞龙、美食等节庆元素，热闹喜庆，渗透文化认知。" },
  { label: "海洋探险型", tag: "海底世界", value: "海洋探险型：潜入五彩斑斓的海底世界，认识海洋动物朋友，情节新奇有趣，传递保护海洋的小意识。" },
  { label: "太空科幻型", tag: "宇宙奇想", value: "太空科幻型：乘坐飞船遨游宇宙，遇见星球、机器人和外星人朋友，画面充满未来感，激发对宇宙的好奇。" },
  { label: "复古民间故事型", tag: "经典回味", value: "复古民间故事型：采用民间故事叙事方式，有老故事的韵味和寓意，画面带传统质朴质感，适合品格启蒙。" },
  { label: "动物拟人型", tag: "萌趣动物村", value: "动物拟人型：动物角色穿衣说话、上学上班，像小朋友一样生活和交朋友，萌趣可爱，贴近孩子的同伴世界。" },
];

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
  const [styleCardsExpanded, setStyleCardsExpanded] = useState(false);
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
    style: STYLE_PRESETS[0].value,
    storyStyle: STORY_STYLE_PRESETS[0].value,
    storyFramework: "",
  });
  const [searchParams, setSearchParams] = useSearchParams();
  const resumeBookId = searchParams.get("bookId");
  const resumedBookIdRef = useRef<string | null>(null);
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
  const reviewForStep = (nextStep: number): null | "plan" | "roles" | "pages" => {
    if (nextStep === 1) return "plan";
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
    setSearchParams({ bookId }, { replace: true });
  };
  const updateRequestForm = (patch: Partial<typeof form>) => {
    // 已有生成产出时，修改需求会先确认再清空，避免误操作抹掉已生成内容。
    const hasGenerated = Boolean(
      createdBookId
      || generationOutputs.storybook_plan
      || editableRoles.length
      || editablePages.length
      || planDraft.summary
      || planDraft.outlineText,
    );
    if (hasGenerated && !window.confirm("修改需求会清空已生成的方案、角色和分页内容，确定继续吗？")) {
      return;
    }
    setForm((current) => ({ ...current, ...patch }));
    setGenerationOutputs({});
    setPlanDraft({ summary: "", outlineText: "", roleRequirementsText: "", reviewPointsText: "" });
    setEditableRoles([]);
    setEditablePages([]);
    setCreatedBookId(null);
    setUnlockedStep(0);
    setNotice(null);
    setRetryJob(null);
    // 需求被修改后旧绘本作废，清掉地址栏的 bookId，避免刷新又恢复旧进度。
    resumedBookIdRef.current = null;
    if (searchParams.get("bookId")) setSearchParams({}, { replace: true });
  };

  useEffect(() => {
    getWorkspaceGenerationProvider(workspace.id).then(setProvider).catch(() => setProvider(null));
  }, [workspace.id]);

  // 断线恢复：刷新后如果还有向导类生成任务在跑，恢复表单上下文并继续等待结果。
  useEffect(() => {
    if (resumeBookId) return;
    let mounted = true;
    listGenerationJobsPage(workspace.id, { limit: 10 })
      .then((page) => {
        if (!mounted) return;
        const active = page.data.find((job) => (
          ["storybook_plan", "storybook_roles", "storybook_pages"].includes(job.jobType)
          && isActiveJobStatus(job.status)
        ));
        if (!active) {
          if (!resumeBookId) {
            const latestRecoverable = page.data
              .filter((job) => (
                ["storybook_plan", "storybook_roles", "storybook_pages"].includes(job.jobType)
                && job.storybookId
                && job.status === "succeeded"
              ))
              .sort((left, right) => Date.parse(right.createdAt) - Date.parse(left.createdAt))[0];
            if (latestRecoverable?.storybookId) {
              setSearchParams({ bookId: latestRecoverable.storybookId }, { replace: true });
            }
          }
          return;
        }
        // 超过 15 分钟仍在排队/运行的任务视为僵死（与后端 stale 锁阈值一致），不再接管，避免向导被旧任务卡住。
        const createdMs = Date.parse(active.createdAt);
        if (Number.isFinite(createdMs) && Date.now() - createdMs > 15 * 60_000) return;
        const input = (active.input || {}) as Record<string, unknown>;
        setForm((current) => ({
          ...current,
          title: typeof input.title === "string" && input.title ? input.title : current.title,
          theme: typeof input.theme === "string" && input.theme ? input.theme : current.theme,
          ageGroup: typeof input.age_group === "string" && input.age_group ? input.age_group : current.ageGroup,
          pageCount: typeof input.page_count === "string" && input.page_count ? input.page_count : current.pageCount,
          useScene: typeof input.use_scene === "string" && input.use_scene ? input.use_scene : current.useScene,
          style: typeof input.style === "string" && input.style ? input.style : current.style,
          storyStyle: typeof input.story_style === "string" && input.story_style ? input.story_style : current.storyStyle,
          storyFramework: typeof input.story_framework === "string" ? input.story_framework : current.storyFramework,
        }));
        if (active.storybookId) {
          setCreatedBookId(active.storybookId);
          rememberStorybookInUrl(active.storybookId);
        }
        setGeneratingStep(active.jobType);
        setNotice({
          title: "已恢复进行中的生成任务",
          copy: `检测到未完成的${generationJobTypeLabel[active.jobType] || "生成"}任务，正在继续等待结果。任务编号：${active.id.slice(0, 8)}。`,
        });
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
        const latestByType = new Map<string, GenerationJob>();
        [...jobsPage.data]
          .sort((a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt))
          .forEach((job) => {
            if (job.storybookId !== book.id) return;
            if (!["storybook_plan", "storybook_roles", "storybook_pages"].includes(job.jobType)) return;
            if (job.status !== "succeeded" || !job.output) return;
            if (!latestByType.has(job.jobType)) latestByType.set(job.jobType, job);
          });
        const planJob = latestByType.get("storybook_plan");
        const rolesJob = latestByType.get("storybook_roles");
        const pagesJob = latestByType.get("storybook_pages");
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
        const restoredStep = pagesJob || ["editing", "image_pending", "exportable", "submitted", "listed"].includes(book.status)
          ? 3
          : rolesJob || book.status === "roles_pending"
            ? 2
            : planJob || book.status !== "draft"
              ? 1
              : 0;
        goToStep(restoredStep);
        setEditingReview(
          restoredStep === 1
            ? "plan"
            : restoredStep === 2
              ? "roles"
              : restoredStep === 3 && !["exportable", "submitted", "listed"].includes(book.status)
                ? "pages"
                : null,
        );
        setNotice({
          title: "已恢复向导进度",
          copy: pagesJob || book.pages.length
            ? "已载入上次的方案、角色和分页，可继续确认分页。"
            : rolesJob || book.roles.length
              ? "已载入上次的方案和角色，可继续确认角色并生成分页。"
              : planJob || book.status !== "draft"
                ? "已载入上次确认的绘本方案，可继续生成角色与道具。"
                : "这本绘本还没有生成记录，请从需求开始。",
        });
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
    // 先把 ref 占住，避免本次写 URL 又触发一次完整恢复、覆盖正在编辑的内容。
    resumedBookIdRef.current = createdBookId;
    setSearchParams({ bookId: createdBookId }, { replace: true });
  }, [createdBookId, resumeBookId, setSearchParams]);
  const ensureStorybookCreated = async () => {
    if (createdBookId) return createdBookId;
    setCreating(true);
    try {
      const book = await createStorybook(workspace.id, {
        title: form.title.trim() || form.theme.trim() || "新建普通绘本",
        ageGroup: form.ageGroup,
        useScene: form.useScene,
        teachingGoal: form.theme.trim() || "帮助孩子理解班级规则和生活习惯",
        coverTone: form.style.trim(),
      });
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
  ): Promise<GenerationJob | null> => {
    setGeneratingStep(jobType);
    setRetryJob(null);
    setNotice(null);
    try {
      // 每个向导生成任务都必须绑定到绘本草稿。
      // 否则第一步方案生成只存在前端内存里，刷新页面后无法从后端恢复。
      const bookId = await ensureStorybookCreated();
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
      coverTone: form.style.trim(),
    });
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
      <div className="wizard-shell">
        <WizardSideNav
          title="普通绘本流程"
          copy="先确认故事方案，再确认角色道具，最后编辑分页并导出。"
          steps={steps}
          active={step}
          maxUnlockedStep={unlockedStep}
          onSelect={(next) => { if (!generatingStep) goToStep(next); }}
        />
        <Card className="wizard-card">
          {generatingStep && ["storybook_plan", "storybook_roles", "storybook_pages"].includes(generatingStep) && (
            <div className="generation-progress">
              {([
                ["storybook_plan", "绘本方案"],
                ["storybook_roles", "角色道具"],
                ["storybook_pages", "分页图文"],
              ] as const).map(([jobType, label], index, list) => {
                const currentIndex = list.findIndex(([jt]) => jt === generatingStep);
                const state = index < currentIndex ? "done" : index === currentIndex ? "active" : "pending";
                return (
                  <div key={jobType} className={`generation-progress-step ${state}`}>
                    <span className="generation-progress-dot">{state === "done" ? "✓" : index + 1}</span>
                    <span>{label}{state === "active" ? " 生成中…" : ""}</span>
                  </div>
                );
              })}
              <span className="generation-progress-hint">生成完成后可继续编辑</span>
            </div>
          )}
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
              <label>绘本标题<input value={form.title} disabled={Boolean(generatingStep)} onChange={(event) => updateRequestForm({ title: event.target.value })} /></label>
              <label>绘本主题<input value={form.theme} disabled={Boolean(generatingStep)} onChange={(event) => updateRequestForm({ theme: event.target.value })} /></label>
              <label>年龄段<select value={form.ageGroup} disabled={Boolean(generatingStep)} onChange={(event) => updateRequestForm({ ageGroup: event.target.value })}><option>3-4 岁</option><option>4-5 岁</option><option>5-6 岁</option></select></label>
              <label>页数<input type="number" value={form.pageCount} disabled={Boolean(generatingStep)} onChange={(event) => updateRequestForm({ pageCount: event.target.value })} /></label>
              <label>使用场景<select value={form.useScene} disabled={Boolean(generatingStep)} onChange={(event) => updateRequestForm({ useScene: event.target.value })}>
                <option>课堂共读</option>
                <option>规则引导</option>
                <option>家园沟通</option>
                <option>入园适应</option>
                <option>睡前故事</option>
                <option>情绪管理</option>
                <option>安全教育</option>
                <option>健康与生活自理</option>
                <option>节日与节气活动</option>
                <option>户外探索</option>
                <option>区域活动延伸</option>
              </select></label>
              <div className="span-2">
                <span className="field-label">画面风格</span>
                <div className="style-preset-grid">
                  {(styleCardsExpanded ? STYLE_PRESETS : STYLE_PRESETS.slice(0, 6)).map((preset) => (
                    <button
                      key={preset.label}
                      type="button"
                      className={`style-preset ${form.style === preset.value ? "active" : ""}`}
                      disabled={Boolean(generatingStep)}
                      onClick={() => updateRequestForm({ style: preset.value })}
                    >
                      <img src={preset.image} alt={preset.label} loading="lazy" />
                      <span className="style-preset-caption">
                        <strong>{preset.label}</strong>
                        <em>{preset.tag}</em>
                      </span>
                      {form.style === preset.value && <span className="style-preset-check">✓</span>}
                    </button>
                  ))}
                </div>
                <button
                  type="button"
                  className="style-preset-toggle"
                  disabled={Boolean(generatingStep)}
                  onClick={() => setStyleCardsExpanded((v) => !v)}
                >
                  {styleCardsExpanded ? "收起风格 ▲" : `展开更多风格（共 ${STYLE_PRESETS.length} 种）▼`}
                </button>
                <textarea
                  rows={2}
                  value={form.style}
                  disabled={Boolean(generatingStep)}
                  placeholder="选择上方预设风格，或在这里直接描述想要的风格"
                  onChange={(event) => updateRequestForm({ style: event.target.value })}
                />
                <p className="form-hint">选中预设后可直接在文本框里微调，生成时会按这里的描述执行。</p>
              </div>
              <div className="span-2">
                <span className="field-label">故事风格</span>
                <div className="story-style-chips">
                  {STORY_STYLE_PRESETS.map((preset) => (
                    <button
                      key={preset.label}
                      type="button"
                      className={`story-style-chip ${form.storyStyle === preset.value ? "active" : ""}`}
                      disabled={Boolean(generatingStep)}
                      title={preset.value}
                      onClick={() => updateRequestForm({ storyStyle: preset.value })}
                    >
                      <strong>{preset.label}</strong>
                      <span>{preset.tag}</span>
                    </button>
                  ))}
                </div>
                <textarea
                  rows={2}
                  value={form.storyStyle}
                  disabled={Boolean(generatingStep)}
                  placeholder="选择上方预设故事风格，或在这里直接描述想要的情节基调"
                  onChange={(event) => updateRequestForm({ storyStyle: event.target.value })}
                />
                <p className="form-hint">故事风格决定情节基调，画面风格决定怎么画，两者可自由组合；文本框留空则由 AI 自由发挥。</p>
              </div>
              <label className="span-2">
                故事框架（可选）
                <textarea
                  rows={4}
                  value={form.storyFramework}
                  disabled={Boolean(generatingStep)}
                  placeholder={"可以简单写几句故事的走向，比如：\n开头：小猫第一次上幼儿园，紧紧抓着妈妈的手。\n经过：它不敢加入游戏，后来在小兔的邀请下一起搭积木。\n结尾：放学时小猫已经交到了两个好朋友。\n不写也可以，AI 会根据主题自由创作。"}
                  onChange={(event) => updateRequestForm({ storyFramework: event.target.value })}
                />
                <p className="form-hint">填写后，AI 会严格按你的框架展开分页；留空则由 AI 自由创作。</p>
              </label>
            </div>
          )}
          {step === 1 && <GenerationReviewBlock showMeta title="绘本方案" output={generationOutputs.storybook_plan} items={storybookPlanItems(generationOutputs.storybook_plan, form, planDraft)} regenerating={generatingStep === "storybook_plan"} onRegenerate={() => void regeneratePlanWithCascade()} onEdit={() => setEditingReview(editingReview === "plan" ? null : "plan")} editing={editingReview === "plan"} editor={<><PlanEditor form={form} plan={planDraft} onFormChange={setForm} onPlanChange={setPlanDraft} /><p className="form-hint">修改在重新生成时生效；离开页面前请点「重新生成」。</p></>} />}
          {step === 2 && <GenerationReviewBlock showMeta title="角色与关键道具" output={generationOutputs.storybook_roles} items={storybookRoleItems(generationOutputs.storybook_roles, currentRoles, planDraft, form)} regenerating={generatingStep === "storybook_roles"} onRegenerate={() => runGeneration("storybook_roles", "已重新生成角色")} onEdit={() => setEditingReview(editingReview === "roles" ? null : "roles")} editing={editingReview === "roles"} editor={<><RoleEditor roles={currentRoles.length ? currentRoles : roleDraftsFromPlan(planDraft, form)} onChange={setEditableRoles} /><p className="form-hint">修改在重新生成时生效；离开页面前请点「重新生成」。</p></>} />}
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

function PlanEditor({
  form,
  plan,
  onFormChange,
  onPlanChange,
}: {
  form: { title: string; theme: string; ageGroup: string; pageCount: string; useScene: string; style: string; storyStyle: string; storyFramework: string };
  plan: EditablePlan;
  onFormChange: (value: { title: string; theme: string; ageGroup: string; pageCount: string; useScene: string; style: string; storyStyle: string; storyFramework: string }) => void;
  onPlanChange: (value: EditablePlan) => void;
}) {
  return (
    <div className="review-editor">
      <label>绘本标题<input value={form.title} onChange={(event) => onFormChange({ ...form, title: event.target.value })} /></label>
      <label>教学目标<input value={form.theme} onChange={(event) => onFormChange({ ...form, theme: event.target.value })} /></label>
      <label className="span-2">故事概述<textarea rows={3} value={plan.summary} onChange={(event) => onPlanChange({ ...plan, summary: event.target.value })} /></label>
      <div className="span-2 outline-editor">
        <span className="field-label">分页节奏（每页一条，可逐页修改）</span>
        <OutlineLinesEditor
          text={plan.outlineText}
          onChange={(outlineText) => onPlanChange({ ...plan, outlineText })}
        />
      </div>
      <label className="span-2">角色需求<textarea rows={3} value={plan.roleRequirementsText} onChange={(event) => onPlanChange({ ...plan, roleRequirementsText: event.target.value })} /></label>
      <label className="span-2">老师确认重点<textarea rows={3} value={plan.reviewPointsText} onChange={(event) => onPlanChange({ ...plan, reviewPointsText: event.target.value })} /></label>
    </div>
  );
}

/** 分页节奏逐页编辑：按行拆分，每页一个独立输入框，写回时按行合并。
 *  页数由生成结果固定：输入框内禁止换行（回车忽略、粘贴内容去掉换行），避免多出空白页。 */
function OutlineLinesEditor({ text, onChange }: { text: string; onChange: (text: string) => void }) {
  const lines = text.split("\n");
  return (
    <div className="outline-lines">
      {lines.map((line, index) => (
        <textarea
          key={index}
          rows={2}
          value={line}
          placeholder={`第 ${index + 1} 页`}
          onKeyDown={(event) => {
            if (event.key === "Enter") event.preventDefault();
          }}
          onChange={(event) => {
            const next = [...lines];
            next[index] = event.target.value.replace(/\r?\n+/g, " ");
            onChange(next.join("\n"));
          }}
        />
      ))}
    </div>
  );
}

function RoleEditor({ roles, onChange }: { roles: EditableRole[]; onChange: (roles: EditableRole[]) => void }) {
  const sortedRoleEntries = sortRoleEntriesByImportance(roles);
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const selectedEntry = sortedRoleEntries.find((entry) => entry.index === selectedIndex) || sortedRoleEntries[0];
  const selectedRole = selectedEntry?.role;
  const activeIndex = selectedEntry?.index ?? 0;
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
      </section>
    </div>
  );
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

function roleTypeLabel(roleType: StorybookRole["roleType"]) {
  const labels: Record<StorybookRole["roleType"], string> = {
    protagonist: "主角",
    supporting: "配角",
    peer: "同伴角色",
    teacher: "老师形象",
    prop: "关键道具",
  };
  return labels[roleType] || "角色";
}

function PageEditor({ pages, roles, onChange }: { pages: EditablePage[]; roles: EditableRole[]; onChange: (pages: EditablePage[]) => void }) {
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
  // 可选故事风格：情节基调，只在用户选择时传给 AI。
  if (form.storyStyle?.trim()) {
    base.story_style = form.storyStyle.trim();
  }
  // 可选故事框架：只在用户填写时传给 AI。
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
