import { ArrowRight, CheckCircle2, Copy, Download, MoreHorizontal, Pencil, Send } from "lucide-react";
import { ChangeEvent, FormEvent, type ReactNode, useEffect, useState } from "react";
import { Link, useLocation, useNavigate, useOutletContext, useParams } from "react-router-dom";
import {
  cancelGenerationJob,
  createGenerationJob,
  createPageImageTask,
  createRoleReferenceImageTask,
  createShareLink,
  createStorybookExport,
  deleteStorybook,
  downloadGenerationImageFile,
  downloadStorybookExportFile,
  duplicateStorybook,
  getStorybookExport,
  getStorybook,
  listShareLinksPage,
  listStorybookGenerationJobs,
  listStorybookExportsPage,
  revokeShareLink,
  retryGenerationJob,
  updateStorybook,
  updateStorybookPage,
  updateStorybookRole,
  type ExportJob,
  type GenerationJob,
  type ShareLink,
} from "../../api/client";
import { ActionButton, Badge, Card, ImageLightbox, Modal, Notice, PageHeader, SkeletonBlock, Toast, statusTone } from "../../components/ui";
import type { Storybook, StorybookQualityReport, StorybookRole, Workspace } from "../../types/domain";
import { absoluteAppUrl, copyText } from "../../utils/clipboard";
import { cacheImagePreview, getCachedImagePreview } from "../../utils/imagePreviewCache";
import {
  generationErrorMessage,
  generationStatusLabel,
  isActiveJobStatus,
  pollGenerationJob,
  pollUntilSettled,
} from "../../utils/generation";
import {
  generationJobNextAction,
  generationJobTypeLabel,
  pageStatusLabel,
  storybookNextAction,
  storybookSourceLabel,
} from "../../utils/labels";

export function StorybookDetailPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const { storybookId } = useParams();
  const navigate = useNavigate();
  const location = useLocation();
  const [remoteBook, setRemoteBook] = useState<Storybook | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const book = remoteBook;
  const [selectedPageId, setSelectedPageId] = useState<string | undefined>(undefined);
  const [pageForm, setPageForm] = useState({ title: "", body: "", illustrationPrompt: "" });
  const [notice, setNotice] = useState<{ title: string; copy: string; tone?: "good" | "info"; action?: ReactNode } | null>(null);
  const [retryImageJob, setRetryImageJob] = useState<GenerationJob | null>(null);
  const [generationJobs, setGenerationJobs] = useState<GenerationJob[]>([]);
  const [cancelingJobId, setCancelingJobId] = useState<string | null>(null);
  const [zoomedImage, setZoomedImage] = useState<{ src: string; alt: string } | null>(null);
  const [exportJobs, setExportJobs] = useState<ExportJob[]>([]);
  const [shareOpen, setShareOpen] = useState(false);
  const [shareLinks, setShareLinks] = useState<ShareLink[]>([]);
  const [shareSaving, setShareSaving] = useState(false);
  const [revokingShareId, setRevokingShareId] = useState<string | null>(null);
  const [createdShareUrl, setCreatedShareUrl] = useState<string | null>(null);
  const [shareExpiry, setShareExpiry] = useState<"7d" | "30d" | "never">("7d");
  const [exporting, setExporting] = useState(false);
  const [duplicating, setDuplicating] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [moreMenuOpen, setMoreMenuOpen] = useState(false);
  const [duplicateOpen, setDuplicateOpen] = useState(false);
  const [duplicateTitle, setDuplicateTitle] = useState("");
  const [deliverySaving, setDeliverySaving] = useState(false);
  const [reviewSaving, setReviewSaving] = useState(false);
  const [metaOpen, setMetaOpen] = useState(false);
  const [metaSaving, setMetaSaving] = useState(false);
  const [metaForm, setMetaForm] = useState({
    title: "",
    ageGroup: "4-5 岁",
    useScene: "",
    teachingGoal: "",
    coverTone: "",
  });
  const [imageGenerating, setImageGenerating] = useState(false);
  // 记录正在重写插图描述的页面 ID，避免切换绘本/分页后按钮状态残留。
  const [promptRewritingPageId, setPromptRewritingPageId] = useState<string | null>(null);
  const [currentImagePreviewUrl, setCurrentImagePreviewUrl] = useState("");
  const [currentImagePreviewError, setCurrentImagePreviewError] = useState("");
  const [visibilitySaving, setVisibilitySaving] = useState(false);
  const [visibilityValue, setVisibilityValue] = useState<Storybook["visibility"]>("private");
  const [pageEditorOpen, setPageEditorOpen] = useState(false);
  const [roleManagerOpen, setRoleManagerOpen] = useState(false);
  const [selectedRoleId, setSelectedRoleId] = useState<string | undefined>(undefined);
  const [roleForm, setRoleForm] = useState<{
    name: string;
    roleType: StorybookRole["roleType"];
    appearance: string;
    storyFunction: string;
    needsConsistency: boolean;
  }>({ name: "", roleType: "teacher", appearance: "", storyFunction: "", needsConsistency: true });
  const [roleSaving, setRoleSaving] = useState(false);
  const [roleImageGenerating, setRoleImageGenerating] = useState(false);
  const [roleReferencePreviewUrl, setRoleReferencePreviewUrl] = useState("");
  const [roleReferencePreviewError, setRoleReferencePreviewError] = useState("");
  const selectedPage = book?.pages.find((page) => page.id === selectedPageId) || book?.pages[0];
  const selectedRole = book?.roles.find((role) => role.id === selectedRoleId) || book?.roles[0];
  const selectedRolePageCount = book && selectedRole ? rolePageUsageCount(book, selectedRole) : 0;
  const selectedRoleNeedsReference = Boolean(selectedRole?.needsConsistency && selectedRolePageCount >= 2);
  const selectedRoleReferenceJob = selectedRole ? activeRoleReferenceJob(generationJobs, selectedRole.id) : undefined;
  const selectedRoleReferenceGenerating = roleImageGenerating || Boolean(selectedRoleReferenceJob);
  const roleReferencePromptPreview = buildRoleReferencePrompt(roleForm, book?.coverTone || "");
  const pageHasUnsavedChanges = selectedPage
    ? pageForm.title !== selectedPage.title
      || pageForm.body !== selectedPage.body
      || pageForm.illustrationPrompt !== selectedPage.illustrationPrompt
    : false;
  const deliveryBlockers = book ? [
    ...(book.pages.length ? [] : ["至少需要一个分页"]),
    ...(book.roles.length ? [] : ["至少需要一个角色或道具设定"]),
    ...(book.pages.some((page) => page.status === "generating") ? ["仍有插图正在生成"] : []),
    ...(book.pages.some((page) => page.status === "failed") ? ["存在插图生成失败的分页"] : []),
  ] : [];
  const deliveryWarnings = book ? [
    ...(book.pages.some((page) => page.status === "needs_regeneration") ? ["有页面需要重绘，可先交付文字版，也建议稍后补图"] : []),
  ] : [];
  const canDeliver =
    Boolean(book && book.id === storybookId && (book.status === "exportable" || book.status === "listed"));
  const canMarkDeliverable =
    Boolean(book && book.id === storybookId && (book.status === "editing" || book.status === "image_pending") && deliveryBlockers.length === 0);
  const quality = book ? book.quality || buildLocalStorybookQuality(book) : undefined;
  const reviewDeliveryReminder = book && book.teacherReviewStatus !== "confirmed"
    ? "这本绘本还没有老师复核记录，建议先点击“老师已复核”后再交付；如需演示仍可继续导出或分享。"
    : "";
  const qualityDeliveryBlocker = quality?.status === "blocked"
    ? "生成质量检查存在阻断项，请先修正后再导出或创建分享链接。"
    : "";
  const effectiveDeliveryBlocker = qualityDeliveryBlocker || deliveryBlockers[0] || "";
  const canStartDelivery = canDeliver && !qualityDeliveryBlocker;
  const customizationBlocker = book ? customizationBlockerFor(book, quality) : "请等待当前绘本加载完成";
  const canCreateCustomVersion = book?.type === "plain" && !customizationBlocker;
  const selectedPageQuality = selectedPage && quality
    ? quality.pages.find((page) => page.pageId === selectedPage.id)
    : undefined;
  const firstActionableQualityPage = quality?.pages.find((page) => page.status === "blocked")
    || quality?.pages.find((page) => page.status === "needs_review");
  const firstRoleNeedingReference = book?.roles.find((role) => roleNeedsReference(book, role) && (role.referenceStatus !== "ready" || !role.referenceImageUrl));
  const selectedPageReferenceText = selectedPage
    ? `${pageForm.title || selectedPage.title} ${pageForm.body || selectedPage.body} ${pageForm.illustrationPrompt || selectedPage.illustrationPrompt}`
    : "";
  const selectedPageReferencedRoles = book?.roles.filter((role) => roleNeedsReference(book, role) && selectedPageReferenceText.includes(role.name)) || [];
  const selectedPageUsableReferenceRoles = selectedPageReferencedRoles.filter((role) => role.referenceImageUrl);
  const selectedPageMissingReferenceRoles = selectedPageReferencedRoles.filter((role) => !role.referenceImageUrl);
  const selectedPageStaleReferenceRoles = selectedPageReferencedRoles.filter((role) => role.referenceImageUrl && role.referenceStatus !== "ready");
  const pageImageReferenceBlocker = selectedPageMissingReferenceRoles.length
    ? `本页提到了 ${selectedPageMissingReferenceRoles.map((role) => role.name).join("、")}，请先生成角色参考图再生成插图。`
    : "";
  const routeResultNotice = resultNoticeFromSearch(location.search);
  const visibleNotice = notice || routeResultNotice;

  useEffect(() => {
    if (!storybookId) return;
    let mounted = true;
    setLoading(true);
    setRemoteBook(null);
    setShareLinks([]);
    setExportJobs([]);
    setGenerationJobs([]);
    setSelectedPageId(undefined);
    setSelectedRoleId(undefined);
    setCreatedShareUrl(null);
    setError("");
    async function load() {
      try {
        const item = await getStorybook(workspace.id, storybookId!);
        if (!mounted) return;
        setRemoteBook(item);
        setSelectedPageId(item.pages[0]?.id);
        setSelectedRoleId(item.roles[0]?.id);
        setVisibilityValue(item.visibility);
        const [linksResult, exportsResult, jobsResult] = await Promise.allSettled([
          listShareLinksPage(workspace.id, item.id, { limit: 8 }),
          listStorybookExportsPage(workspace.id, item.id, { limit: 8 }),
          listStorybookGenerationJobs(workspace.id, item.id, { limit: 50 }),
        ]);
        if (!mounted) return;
        setShareLinks(linksResult.status === "fulfilled" ? linksResult.value.data : []);
        setExportJobs(exportsResult.status === "fulfilled" ? exportsResult.value.data : []);
        setGenerationJobs(jobsResult.status === "fulfilled" ? jobsResult.value : []);
        setError("");
      } catch (err) {
        if (!mounted) return;
        setRemoteBook(null);
        setShareLinks([]);
        setExportJobs([]);
        setGenerationJobs([]);
        setError(err instanceof Error ? err.message : "无法读取绘本详情");
      } finally {
        if (mounted) setLoading(false);
      }
    }
    load();
    return () => {
      mounted = false;
    };
  }, [storybookId, workspace.id]);

  useEffect(() => {
    if (!selectedPage) return;
    setPageForm({
      title: selectedPage.title,
      body: selectedPage.body,
      illustrationPrompt: selectedPage.illustrationPrompt,
    });
    setPageEditorOpen(false);
  }, [selectedPage?.id]);

  useEffect(() => {
    if (!book) return;
    setVisibilityValue(book.visibility);
    setMetaForm({
      title: book.title,
      ageGroup: book.ageGroup,
      useScene: book.useScene,
      teachingGoal: book.teachingGoal,
      coverTone: book.coverTone,
    });
  }, [book?.id, book?.title, book?.visibility, book?.ageGroup, book?.useScene, book?.teachingGoal, book?.coverTone]);

  useEffect(() => {
    if (!selectedRole) return;
    setRoleForm({
      name: selectedRole.name,
      roleType: selectedRole.roleType,
      appearance: cleanVisualAppearance(selectedRole.appearance),
      storyFunction: selectedRole.storyFunction,
      needsConsistency: selectedRole.needsConsistency,
    });
  }, [selectedRole?.id]);

  function updatePageForm(event: ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) {
    setPageForm((current) => ({ ...current, [event.target.name]: event.target.value }));
  }

  function cancelPageEdit() {
    if (!selectedPage) return;
    setPageForm({
      title: selectedPage.title,
      body: selectedPage.body,
      illustrationPrompt: selectedPage.illustrationPrompt,
    });
    setPageEditorOpen(false);
  }

  function updateRoleForm(event: ChangeEvent<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>) {
    const { name, value } = event.target;
    setRoleForm((current) => ({ ...current, [name]: value }));
  }

  function focusQualityPage(page: StorybookQualityReport["pages"][number]) {
    setSelectedPageId(page.pageId);
    setNotice({
      title: `已定位到第 ${page.pageNumber} 页`,
      copy: page.issues[0] || page.suggestions[0] || "请在下方检查正文、插图描述和插图生成状态。",
      tone: "info",
    });
    window.setTimeout(() => document.getElementById("storybook-page-editor")?.scrollIntoView({ behavior: "smooth", block: "start" }), 0);
  }

  function focusRoleReference(role: StorybookRole) {
    setSelectedRoleId(role.id);
    setRoleManagerOpen(true);
  }

  async function refreshShareLinks(storybookId = book?.id) {
    if (!storybookId) return;
    setShareLinks((await listShareLinksPage(workspace.id, storybookId, { limit: 8 })).data);
  }

  async function refreshExportJobs(storybookId = book?.id) {
    if (!storybookId) return;
    setExportJobs((await listStorybookExportsPage(workspace.id, storybookId, { limit: 8 })).data);
  }

  async function refreshGenerationJobs(storybookId = book?.id) {
    if (!storybookId) return;
    // 当前页插图是从任务列表里按 page_id 反查的，窗口太小时旧插图任务会掉出列表导致图"消失"。
    setGenerationJobs(await listStorybookGenerationJobs(workspace.id, storybookId, { limit: 50 }));
  }

  async function refreshStorybook(storybookId = book?.id) {
    if (!storybookId) return undefined;
    const updated = await getStorybook(workspace.id, storybookId);
    setRemoteBook(updated);
    setSelectedPageId((current) => current && updated.pages.some((page) => page.id === current) ? current : updated.pages[0]?.id);
    setSelectedRoleId((current) => current && updated.roles.some((role) => role.id === current) ? current : updated.roles[0]?.id);
    setVisibilityValue(updated.visibility);
    return updated;
  }

  const currentPageImageJob = latestPageImageJob(generationJobs, selectedPage?.id);
  const activeCurrentPageImageJob = activePageImageJob(generationJobs, selectedPage?.id);
  const currentPageImage = extractImageResult(currentPageImageJob?.output);
  const imageActionBusy = imageGenerating || Boolean(activeCurrentPageImageJob);
  const shouldShowImageGenerationAction = Boolean(selectedPage);
  const promptRewriting = promptRewritingPageId !== null && promptRewritingPageId === selectedPage?.id;

  useEffect(() => {
    if (!currentPageImage) {
      setCurrentImagePreviewUrl("");
      setCurrentImagePreviewError("");
      return;
    }
    if (!currentPageImageJob) {
      setCurrentImagePreviewUrl(currentPageImage.imageUrl);
      setCurrentImagePreviewError("");
      return;
    }
    const jobId = currentPageImageJob.id;
    const cached = getCachedImagePreview(jobId);
    if (cached) {
      setCurrentImagePreviewUrl(cached);
      setCurrentImagePreviewError("");
      return;
    }
    let active = true;
    setCurrentImagePreviewUrl("");
    setCurrentImagePreviewError("");
    downloadGenerationImageFile(workspace.id, jobId)
      .then((file) => {
        if (!active) return;
        const url = window.URL.createObjectURL(file);
        cacheImagePreview(jobId, url);
        setCurrentImagePreviewUrl(url);
      })
      .catch((err) => {
        if (active) {
          setCurrentImagePreviewUrl("");
          setCurrentImagePreviewError(err instanceof Error ? err.message : "插图文件读取失败");
        }
      });
    return () => {
      active = false;
    };
  }, [currentPageImage?.imageUrl, currentPageImageJob?.id, workspace.id]);

  // 当前页有进行中的插图任务时统一轮询；页面切后台自动暂停，完成后刷新绘本与质量检查。
  useEffect(() => {
    if (!book?.id || !selectedPage?.id || !activeCurrentPageImageJob) return;

    let active = true;
    const currentPageId = selectedPage.id;
    pollGenerationJob(workspace.id, activeCurrentPageImageJob, {
      timeoutMs: 300_000,
      onUpdate: (job) => {
        if (!active) return;
        setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
      },
    })
      .then(async (job) => {
        if (!active || isActiveJobStatus(job.status)) return;
        await refreshGenerationJobs(book.id);
        await refreshStorybook(book.id);
        setSelectedPageId(currentPageId);
        if (job.status === "failed") {
          setRetryImageJob(job);
          setNotice({ title: "插图生成失败", copy: `${generationErrorMessage(job)}。任务编号：${job.id.slice(0, 8)}。`, tone: "info" });
          return;
        }
        setRetryImageJob(null);
        setNotice({ title: "真实插图已生成", copy: `任务${generationStatusLabel(job.status)}，当前页结果已刷新。`, tone: "good" });
      })
      .catch((err) => {
        if (active) {
          setNotice({ title: "插图状态刷新失败", copy: err instanceof Error ? err.message : "请稍后手动刷新页面", tone: "info" });
        }
      });
    return () => {
      active = false;
    };
  }, [activeCurrentPageImageJob?.id, book?.id, selectedPage?.id, workspace.id]);

  useEffect(() => {
    if (!selectedRole?.referenceImageUrl) {
      setRoleReferencePreviewUrl("");
      setRoleReferencePreviewError("");
      return;
    }
    const referenceJobId = generationJobIdFromImageUrl(selectedRole.referenceImageUrl);
    if (!referenceJobId) {
      setRoleReferencePreviewUrl("");
      setRoleReferencePreviewError("角色参考图地址缺少生成任务编号");
      return;
    }
    const cached = getCachedImagePreview(referenceJobId);
    if (cached) {
      setRoleReferencePreviewUrl(cached);
      setRoleReferencePreviewError("");
      return;
    }

    let active = true;
    setRoleReferencePreviewUrl("");
    setRoleReferencePreviewError("");
    downloadGenerationImageFile(workspace.id, referenceJobId)
      .then((file) => {
        if (!active) return;
        const url = window.URL.createObjectURL(file);
        cacheImagePreview(referenceJobId, url);
        setRoleReferencePreviewUrl(url);
      })
      .catch((err) => {
        if (active) {
          setRoleReferencePreviewUrl("");
          setRoleReferencePreviewError(err instanceof Error ? err.message : "角色参考图读取失败");
        }
      });
    return () => {
      active = false;
    };
  }, [selectedRole?.id, selectedRole?.referenceImageUrl, workspace.id]);

  async function savePage() {
    if (!selectedPage || !storybookId) return;
    try {
      const updated = await updateStorybookPage(workspace.id, storybookId, selectedPage.id, {
        title: pageForm.title,
        body: pageForm.body,
        illustrationPrompt: pageForm.illustrationPrompt,
      });
      await refreshGenerationJobs(storybookId);
      await refreshStorybook(storybookId);
      setNotice({ title: "当前页已保存", copy: `第 ${updated.pageNumber} 页修改已写入后端。`, tone: "good" });
      setPageEditorOpen(false);
      setRetryImageJob(null);
    } catch (err) {
      setNotice({ title: "保存失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    }
  }

  async function saveVisibility() {
    if (!book) return;
    setVisibilitySaving(true);
    try {
      const updated = await updateStorybook(workspace.id, book.id, { visibility: visibilityValue });
      setRemoteBook(updated);
      setNotice({ title: "共享设置已保存", copy: `《${updated.title}》当前可见性：${visibilityLabel(updated.visibility)}。`, tone: "good" });
      setRetryImageJob(null);
    } catch (err) {
      setNotice({ title: "共享设置失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setVisibilitySaving(false);
    }
  }

  async function saveRole() {
    if (!book || !selectedRole) return;
    setRoleSaving(true);
    try {
      const updated = await updateStorybookRole(workspace.id, book.id, selectedRole.id, {
        name: roleForm.name,
        roleType: roleForm.roleType,
        appearance: cleanVisualAppearance(roleForm.appearance),
        storyFunction: roleForm.storyFunction,
        needsConsistency: roleForm.needsConsistency,
        referenceImagePrompt: buildRoleReferencePrompt(roleForm, book.coverTone),
      });
      await refreshGenerationJobs(book.id);
      await refreshStorybook(book.id);
      setNotice({ title: "角色设定已保存", copy: `${updated.name} 的外观设定已写入后端，参考图提示词会自动跟随外观更新。`, tone: "good" });
      setRetryImageJob(null);
    } catch (err) {
      setNotice({ title: "角色保存失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setRoleSaving(false);
    }
  }

  async function generateRoleReferenceImage() {
    if (!book || !selectedRole) return;
    if (!selectedRoleNeedsReference) {
      setNotice({ title: "无需生成参考图", copy: `${selectedRole.name} 当前只在 ${selectedRolePageCount} 页出现，不需要单独生成角色参考图。`, tone: "info" });
      return;
    }
    setRoleImageGenerating(true);
    try {
      const job = await createRoleReferenceImageTask(workspace.id, book.id, selectedRole.id, {
        referenceImageUrls: [],
        imageMode: "text_to_image",
      });
      setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
      const settledJob = await pollGenerationJob(workspace.id, job, {
        timeoutMs: 300_000,
        onUpdate: (current) => setGenerationJobs((jobs) => [current, ...jobs.filter((item) => item.id !== current.id)]),
      });
      await refreshGenerationJobs(book.id);
      if (settledJob.status === "queued" || settledJob.status === "running") {
        setNotice({
          title: "参考图仍在生成",
          copy: `任务${generationStatusLabel(settledJob.status)}，完成前会持续显示为生成中。任务编号：${settledJob.id.slice(0, 8)}。`,
          tone: "info",
        });
        return;
      }
      const updated = await getStorybook(workspace.id, book.id);
      setRemoteBook(updated);
      const updatedRole = updated.roles.find((role) => role.id === selectedRole.id);
      if (settledJob.status === "failed") {
        setNotice({
          title: "角色参考图生成失败",
          copy: `${generationErrorMessage(settledJob)}。任务编号：${settledJob.id.slice(0, 8)}。`,
          tone: "info",
        });
        return;
      }
      if (!updatedRole?.referenceImageUrl || updatedRole.referenceStatus !== "ready") {
        setNotice({
          title: "参考图已完成，等待写回",
          copy: "生图任务已结束，但角色参考图状态还没有刷新完成，请稍后重新打开角色管理查看。",
          tone: "info",
        });
        return;
      }
      setNotice({
        title: "角色参考图已生成",
        copy: `${updatedRole.name} 的参考图已写回角色，后续插图会优先引用。`,
        tone: "good",
      });
    } catch (err) {
      setNotice({ title: "角色参考图生成失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setRoleImageGenerating(false);
    }
  }

  async function rewritePagePrompt() {
    if (!book || !selectedPage) return;
    if (pageHasUnsavedChanges
      && !window.confirm("本页还有未保存的修改，AI 重写会基于已保存的正文生成新插图描述，未保存的修改可能被覆盖。仍要继续吗？")) {
      return;
    }
    setPromptRewritingPageId(selectedPage.id);
    try {
      const job = await createGenerationJob(workspace.id, {
        jobType: "storybook_page_prompt",
        storybookId: book.id,
        input: { page_id: selectedPage.id },
      });
      setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
      const settledJob = await pollGenerationJob(workspace.id, job, {
        timeoutMs: 240_000,
        onUpdate: (current) => setGenerationJobs((jobs) => [current, ...jobs.filter((item) => item.id !== current.id)]),
      });
      await refreshGenerationJobs(book.id);
      if (settledJob.status === "queued" || settledJob.status === "running") {
        setNotice({
          title: "插图描述仍在重写",
          copy: `任务${generationStatusLabel(settledJob.status)}，完成后可刷新查看。任务编号：${settledJob.id.slice(0, 8)}。`,
          tone: "info",
        });
        return;
      }
      if (settledJob.status === "failed") {
        setNotice({
          title: "插图描述重写失败",
          copy: `${generationErrorMessage(settledJob)}。任务编号：${settledJob.id.slice(0, 8)}。`,
          tone: "info",
        });
        return;
      }
      const updated = await getStorybook(workspace.id, book.id);
      setRemoteBook(updated);
      const updatedPage = updated.pages.find((page) => page.id === selectedPage.id);
      if (updatedPage) {
        setPageForm({
          title: updatedPage.title,
          body: updatedPage.body,
          illustrationPrompt: updatedPage.illustrationPrompt,
        });
      }
      setNotice({
        title: "插图描述已重写",
        copy: updatedPage?.status === "needs_regeneration"
          ? "新的插图描述已写回本页，本页插图已标记为待重新生成，确认后点击重绘插图即可。"
          : "新的插图描述已写回本页，确认后可生成插图。",
        tone: "good",
      });
    } catch (err) {
      setNotice({ title: "插图描述重写失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setPromptRewritingPageId(null);
    }
  }

  async function generateIllustration() {
    if (!book || !selectedPage) return;
    if (pageImageReferenceBlocker) {
      setNotice({ title: "先补齐角色参考图", copy: pageImageReferenceBlocker, tone: "info" });
      if (selectedPageMissingReferenceRoles[0]) focusRoleReference(selectedPageMissingReferenceRoles[0]);
      return;
    }
    setImageGenerating(true);
    setRetryImageJob(null);
    try {
      const referenceRoles = book.roles.filter((role) => role.needsConsistency && role.referenceImageUrl);
      const job = await createPageImageTask(workspace.id, book.id, selectedPage.id, {
        prompt: pageForm.illustrationPrompt,
        referenceRoleIds: referenceRoles.map((role) => role.id),
        imageMode: referenceRoles.length ? "reference_image" : "text_to_image",
      });
      // 交给统一的轮询 effect 跟踪完成状态，避免这里再重复轮询。
      setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
      setNotice({
        title: "真实插图生成已开始",
        copy: `当前页已加入生图队列，完成后这里会自动刷新。任务编号：${job.id.slice(0, 8)}。`,
        tone: "info",
      });
    } catch (err) {
      setNotice({ title: "插图生成失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setImageGenerating(false);
    }
  }

  async function retryIllustration() {
    if (!book || !selectedPage || !retryImageJob) return;
    setImageGenerating(true);
    setNotice(null);
    try {
      const job = await retryGenerationJob(workspace.id, retryImageJob.id);
      setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
      setRetryImageJob(null);
    } catch (err) {
      setNotice({ title: "插图重试失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setImageGenerating(false);
    }
  }

  async function cancelJob(job: GenerationJob) {
    if (!book) return;
    setCancelingJobId(job.id);
    try {
      const canceled = await cancelGenerationJob(workspace.id, job.id);
      setGenerationJobs((jobs) => jobs.map((item) => item.id === canceled.id ? canceled : item));
      setNotice({ title: "已取消生成任务", copy: "这条生成任务不会继续执行，可以按需重新发起生成。", tone: "good" });
    } catch (err) {
      setNotice({ title: "取消失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setCancelingJobId(null);
    }
  }

  async function waitForExportJob(storybookId: string, initialJob: ExportJob) {
    return pollUntilSettled(
      () => getStorybookExport(workspace.id, storybookId, initialJob.id),
      initialJob,
      {
        timeoutMs: 120_000,
        onUpdate: (job) => setExportJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]),
      },
    );
  }

  async function exportPdf() {
    if (!book) return;
    if (!canDeliver) {
      setNotice({ title: "还不能导出", copy: "请先完成编辑并将绘本标记为可交付，再创建 PDF 导出。", tone: "info" });
      return;
    }
    if (qualityDeliveryBlocker) {
      setNotice({ title: "暂不能导出", copy: qualityDeliveryBlocker, tone: "info" });
      return;
    }
    setExporting(true);
    try {
      const job = await createStorybookExport(workspace.id, book.id);
      const settledJob = await waitForExportJob(book.id, job);
      await refreshExportJobs(book.id);
      await refreshGenerationJobs(book.id);
      setNotice({
        title: settledJob.status === "failed" ? "PDF 导出失败" : settledJob.status === "succeeded" ? "PDF 导出已完成" : "PDF 导出任务已创建",
        copy: settledJob.fileUrl
          ? `导出文件：${settledJob.fileUrl}。这表示后端已经生成了可下载 PDF。${reviewDeliveryReminder ? ` ${reviewDeliveryReminder}` : ""}`
          : settledJob.status === "failed"
            ? exportFailureText(settledJob)
            : `任务状态：${exportStatusLabel(settledJob.status)}。导出完成后会生成可下载文件。${reviewDeliveryReminder ? ` ${reviewDeliveryReminder}` : ""}`,
        tone: settledJob.status === "failed" ? "info" : "good",
      });
      setRetryImageJob(null);
    } catch (err) {
      setNotice({ title: "导出失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setExporting(false);
    }
  }

  async function duplicateCurrentStorybook() {
    if (!book) return;
    const title = duplicateTitle.trim();
    if (!title) {
      setNotice({ title: "副本名称不能为空", copy: "请先填写一个便于后续识别的副本名称。", tone: "info" });
      return;
    }
    setDuplicating(true);
    try {
      const duplicated = await duplicateStorybook(workspace.id, book.id, { title });
      setRemoteBook(duplicated);
      setSelectedPageId(duplicated.pages[0]?.id);
      setSelectedRoleId(duplicated.roles[0]?.id);
      setShareLinks([]);
      setExportJobs([]);
      setGenerationJobs([]);
      setCreatedShareUrl(null);
      setDuplicateOpen(false);
      setDuplicateTitle("");
      navigate(`/app/${workspace.id}/storybooks/${duplicated.id}`);
    } catch (err) {
      setNotice({ title: "复制失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setDuplicating(false);
    }
  }

  async function markDeliverable() {
    if (!book) return;
    if (deliveryBlockers.length) {
      setNotice({ title: "暂不能标记可交付", copy: deliveryBlockers.join("；"), tone: "info" });
      return;
    }
    setDeliverySaving(true);
    try {
      const updated = await updateStorybook(workspace.id, book.id, { status: "exportable" });
      setRemoteBook(updated);
      setNotice({ title: "绘本已标记可交付", copy: `《${updated.title}》现在可导出 PDF，也可作为定制绘本母本。`, tone: "good" });
    } catch (err) {
      setNotice({ title: "状态更新失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setDeliverySaving(false);
    }
  }

  async function saveTeacherReview(status: "pending" | "confirmed") {
    if (!book) return;
    if (status === "confirmed" && quality?.status === "blocked") {
      setNotice({ title: "暂不能确认复核", copy: "生成质量检查仍有阻断项，请先修正分页、角色或插图问题。", tone: "info" });
      return;
    }
    setReviewSaving(true);
    try {
      const updated = await updateStorybook(workspace.id, book.id, { teacherReviewStatus: status });
      setRemoteBook(updated);
      setNotice({
        title: status === "confirmed" ? "老师复核已确认" : "已重新设为待复核",
        copy: status === "confirmed"
          ? "系统已记录这次人工复核。后续修改分页或角色后会自动回到待复核。"
          : "这本绘本会重新进入老师复核队列。",
        tone: "good",
      });
    } catch (err) {
      setNotice({ title: "复核状态保存失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setReviewSaving(false);
    }
  }

  async function saveMetadata(event: FormEvent) {
    event.preventDefault();
    if (!book) return;
    setMetaSaving(true);
    try {
      const updated = await updateStorybook(workspace.id, book.id, {
        title: metaForm.title,
        ageGroup: metaForm.ageGroup,
        useScene: metaForm.useScene,
        teachingGoal: metaForm.teachingGoal,
        coverTone: metaForm.coverTone,
      });
      setRemoteBook(updated);
      setMetaOpen(false);
      setNotice({ title: "绘本信息已保存", copy: `《${updated.title}》的年龄段、场景、目标和封面风格已更新。`, tone: "good" });
    } catch (err) {
      setNotice({ title: "信息保存失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setMetaSaving(false);
    }
  }

  async function openExportPdf(job: ExportJob) {
    if (!book) return;
    try {
      const file = await downloadStorybookExportFile(workspace.id, book.id, job.id);
      const url = window.URL.createObjectURL(file);
      const opened = window.open(url, "_blank", "noopener,noreferrer");
      if (opened) {
        window.setTimeout(() => window.URL.revokeObjectURL(url), 60_000);
        setNotice({ title: "PDF 已打开", copy: "已通过当前登录态下载导出文件。", tone: "good" });
      } else {
        // 浏览器拦截了异步弹窗：保留 blob 地址，改为显式按钮打开
        setNotice({
          title: "浏览器拦截了自动打开",
          copy: "请点击右侧按钮查看 PDF。",
          tone: "info",
          action: <a className="button secondary" href={url} target="_blank" rel="noreferrer">打开 PDF</a>,
        });
      }
    } catch (err) {
      setNotice({ title: "PDF 打开失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    }
  }

  async function createShare() {
    if (!book) return;
    if (!canDeliver) {
      setNotice({ title: "还不能分享", copy: "请先完成编辑并将绘本标记为可交付，再创建家庭分享链接。", tone: "info" });
      return;
    }
    if (qualityDeliveryBlocker) {
      setNotice({ title: "暂不能分享", copy: qualityDeliveryBlocker, tone: "info" });
      return;
    }
    setShareSaving(true);
    try {
      const link = await createShareLink(workspace.id, book.id, {
        expiresAt: shareExpiryToIso(shareExpiry),
      });
      await refreshShareLinks(book.id);
      await refreshGenerationJobs(book.id);
      setCreatedShareUrl(link.url);
      setNotice({ title: "分享链接已创建", copy: `链接：${link.url}。${shareExpiryLabel(link.expiresAt)}。收到这个链接的人可以直接打开家庭分享页。${reviewDeliveryReminder ? ` ${reviewDeliveryReminder}` : ""}`, tone: "good" });
      setRetryImageJob(null);
    } catch (err) {
      setNotice({ title: "分享失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setShareSaving(false);
    }
  }

  async function revokeShare(link: ShareLink) {
    if (!book) return;
    setShareSaving(true);
    setRevokingShareId(link.id);
    try {
      await revokeShareLink(workspace.id, book.id, link.id);
      setShareLinks((current) => current.filter((item) => item.id !== link.id));
      await refreshGenerationJobs(book.id);
      setNotice({ title: "分享链接已撤回", copy: "获得旧链接的人将无法继续查看或导出这本绘本。", tone: "good" });
      setRetryImageJob(null);
    } catch (err) {
      setNotice({ title: "撤回失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setShareSaving(false);
      setRevokingShareId(null);
    }
  }

  async function copyShareUrl(link: ShareLink) {
    const fullUrl = absoluteAppUrl(link.url);
    setNotice({ title: "分享链接已准备复制", copy: fullUrl, tone: "good" });
    copyText(fullUrl).catch(() => undefined);
  }

  if (loading) {
    return (
      <div className="page-stack" aria-label="绘本加载中">
        <SkeletonBlock className="skeleton-detail-header" />
        <div className="detail-layout">
          <SkeletonBlock className="skeleton-strip" />
          <SkeletonBlock className="skeleton-detail-main" />
        </div>
      </div>
    );
  }

  if (error || !book || !selectedPage) {
    return <div className="page-stack"><Notice title="绘本详情加载失败" copy={error || "当前绘本不存在"} tone="info" /></div>;
  }

  async function confirmDeleteBook() {
    if (!book) return;
    setDeleting(true);
    try {
      await deleteStorybook(workspace.id, book.id);
      navigate("../storybooks", { replace: true });
    } catch (err) {
      setDeleting(false);
      setDeleteOpen(false);
      setNotice({ title: "删除失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    }
  }

  const scrollToWorkspace = () => {
    document.getElementById("page-workspace")?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  return (
    <div className="page-stack">
      <PageHeader
        eyebrow={book.type === "plain" ? "普通绘本详情" : "定制绘本详情"}
        title={book.title}
        copy={`${book.teachingGoal}。${storybookSourceLabel(book)}。归属：${workspace.name}`}
        actionClassName="storybook-detail-actions"
        className="storybook-detail-header"
        actions={
          <>
            {/* 主操作：按状态只保留一个 */}
            {canDeliver ? (
              <ActionButton className="button primary" disabled={exporting || !canStartDelivery} disabledHint={qualityDeliveryBlocker || reviewDeliveryReminder || (exporting ? "导出进行中" : undefined)} onClick={exportPdf}><Download size={16} />{exporting ? "导出中..." : "导出 PDF"}</ActionButton>
            ) : (book.status === "editing" || book.status === "image_pending") ? (
              <ActionButton className="button primary" disabled={deliverySaving || !canMarkDeliverable} disabledHint={deliveryBlockers.join("；") || "请等待当前绘本加载完成"} onClick={markDeliverable}><CheckCircle2 size={16} />{deliverySaving ? "确认中..." : "标记可交付"}</ActionButton>
            ) : (
              <button className="button primary" type="button" onClick={scrollToWorkspace}>继续处理分页</button>
            )}
            {/* 次操作 */}
            {canDeliver ? (
              <ActionButton className="button secondary" disabled={!canStartDelivery} disabledHint={qualityDeliveryBlocker || reviewDeliveryReminder || undefined} onClick={() => setShareOpen(true)}><Send size={16} />分享</ActionButton>
            ) : (book.status === "editing" || book.status === "image_pending") ? (
              <button className="button secondary" type="button" onClick={scrollToWorkspace}>继续处理分页</button>
            ) : null}
            {/* 其余操作收敛进更多菜单 */}
            <div className="more-menu">
              <button className="button secondary" type="button" onClick={() => setMoreMenuOpen((open) => !open)}><MoreHorizontal size={16} />更多</button>
              {moreMenuOpen && (
                <>
                  <button className="menu-overlay" type="button" aria-label="关闭菜单" onClick={() => setMoreMenuOpen(false)} />
                  <div className="more-menu-pop">
                    {book.type === "plain" && canCreateCustomVersion && (
                      <Link to="customize" onClick={() => setMoreMenuOpen(false)}>生成定制版<ArrowRight size={14} /></Link>
                    )}
                    <button type="button" onClick={() => { setMoreMenuOpen(false); setRoleManagerOpen(true); }}><Pencil size={14} />管理角色</button>
                    <button type="button" onClick={() => { setMoreMenuOpen(false); setMetaOpen(true); }}><Pencil size={14} />编辑信息</button>
                    <button type="button" disabled={duplicating} onClick={() => { setMoreMenuOpen(false); setDuplicateTitle(`${book.title} 副本`); setDuplicateOpen(true); }}><Copy size={14} />{duplicating ? "复制中..." : "复制副本"}</button>
                    <button type="button" className="danger" onClick={() => { setMoreMenuOpen(false); setDeleteOpen(true); }}>删除绘本</button>
                  </div>
                </>
              )}
            </div>
          </>
        }
      />
      {visibleNotice && !retryImageJob && (visibleNotice.tone ?? "good") === "good" && (
        <Toast title={visibleNotice.title} copy={visibleNotice.copy} onClose={() => setNotice(null)} />
      )}
      {visibleNotice && (retryImageJob || (visibleNotice.tone ?? "good") !== "good") && (
        <Notice
          title={visibleNotice.title}
          copy={visibleNotice.copy}
          tone={retryImageJob ? "danger" : visibleNotice.tone || "good"}
          action={retryImageJob ? <button className="button secondary" type="button" disabled={imageGenerating} onClick={retryIllustration}>重新生成插图</button> : visibleNotice.action}
        />
      )}

      {deleteOpen && (
        <Modal title={`删除《${book.title}》？`} onClose={() => !deleting && setDeleteOpen(false)}>
          <div className="form-stack">
            <p>删除后不可恢复：这本绘本的分页、角色、生成记录、分享链接和导出记录会一并移除。</p>
            <div className="modal-actions">
              <button className="button secondary" type="button" disabled={deleting} onClick={() => setDeleteOpen(false)}>取消</button>
              <button className="button danger" type="button" disabled={deleting} onClick={() => void confirmDeleteBook()}>{deleting ? "删除中..." : "确认删除"}</button>
            </div>
          </div>
        </Modal>
      )}

      {metaOpen && (
        <Modal title="编辑绘本信息" onClose={() => setMetaOpen(false)}>
          <form onSubmit={saveMetadata}>
            <label>绘本标题<input value={metaForm.title} onChange={(event) => setMetaForm((current) => ({ ...current, title: event.target.value }))} /></label>
            <label>年龄段<select value={metaForm.ageGroup} onChange={(event) => setMetaForm((current) => ({ ...current, ageGroup: event.target.value }))}><option>3-4 岁</option><option>4-5 岁</option><option>5-6 岁</option></select></label>
            <label>使用场景<input value={metaForm.useScene} onChange={(event) => setMetaForm((current) => ({ ...current, useScene: event.target.value }))} /></label>
            <label>教学目标<textarea rows={3} value={metaForm.teachingGoal} onChange={(event) => setMetaForm((current) => ({ ...current, teachingGoal: event.target.value }))} /></label>
            <label>封面风格<input value={metaForm.coverTone} onChange={(event) => setMetaForm((current) => ({ ...current, coverTone: event.target.value }))} /></label>
            <div className="modal-actions">
              <button className="button secondary" type="button" onClick={() => setMetaOpen(false)}>取消</button>
              <button className="button primary" type="submit" disabled={metaSaving}>{metaSaving ? "保存中" : "保存信息"}</button>
            </div>
          </form>
        </Modal>
      )}

      {duplicateOpen && (
        <Modal title="复制为新绘本" onClose={() => setDuplicateOpen(false)}>
          <form onSubmit={(event) => { event.preventDefault(); duplicateCurrentStorybook(); }}>
            <label>副本名称<input value={duplicateTitle} onChange={(event) => setDuplicateTitle(event.target.value)} /></label>
            <p className="task-summary">系统会复制分页正文、插图描述、角色设定和参考图，创建为新的私有草稿，不会覆盖当前绘本。</p>
            <div className="modal-actions">
              <button className="button secondary" type="button" onClick={() => setDuplicateOpen(false)}>取消</button>
              <button className="button primary" type="submit" disabled={duplicating}>{duplicating ? "复制中..." : "确认复制"}</button>
            </div>
          </form>
        </Modal>
      )}

      {roleManagerOpen && selectedRole && (
        <Modal title="管理整本绘本的角色与道具" className="role-manager-modal" onClose={() => setRoleManagerOpen(false)}>
          <div id="storybook-role-editor" className="role-manager-content">
            <div className="role-manager-list">
              <p className="task-summary">这些设定属于整本绘本，会影响所有分页插图的一致性。</p>
              <div className="compact-list">
                {book.roles.map((role) => (
                  <button className={`compact-row ${selectedRole?.id === role.id ? "active" : ""}`} type="button" key={role.id} onClick={() => setSelectedRoleId(role.id)}>
                    <div><strong>{role.name}</strong><span>{cleanVisualAppearance(role.appearance)}</span></div>
                    <div className="badge-stack">
                      <Badge>{roleLabelMap(role.roleType)}</Badge>
                      {roleNeedsReference(book, role) ? (
                        activeRoleReferenceJob(generationJobs, role.id) ? (
                          <Badge tone="neutral">生成中</Badge>
                        ) : (
                          <Badge tone={role.referenceStatus === "ready" ? "good" : role.referenceStatus === "failed" ? "danger" : "neutral"}>{roleReferenceStatusLabel(role.referenceStatus)}</Badge>
                        )
                      ) : (
                        <Badge tone="neutral">单页出现</Badge>
                      )}
                    </div>
                  </button>
                ))}
              </div>
            </div>
            <div className="form-stack">
              <section className="share-section">
                <h4 className="share-section-title">角色设定</h4>
                <label>角色名称<input name="name" value={roleForm.name} onChange={updateRoleForm} /></label>
                <label>
                  视觉类型
                  <select name="roleType" value={roleForm.roleType} onChange={updateRoleForm}>
                    <option value="protagonist">主角</option>
                    <option value="supporting">配角</option>
                    <option value="peer">同伴角色</option>
                    <option value="teacher">老师形象</option>
                    <option value="prop">关键道具</option>
                  </select>
                </label>
                <label>稳定外观<textarea name="appearance" rows={4} value={roleForm.appearance} onChange={updateRoleForm} /></label>
                <label className="check-row"><input type="checkbox" checked={roleForm.needsConsistency} onChange={(event) => setRoleForm((current) => ({ ...current, needsConsistency: event.target.checked }))} />跨页保持一致（出现 2 页以上才需要参考图）</label>
              </section>
              <section className="share-section">
                <h4 className="share-section-title">参考图</h4>
                <div className="reference-preview">
                {!selectedRoleNeedsReference ? (
                  <div className="reference-empty">无需参考图</div>
                ) : selectedRoleReferenceGenerating ? (
                  <div className="reference-empty">正在生成新的参考图</div>
                ) : selectedRole.referenceImageUrl ? (
                  roleReferencePreviewUrl ? (
                    <button className="image-zoom-trigger" type="button" title="点击放大查看" onClick={() => setZoomedImage({ src: roleReferencePreviewUrl, alt: `${selectedRole.name} 的角色参考图` })}>
                      <img src={roleReferencePreviewUrl} alt={`${selectedRole.name} 的角色参考图`} />
                    </button>
                  ) : roleReferencePreviewError ? (
                    <div className="reference-empty">参考图读取失败：{roleReferencePreviewError}</div>
                  ) : (
                    <div className="reference-empty">正在读取角色参考图</div>
                  )
                ) : (
                  <div className="reference-empty">待生成角色参考图</div>
                )}
                <div>
                  <Badge tone={selectedRole.referenceStatus === "ready" ? "good" : selectedRole.referenceStatus === "failed" ? "danger" : "neutral"}>
                    {selectedRoleReferenceGenerating ? "生成中" : selectedRoleNeedsReference ? roleReferenceStatusLabel(selectedRole.referenceStatus) : "单页出现"}
                  </Badge>
                  <p>{selectedRoleReferenceGenerating ? "参考图任务还在生成，完成后会写回角色并用于后续分页插图。" : selectedRoleNeedsReference ? "先确认角色参考图，再生成分页插图，可以显著提高跨页形象一致性。" : selectedRole.referenceImageUrl ? "这个角色或道具当前没有跨页重复出现，已有历史参考图不会用于分页插图。" : "这个角色或道具当前没有跨页重复出现，不需要单独生成参考图。"}</p>
                </div>
              </div>
              {selectedRoleNeedsReference ? (
                <div className="reference-prompt-preview">
                  <div>
                    <strong>参考图生成依据</strong>
                    <span>由角色名称、视觉类型和外观设定自动生成；故事作用不参与参考图，避免把剧情动作画进角色标准照。</span>
                  </div>
                  <details className="prompt-details">
                    <summary>查看完整生成提示词</summary>
                    <p>{roleReferencePromptPreview}</p>
                  </details>
                </div>
              ) : (
                <div className="reference-prompt-preview muted">
                  <div>
                    <strong>无需生成参考图</strong>
                    <span>当前只按分页里的插图描述生成画面；如果后续这个角色跨页重复出现，再开启参考图。</span>
                  </div>
                </div>
              )}
              </section>
              <section className="share-section">
                <div className="inline-actions editor-actions modal-editor-actions share-actions">
                  <button className="button secondary" type="button" disabled={roleSaving} onClick={saveRole}>{roleSaving ? "保存中..." : "保存角色设定"}</button>
                  <ActionButton className="button primary" disabled={selectedRoleReferenceGenerating || !selectedRoleNeedsReference} disabledHint={!selectedRoleNeedsReference ? "只出现一次的角色或道具不需要参考图" : "生成进行中，请稍候"} onClick={generateRoleReferenceImage}>
                    {selectedRoleReferenceGenerating ? "生成中..." : !selectedRoleNeedsReference ? "无需参考图" : selectedRole.referenceImageUrl ? "重绘参考图" : "生成参考图"}
                  </ActionButton>
                </div>
              </section>
            </div>
          </div>
        </Modal>
      )}

      <div className="workspace-section-head">
        <p className="eyebrow">本页工作台</p>
        <h2>逐页检查内容与插图</h2>
      </div>
      <section className="detail-layout" id="page-workspace">
        <aside className="page-strip">
          <h2>页面</h2>
          {book.pages.map((page) => (
            <button key={page.id} type="button" className={`page-thumb ${selectedPage.id === page.id ? "active" : ""}`} onClick={() => setSelectedPageId(page.id)}>
              <span>第 {page.pageNumber} 页</span>
              <strong>{page.title}</strong>
              <Badge tone={statusTone(page.status)}>{pageStatusLabel[page.status]}</Badge>
            </button>
          ))}
        </aside>
        <div className="storybook-workspace-main">
          <Card className="preview-panel">
            <div className="storybook-preview-art"><span>{book.coverTone}</span><strong>{book.title}</strong></div>
            <div className="page-content-toolbar">
              <span>第 {selectedPage.pageNumber} 页</span>
              {!pageEditorOpen && (
                <button className="button secondary" type="button" onClick={() => setPageEditorOpen(true)}>
                  编辑本页
                </button>
              )}
            </div>
            {pageEditorOpen ? (
              <div className="inline-page-editor">
                <label>页面标题<input name="title" value={pageForm.title} onChange={updatePageForm} /></label>
                <label>正文<textarea name="body" rows={5} value={pageForm.body} onChange={updatePageForm} /></label>
                <label>插图描述<textarea name="illustrationPrompt" rows={4} value={pageForm.illustrationPrompt} onChange={updatePageForm} /></label>
                <div className="inline-actions editor-actions contextual-actions">
                  <button className="button secondary" type="button" onClick={cancelPageEdit}>取消编辑</button>
                  <button className="button primary" type="button" disabled={!pageHasUnsavedChanges} onClick={savePage}>保存本页修改</button>
                </div>
                {pageHasUnsavedChanges && (
                  <p className="form-hint">保存本页修改后，可在当前页检查中按新描述重绘插图。</p>
                )}
              </div>
            ) : (
              <>
                <h2>{selectedPage.title}</h2>
                <p>{selectedPage.body}</p>
                <details className="prompt-details">
                  <summary>查看插图描述</summary>
                  <p>{selectedPage.illustrationPrompt}</p>
                </details>
              </>
            )}
            {activeCurrentPageImageJob ? (
              <div className="preview-image-block">
                <Badge tone="info">当前页插图任务</Badge>
                <div className="image-placeholder-note">
                  <strong>正在生成真实插图</strong>
                  <span>
                    任务{generationStatusLabel(activeCurrentPageImageJob.status)}，请稍等。任务编号：{activeCurrentPageImageJob.id.slice(0, 8)}。
                  </span>
                </div>
                <details className="prompt-details">
                  <summary>查看生成中的插图描述</summary>
                  <p>{pageForm.illustrationPrompt || selectedPage.illustrationPrompt}</p>
                </details>
                <small>完成后这里会自动刷新为真实插图。</small>
              </div>
            ) : currentPageImage && (
              <div className="preview-image-block">
                <Badge tone="info">当前页插图结果</Badge>
                {currentImagePreviewUrl ? (
                  <button className="image-zoom-trigger" type="button" title="点击放大查看" onClick={() => setZoomedImage({ src: currentImagePreviewUrl, alt: currentPageImage.altText || selectedPage.title })}>
                    <img src={currentImagePreviewUrl} alt={currentPageImage.altText || selectedPage.title} />
                  </button>
                ) : currentImagePreviewError ? (
                  <p>插图文件读取失败：{currentImagePreviewError}</p>
                ) : (
                  <p>正在读取当前登录态下的插图文件。</p>
                )}
                <details className="prompt-details">
                  <summary>查看完整生成提示词</summary>
                  <p>{currentPageImage.prompt}</p>
                </details>
                <small>{currentPageImage.styleNotes.join(" · ")}</small>
              </div>
            )}
          </Card>
          <aside className="editor-panel">
          <Card id="storybook-page-editor">
            <div className="panel-title-row">
              <div>
                <h2>当前页检查</h2>
                <p>只处理本页需要复核或重绘的内容。</p>
              </div>
            </div>
            {selectedPageQuality && selectedPageQuality.status !== "passed" && (
              <div className="quality-focus-callout">
                <Badge tone={qualityTone(selectedPageQuality.status)}>{qualityStatusLabel(selectedPageQuality.status)}</Badge>
                <div>
                  <strong>当前页检查</strong>
                  {(selectedPageQuality.issues.length > 0 || selectedPageQuality.suggestions.length > 0) ? (
                    <>
                      {selectedPageQuality.issues.map((issue) => (
                        <span key={`selected-issue-${issue}`}>问题：{issue}</span>
                      ))}
                      {selectedPageQuality.suggestions.map((suggestion) => (
                        <span key={`selected-suggestion-${suggestion}`}>建议：{suggestion}</span>
                      ))}
                    </>
                  ) : (
                    <span>请检查正文、插图描述和插图生成状态。</span>
                  )}
                </div>
              </div>
            )}
            <div className={`reference-guard-callout ${pageImageReferenceBlocker ? "needs-reference" : ""}`}>
              <Badge tone={pageImageReferenceBlocker || selectedPageStaleReferenceRoles.length ? "warn" : selectedPageUsableReferenceRoles.length ? "good" : "neutral"}>插图参考图</Badge>
              <div>
                <strong>{pageImageReferenceBlocker ? "先补齐本页角色参考图" : selectedPageStaleReferenceRoles.length ? "本页已有参考图，建议更新" : selectedPageUsableReferenceRoles.length ? "本页会引用角色参考图" : "本页未识别到需固定形象的角色"}</strong>
                {selectedPageUsableReferenceRoles.length > 0 && (
                  <span>已有参考图：{selectedPageUsableReferenceRoles.map((role) => role.name).join("、")}。</span>
                )}
                {selectedPageMissingReferenceRoles.length > 0 && (
                  <span>缺少参考图：{selectedPageMissingReferenceRoles.map((role) => role.name).join("、")}。</span>
                )}
                {selectedPageStaleReferenceRoles.length > 0 && (
                  <span>需更新参考图：{selectedPageStaleReferenceRoles.map((role) => role.name).join("、")}。当前已有图仍可用于生成，更新后跨页一致性更稳。</span>
                )}
                {!selectedPageReferencedRoles.length && (
                  <span>如果本页出现固定主角、老师或关键道具，请在插图描述中写出名称，系统才会带入对应参考图。</span>
                )}
              </div>
              {selectedPageMissingReferenceRoles[0] && (
                <button className="button secondary" type="button" onClick={() => focusRoleReference(selectedPageMissingReferenceRoles[0])}>
                  管理角色参考图
                </button>
              )}
            </div>
            {shouldShowImageGenerationAction && !pageEditorOpen && (
              <div className="inline-actions">
                <button
                  className="button primary"
                  type="button"
                  disabled={imageActionBusy || promptRewriting || Boolean(pageImageReferenceBlocker)}
                  title={pageImageReferenceBlocker || undefined}
                  onClick={generateIllustration}
                >
                  {pageImageActionLabel(selectedPage.status, imageActionBusy)}
                </button>
                <button
                  className="button secondary"
                  type="button"
                  disabled={promptRewriting || imageActionBusy}
                  title="让 AI 基于本页正文重新创作插图描述"
                  onClick={rewritePagePrompt}
                >
                  {promptRewriting ? "AI 重写中..." : "AI 重写插图描述"}
                </button>
              </div>
            )}
          </Card>
          </aside>
        </div>
      </section>

      {shareOpen && (
        <Modal title="管理分享链接" onClose={() => setShareOpen(false)}>
          <section className="share-section">
            <p className="share-meta">分享范围：获得链接的人可查看当前绘本版本 · 当前空间：<strong>{workspace.name}</strong></p>
            <p className="share-meta">
              可见性 <strong>{visibilityLabel(book.visibility)}</strong>
              <span className="share-meta-sep">·</span>导出 <strong>{exportJobs.length ? exportStatusLabel(exportJobs[0].status) : "暂无记录"}</strong>
              <span className="share-meta-sep">·</span>分享链接 <strong>{shareLinks.length ? `${shareLinks.length} 个有效链接` : "未创建"}</strong>
              <span className="share-meta-sep">·</span>复核 <strong>{teacherReviewLabel(book.teacherReviewStatus)}</strong>
            </p>
            <div className="delivery-status-main modal-delivery-status">
              <div>
                <p className="eyebrow">分享前检查</p>
                <h3>{effectiveDeliveryBlocker ? "先处理阻断项，再分享" : book.teacherReviewStatus === "confirmed" ? "已复核，可以分享" : "建议老师复核后分享"}</h3>
                <p>{effectiveDeliveryBlocker || deliveryWarnings[0] || reviewDeliveryReminder || "页面、角色和插图检查已通过，可以创建分享链接。"}</p>
              </div>
              {quality && (
                <div className="delivery-status-actions">
                  <Badge tone={qualityTone(quality.status)}>{qualityStatusLabel(quality.status)}</Badge>
                  <button
                    className={book.teacherReviewStatus === "confirmed" ? "button secondary" : "button primary"}
                    type="button"
                    disabled={reviewSaving || (book.teacherReviewStatus !== "confirmed" && quality.status === "blocked")}
                    title={book.teacherReviewStatus !== "confirmed" && quality.status === "blocked" ? "请先修正生成质量阻断项" : undefined}
                    onClick={() => saveTeacherReview(book.teacherReviewStatus === "confirmed" ? "pending" : "confirmed")}
                  >
                    {reviewSaving ? "保存中..." : book.teacherReviewStatus === "confirmed" ? "重新设为待复核" : quality.status === "blocked" ? "先修正阻断项" : "老师已复核"}
                  </button>
                </div>
              )}
            </div>
            {quality && (firstActionableQualityPage || firstRoleNeedingReference) && (
              <div className="delivery-next-step">
                <div>
                  <strong>{quality.status === "blocked" ? "需要处理" : "建议查看"}</strong>
                  <span>
                    {firstActionableQualityPage
                      ? `第 ${firstActionableQualityPage.pageNumber} 页：${firstActionableQualityPage.issues[0] || firstActionableQualityPage.suggestions[0] || "请核对分页内容。"}`
                      : `${firstRoleNeedingReference?.name} 还没有可用参考图。`}
                  </span>
                </div>
                <div className="inline-actions">
                  {firstActionableQualityPage && (
                    <button className="button secondary" type="button" onClick={() => { setShareOpen(false); focusQualityPage(firstActionableQualityPage); }}>
                      定位问题页
                    </button>
                  )}
                  {firstRoleNeedingReference && (
                    <button className="button secondary" type="button" onClick={() => { setShareOpen(false); focusRoleReference(firstRoleNeedingReference); }}>
                      定位角色参考图
                    </button>
                  )}
                </div>
              </div>
            )}
            <p className="share-meta privacy-note">分享前请确认不包含未授权儿童信息或家庭隐私。</p>
            {quality && (
              <details className="quality-details compact">
                <summary>查看交付检查详情</summary>
                <div className="quality-check-grid">
                  {quality.checks.map((check) => (
                    <div className="quality-check-item" key={check.key}>
                      <Badge tone={qualityTone(check.status)}>{qualityStatusLabel(check.status)}</Badge>
                      <strong>{check.label}</strong>
                      <span>{check.message}</span>
                    </div>
                  ))}
                </div>
                <div className="quality-page-list">
                  {quality.pages.map((page) => (
                    <button className="quality-page-row" type="button" key={page.pageId} onClick={() => { setShareOpen(false); focusQualityPage(page); }}>
                      <div>
                        <strong>第 {page.pageNumber} 页</strong>
                        <span>{qualityPageSummary(page)}</span>
                        {(page.issues.length > 0 || page.suggestions.length > 0) && (
                          <div className="quality-page-notes">
                            {page.issues.map((issue) => (
                              <small className="quality-page-note issue" key={`issue-${issue}`}>问题：{issue}</small>
                            ))}
                            {page.suggestions.map((suggestion) => (
                              <small className="quality-page-note suggestion" key={`suggestion-${suggestion}`}>建议：{suggestion}</small>
                            ))}
                          </div>
                        )}
                      </div>
                      <Badge tone={qualityTone(page.status)}>{qualityStatusLabel(page.status)}</Badge>
                    </button>
                  ))}
                </div>
              </details>
            )}
          </section>

          <section className="share-section">
            <h3 className="share-section-title">分享设置</h3>
            <div className="form-grid">
              <label>
                整本绘本可见范围
                <select value={visibilityValue} onChange={(event) => setVisibilityValue(event.target.value as Storybook["visibility"])}>
                  <option value="private">仅当前空间私有</option>
                  <option value="workspace">园所/空间内共享</option>
                </select>
              </label>
              <button className="button secondary" type="button" disabled={visibilitySaving || visibilityValue === book.visibility} onClick={saveVisibility}>
                {visibilitySaving ? "保存中..." : visibilityValue === book.visibility ? "可见范围已保存" : "保存可见范围"}
              </button>
            </div>
          </section>

          <section className="share-section">
            <h3 className="share-section-title">分享链接</h3>
            {shareLinks.length ? (
              <div className="share-link-list">
                {shareLinks.map((link, index) => (
                  <div className="share-link-row" key={link.id}>
                    <div>
                      <strong>分享链接 {index + 1}</strong>
                      <span>{shareExpiryLabel(link.expiresAt)}</span>
                      <span>{shareAccessLabel(link)}</span>
                    </div>
                    <div className="inline-actions">
                      <a className="button secondary" href={link.url} target="_blank" rel="noreferrer">打开</a>
                      <button className="button secondary" type="button" onClick={() => copyShareUrl(link)}>复制链接</button>
                      <button className="button secondary" type="button" disabled={shareSaving} onClick={() => revokeShare(link)}>
                        {revokingShareId === link.id ? "撤回中..." : "撤回"}
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <p className="share-meta">还没有有效分享链接。</p>
            )}
            <div className="form-grid">
              <label>
                链接有效期
                <select value={shareExpiry} onChange={(event) => setShareExpiry(event.target.value as "7d" | "30d" | "never")}>
                  <option value="7d">7 天有效</option>
                  <option value="30d">30 天有效</option>
                  <option value="never">不过期</option>
                </select>
              </label>
            </div>
            <div className="modal-actions share-actions">
              <button className="button secondary" type="button" onClick={() => setShareOpen(false)}>关闭</button>
              {createdShareUrl && <a className="button secondary" href={createdShareUrl} target="_blank" rel="noreferrer">打开最新分享页</a>}
              <ActionButton className="button primary" disabled={shareSaving || Boolean(qualityDeliveryBlocker)} disabledHint={qualityDeliveryBlocker || (shareSaving ? "处理中，请稍候" : undefined)} onClick={createShare}>
                {shareSaving ? "处理中..." : "创建新的分享链接"}
              </ActionButton>
            </div>
          </section>
        </Modal>
      )}
      {zoomedImage && (
        <ImageLightbox src={zoomedImage.src} alt={zoomedImage.alt} onClose={() => setZoomedImage(null)} />
      )}
    </div>
  );
}

function qualityStatusLabel(status: string) {
  return {
    passed: "检查通过",
    needs_review: "需要复核",
    blocked: "存在阻断",
  }[status] || status;
}

function qualityTone(status: string): "neutral" | "good" | "warn" | "danger" | "info" {
  if (status === "passed") return "good";
  if (status === "blocked") return "danger";
  if (status === "needs_review") return "warn";
  return "neutral";
}

function qualityPageSummary(page: StorybookQualityReport["pages"][number]) {
  if (page.issues.length && page.suggestions.length) return `${page.issues.length} 个问题，${page.suggestions.length} 条建议。`;
  if (page.issues.length) return `${page.issues.length} 个问题需要先处理。`;
  if (page.suggestions.length) return `${page.suggestions.length} 条建议，老师确认后可继续。`;
  return "这一页暂未发现明显问题。";
}

function teacherReviewLabel(status?: string) {
  return status === "confirmed" ? "老师已复核" : "待老师复核";
}

function buildLocalStorybookQuality(book: Storybook): StorybookQualityReport {
  const consistencyRoles = book.roles.filter((role) => roleNeedsReference(book, role));
  const checks: StorybookQualityReport["checks"] = [
    {
      key: "structure",
      label: "内容结构",
      status: book.pages.length && book.roles.length ? "passed" : "blocked",
      message: book.pages.length && book.roles.length ? "已包含分页内容和角色/道具设定。" : "分页或角色设定不完整。",
    },
  ];
  const missingReferences = consistencyRoles.filter((role) => !role.referenceImageUrl);
  const staleReferences = consistencyRoles.filter((role) => role.referenceImageUrl && role.referenceStatus !== "ready");
  checks.push({
    key: "role_references",
    label: "角色参考图",
    status: missingReferences.length || staleReferences.length ? "needs_review" : "passed",
    message: !consistencyRoles.length
      ? "没有跨页重复出现的角色或道具需要参考图；只出现一次的事物无需参考图。"
      : missingReferences.length
      ? staleReferences.length
        ? `缺少参考图：${missingReferences.map((role) => role.name).join("、")}；建议更新参考图：${staleReferences.map((role) => role.name).join("、")}。`
        : `以下跨页出现的角色/道具还需要先生成参考图：${missingReferences.map((role) => role.name).join("、")}。`
      : staleReferences.length
        ? `以下角色/道具已有参考图但建议更新：${staleReferences.map((role) => role.name).join("、")}；当前已有图仍可用于生成。`
        : "跨页重复出现的角色/道具都已有参考图；只出现一次的事物无需参考图。",
  });

  let blockedPages = 0;
  let reviewPages = 0;
  const pages = book.pages.map((page) => {
    const issues: string[] = [];
    const suggestions: string[] = [];
    const pageText = `${page.title} ${page.body}`;
    const combinedText = `${pageText} ${page.illustrationPrompt}`;
    const pageRoles = book.roles.filter((role) => combinedText.includes(role.name));
    const promptHasConfirmedRole = pageRoles.some((role) => page.illustrationPrompt.includes(role.name));
    if (pageRoles.length && !promptHasConfirmedRole) issues.push("插图描述没有明确带入已确认角色/道具名称。");
    if (page.status === "generating") issues.push("插图仍在生成中。");
    if (page.status === "failed") issues.push("插图生成失败，需要重新生成。");
    if (page.status === "needs_regeneration") suggestions.push("当前页标记为需重绘，建议重新生成插图。");
    pageRoles.forEach((role) => {
      if (pageText.includes(role.name) && !page.illustrationPrompt.includes(role.name)) {
        issues.push(`正文出现了「${role.name}」，但插图描述没有同步这个名称。`);
      }
    });
    const status: StorybookQualityReport["status"] = issues.length ? "blocked" : suggestions.length ? "needs_review" : "passed";
    if (status === "blocked") blockedPages += 1;
    if (status === "needs_review") reviewPages += 1;
    return {
      pageId: page.id,
      pageNumber: page.pageNumber,
      status,
      issues,
      suggestions,
    };
  });

  checks.push({
    key: "page_prompts",
    label: "分页一致性",
    status: blockedPages ? "blocked" : reviewPages ? "needs_review" : pages.length ? "passed" : "blocked",
    message: blockedPages
      ? `${blockedPages} 个分页存在阻断问题，需要先修正提示词或重新生成。`
      : reviewPages
        ? `${reviewPages} 个分页需要老师复核或补充描述。`
        : pages.length
          ? "分页描述已带入角色/道具名称，没有发现明显一致性问题。"
          : "还没有可检查的分页。",
  });

  const status: StorybookQualityReport["status"] = checks.some((check) => check.status === "blocked")
    ? "blocked"
    : checks.some((check) => check.status === "needs_review")
      ? "needs_review"
      : "passed";
  return {
    status,
    summary: status === "passed"
      ? "系统检查通过，建议老师做最终阅读确认。"
      : status === "blocked"
        ? "系统发现阻断问题，请先修正角色、提示词或重新生成。"
        : "系统发现需要复核的项目，建议老师确认后再导出或分享。",
    checks,
    pages,
  };
}

function customizationBlockerFor(book: Storybook, quality?: StorybookQualityReport) {
  if (book.type !== "plain") return "只有普通绘本可以继续生成儿童定制版";
  if (!book.pages.length) return "请先生成绘本分页";
  if (!book.roles.length) return "请先确认角色与道具";
  const generatingPages = book.pages.filter((page) => page.status === "generating");
  if (generatingPages.length) return "仍有分页插图正在生成，请完成后再生成定制版";
  const failedPages = book.pages.filter((page) => page.status === "failed");
  if (failedPages.length) return "仍有分页插图生成失败，请修复后再生成定制版";
  const redrawPages = book.pages.filter((page) => page.status === "needs_regeneration");
  if (redrawPages.length) return `仍有 ${redrawPages.length} 页需要重绘，请先完成普通绘本`;
  const missingReferences = book.roles.filter((role) => roleNeedsReference(book, role) && (role.referenceStatus !== "ready" || !role.referenceImageUrl));
  if (missingReferences.length) return `跨页角色参考图未完成：${missingReferences.map((role) => role.name).join("、")}`;
  if (quality?.status === "blocked") return "质量检查存在阻断项，请先修正";
  if (book.status !== "exportable" && book.status !== "listed") return "请先将普通绘本标记为可交付";
  return "";
}

function roleNeedsReference(book: Storybook, role: StorybookRole) {
  return role.needsConsistency && rolePageUsageCount(book, role) >= 2;
}

function activeRoleReferenceJob(jobs: GenerationJob[], roleId: string) {
  return jobs.find((job) => {
    if (job.jobType !== "storybook_role_reference_image") return false;
    if (job.status !== "queued" && job.status !== "running") return false;
    const input = job.input;
    if (!input || typeof input !== "object" || !("role_id" in input)) return false;
    return (input as { role_id?: unknown }).role_id === roleId;
  });
}

function rolePageUsageCount(book: Storybook, role: StorybookRole) {
  return book.pages.filter((page) => {
    const text = `${page.title} ${page.body} ${page.illustrationPrompt}`;
    return text.includes(role.name);
  }).length;
}

function roleLabelMap(roleType: string) {
  return {
    protagonist: "主角",
    supporting: "配角",
    peer: "同伴角色",
    teacher: "老师形象",
    prop: "关键道具",
  }[roleType] || roleType;
}

function visibilityLabel(value: string) {
  return {
    private: "仅当前空间私有",
    workspace: "园所/空间内共享",
    market_submission: "市场投稿中",
    market_listed: "市场已上架",
  }[value] || value;
}

function exportStatusLabel(status: string) {
  return {
    queued: "排队中",
    running: "导出中",
    succeeded: "已完成",
    failed: "导出失败",
  }[status] || status;
}

function exportFailureText(job: ExportJob) {
  return job.lastError ? `失败原因：${job.lastError}` : "导出任务没有成功完成，请稍后重新导出。";
}

function shareExpiryToIso(value: "7d" | "30d" | "never") {
  if (value === "never") return undefined;
  const days = value === "30d" ? 30 : 7;
  const expiresAt = new Date();
  expiresAt.setDate(expiresAt.getDate() + days);
  return expiresAt.toISOString();
}

function shareExpiryLabel(expiresAt?: string) {
  if (!expiresAt) return "长期有效";
  return `有效期至 ${new Date(expiresAt).toLocaleDateString("zh-CN")}`;
}

function shareAccessLabel(link: ShareLink) {
  if (!link.accessCount) return "尚未访问";
  const lastAccess = link.lastAccessedAt ? `，最后访问 ${link.lastAccessedAt}` : "";
  return `已访问 ${link.accessCount} 次${lastAccess}`;
}

function pageImageActionLabel(pageStatus: string, generating = false) {
  if (generating) return "生成中...";
  if (pageStatus === "needs_regeneration" || pageStatus === "failed") return "按当前描述重绘插图";
  if (pageStatus === "ready") return "不满意，重新生成插图";
  return "按当前描述生成插图";
}

function roleReferenceStatusLabel(status?: string) {
  return {
    not_started: "未生成参考图",
    generating: "参考图生成中",
    ready: "参考图已确认",
    needs_regeneration: "需要重绘",
    failed: "生成失败",
  }[status || "not_started"] || "参考图待确认";
}

function roleReferenceStyleClause(coverTone: string) {
  const trimmed = coverTone.trim().replace(/。+$/, "");
  if (!trimmed || trimmed === "温暖、清楚") {
    return "柔和水彩绘本风格，圆润饱满造型，大而富有表现力的眼睛";
  }
  if (trimmed.includes("皮克斯") || trimmed.includes("3D")) {
    return `${trimmed}，高质量3D动画电影质感，立体圆润角色，柔和棚拍光，细腻材质，真实体积感`;
  }
  return trimmed;
}

function buildRoleReferencePrompt(role: Pick<StorybookRole, "name" | "roleType" | "appearance">, coverTone: string) {
  const name = role.name.trim() || "未命名角色";
  const appearance = cleanVisualAppearance(role.appearance) || "请先补充外观设定";
  const style = roleReferenceStyleClause(coverTone);
  const anatomy = roleReferenceAnatomyClause(role, appearance);
  return `${name}，${roleTypeLabel(role.roleType)}，${appearance}，画面风格必须与整本绘本一致：${style}，${anatomy}，表情自然生动、富有神采，姿态自然放松，单一角色标准图，白底或简洁背景，清晰展示完整轮廓或半身，可微微侧身，画面只有这个角色，无人类，无其他角色，保持跨页一致，不要僵硬对称的证件照式站姿`;
}

function isLimbFreeRole(role: Pick<StorybookRole, "name" | "roleType">, appearance: string) {
  const text = `${role.name} ${role.roleType} ${appearance}`;
  return ["无手", "没有手", "无脚", "没有脚", "无手和脚", "没有手和脚", "无四肢", "没有四肢", "蛇", "小蛇", "蚯蚓", "毛毛虫", "蜗牛", "球形"].some((keyword) =>
    text.includes(keyword),
  );
}

function roleReferenceAnatomyClause(role: Pick<StorybookRole, "name" | "roleType">, appearance: string) {
  if (isLimbFreeRole(role, appearance)) {
    return "身体结构必须严格符合外观设定：没有手、没有脚、没有手臂和腿，不要生成手指、鞋子、胳膊或人形四肢，用头部、眼睛、身体弯曲、尾部和整体姿态表达动作";
  }
  return "身体结构必须严格符合外观设定；有手、脚、爪或翅膀时可以清晰表现，但不要凭空添加外观没有写到的肢体";
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

function roleTypeLabel(roleType: StorybookRole["roleType"]) {
  return {
    protagonist: "主角",
    supporting: "配角",
    peer: "同伴角色",
    teacher: "老师形象",
    prop: "关键道具",
  }[roleType] || "角色";
}

function extractPageId(output: unknown) {
  const value = output as { image?: { page_id?: string; target_id?: string; target_type?: string } } | undefined;
  if (value?.image?.page_id) return value.image.page_id;
  return value?.image?.target_type === "page" ? value.image.target_id : undefined;
}

function extractPageIdFromInput(input: unknown) {
  const value = input as { page_id?: string; target_id?: string; target_type?: string } | undefined;
  if (value?.page_id) return value.page_id;
  return value?.target_type === "page" ? value.target_id : undefined;
}

function extractImageResult(output: unknown): { imageUrl: string; altText?: string; prompt?: string; styleNotes: string[]; provider?: string; message?: string } | null {
  const value = output as {
    provider?: string;
    message?: string;
    image?: {
      image_url?: string;
      alt_text?: string;
      prompt?: string;
      style_notes?: string[];
    };
  } | undefined;
  const image = value?.image;
  if (!image?.image_url) return null;
  return {
    imageUrl: image.image_url,
    altText: image.alt_text,
    prompt: image.prompt,
    styleNotes: image.style_notes || [],
    provider: value?.provider,
    message: value?.message,
  };
}

function latestPageImageJob(jobs: GenerationJob[], pageId?: string) {
  if (!pageId) return undefined;
  return jobs
    .filter((job) => job.jobType === "storybook_page_image" && job.output && extractPageId(job.output) === pageId)
    .sort((a, b) => generationJobTimestamp(b) - generationJobTimestamp(a))[0];
}

function activePageImageJob(jobs: GenerationJob[], pageId?: string) {
  if (!pageId) return undefined;
  return jobs
    .filter((job) => (
      job.jobType === "storybook_page_image"
      && (job.status === "queued" || job.status === "running")
      && extractPageIdFromInput(job.input) === pageId
    ))
    .sort((a, b) => generationJobTimestamp(b) - generationJobTimestamp(a))[0];
}

function generationJobTimestamp(job: GenerationJob) {
  return new Date(job.finishedAt || job.createdAt).getTime();
}

function generationJobIdFromImageUrl(url: string) {
  return url.match(/\/generation-jobs\/([^/]+)\/image/)?.[1];
}

function generationJobTitle(job: GenerationJob) {
  return generationJobTypeLabel[job.jobType] || job.jobType;
}

function generationJobCopy(job: GenerationJob) {
  if (job.status === "failed") return generationErrorMessage(job);
  if (job.status === "queued") return "任务已进入队列。";
  if (job.status === "running") return "任务正在生成中。";
  if (job.status === "canceled") return "任务已取消，不会继续执行。";
  if (job.storybookId) return "已写入本书内容。";
  return "已生成结构化结果。";
}

function generationJobTime(job: GenerationJob) {
  return job.finishedAt || job.createdAt;
}

function resultNoticeFromSearch(search: string): { title: string; copy: string; tone: "good"; action?: ReactNode } | null {
  const result = new URLSearchParams(search).get("result");
  if (result === "plain") {
    return {
      title: "生成结果已展示",
      copy: "普通绘本已经生成完成。请先检查故事、角色和分页插图，再导出 PDF 或派生定制版本。",
      tone: "good",
    };
  }
  if (result === "custom") {
    return {
      title: "定制结果已展示",
      copy: "这本定制绘本已经生成完成。请检查儿童信息、故事改写和插图一致性，再导出或分享给家长。",
      tone: "good",
    };
  }
  if (result === "batch-custom") {
    return {
      title: "批量定制结果已展示",
      copy: "已打开第一本定制绘本。请从这里开始逐本检查儿童信息、故事改写和插图一致性。",
      tone: "good",
    };
  }
  return null;
}

function canCancelGenerationJob(job: GenerationJob) {
  return job.status === "queued" || job.status === "failed";
}
