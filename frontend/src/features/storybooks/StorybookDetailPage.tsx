import { ArrowRight, CheckCircle2, Copy, Download, MoreHorizontal, Pencil, Send } from "lucide-react";
import { ChangeEvent, FormEvent, type ReactNode, useEffect, useRef, useState } from "react";
import { Link, useLocation, useNavigate, useOutletContext, useParams } from "react-router-dom";
import {
  cancelGenerationJob,
  createCoverImageTask,
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
  listStorybookImageVariants,
  listShareLinksPage,
  listStorybookGenerationJobs,
  listStorybookExportsPage,
  revokeShareLink,
  selectStorybookImageVariant,
  retryGenerationJob,
  updateStorybook,
  updateStorybookPage,
  updateStorybookRole,
  type ExportJob,
  type GenerationJob,
  type ShareLink,
} from "../../api/client";
import { ActionButton, Badge, Card, ImageLightbox, Modal, Notice, PageHeader, SkeletonBlock, Toast, statusTone } from "../../components/ui";
import type { Storybook, StorybookImageVariant, StorybookQualityReport, StorybookRole, Workspace } from "../../types/domain";
import { absoluteAppUrl, copyText } from "../../utils/clipboard";
import { cacheImagePreview, getCachedImagePreview } from "../../utils/imagePreviewCache";
import { pageAspectCssRatio, pageAspectLabel } from "../../utils/pageAspect";
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
import { ImageVariantStrip } from "./detail/components/ImageVariantStrip";
import { ShareLinksModal } from "./detail/components/ShareLinksModal";
import {
  DeleteStorybookModal,
  DuplicateStorybookModal,
  EditStorybookMetaModal,
} from "./detail/components/StorybookActionModals";
import {
  activePageImageJob,
  activeRoleReferenceJob,
  buildLocalStorybookQuality,
  buildRoleReferencePrompt,
  canCancelGenerationJob,
  cleanVisualAppearance,
  compactPromptSummary,
  customizationBlockerFor,
  exportFailureText,
  exportStatusLabel,
  extractImageResult,
  generationJobCopy,
  generationJobIdFromImageUrl,
  generationJobTime,
  generationJobTitle,
  illustrationShotLabel,
  latestPageImageJob,
  pageImageActionLabel,
  qualityStatusLabel,
  qualityTone,
  resultNoticeFromSearch,
  roleLabelMap,
  roleNeedsReference,
  rolePageUsageCount,
  roleReferenceStatusLabel,
  shareExpiryLabel,
  shareExpiryToIso,
  teacherReviewLabel,
  visibilityLabel,
} from "./detail/helpers";

const COVER_PAGE_ID = "__cover__";
type BulkImageStep = {
  id: string;
  label: string;
  kind: "reference" | "cover" | "page";
  status: "pending" | "running" | "done" | "failed" | "skipped";
  jobId?: string;
  error?: string;
};

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
  const workspaceMainRef = useRef<HTMLDivElement | null>(null);
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
    pageAspectRatio: "portrait_4_5" as Storybook["pageAspectRatio"],
  });
  const [imageGenerating, setImageGenerating] = useState(false);
  const [bulkImageGenerating, setBulkImageGenerating] = useState(false);
  const [bulkImageSteps, setBulkImageSteps] = useState<BulkImageStep[]>([]);
  // 记录正在重写插图描述的页面 ID，避免切换绘本/分页后按钮状态残留。
  const [promptRewritingPageId, setPromptRewritingPageId] = useState<string | null>(null);
  const [currentImagePreviewUrl, setCurrentImagePreviewUrl] = useState("");
  const [currentImagePreviewError, setCurrentImagePreviewError] = useState("");
  const [coverImagePreviewUrl, setCoverImagePreviewUrl] = useState("");
  const [coverImagePreviewError, setCoverImagePreviewError] = useState("");
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
  const [pageImageVariants, setPageImageVariants] = useState<StorybookImageVariant[]>([]);
  const [coverImageVariants, setCoverImageVariants] = useState<StorybookImageVariant[]>([]);
  const [roleImageVariants, setRoleImageVariants] = useState<StorybookImageVariant[]>([]);
  const [selectingVariantId, setSelectingVariantId] = useState<string | null>(null);
  const selectedViewIsCover = selectedPageId === COVER_PAGE_ID;
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
    ...(book.pages.some((page) => page.status === "draft") ? ["仍有分页插图未生成"] : []),
    ...(book.pages.some((page) => page.status === "failed") ? ["存在插图生成失败的分页"] : []),
    ...(book.pages.some((page) => page.status === "needs_regeneration") ? ["存在待重新生成插图的分页"] : []),
  ] : [];
  const deliveryWarnings: string[] = [];
  const canDeliver =
    Boolean(book && book.id === storybookId && (book.status === "exportable" || book.status === "listed"));
  const canMarkDeliverable =
    Boolean(book && book.id === storybookId && (book.status === "editing" || book.status === "image_pending") && deliveryBlockers.length === 0);
  const quality = book ? book.quality || buildLocalStorybookQuality(book) : undefined;
  const reviewDeliveryReminder = book && book.teacherReviewStatus !== "confirmed"
    ? "这本绘本还没有人工复核记录，建议先确认已复核后再导出或分享。"
    : "";
  const qualityDeliveryBlocker = quality?.status === "blocked"
    ? "生成质量检查存在阻断项，请先修正后再导出或创建分享链接。"
    : "";
  const effectiveDeliveryBlocker = qualityDeliveryBlocker || deliveryBlockers[0] || "";
  const canStartDelivery = canDeliver && !qualityDeliveryBlocker;
  const customizationBlocker = book ? customizationBlockerFor(book, quality) : "请等待当前绘本加载完成";
  const canCreateCustomVersion = book?.type === "plain" && !customizationBlocker;
  const selectedPageQuality = selectedPage && !selectedViewIsCover && quality
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
  const activePageImageJobs = book?.pages.filter((page) => activePageImageJob(generationJobs, page.id)).length || 0;
  const shouldShowBulkImageAction = Boolean(book?.pages.length && (book.status === "editing" || book.status === "image_pending"));

  useEffect(() => {
    workspaceMainRef.current?.scrollTo({ top: 0, behavior: "smooth" });
  }, [selectedPageId]);

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
        setSelectedPageId(COVER_PAGE_ID);
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
    if (selectedViewIsCover || !book?.id || !selectedPage?.id) {
      setPageImageVariants([]);
      return;
    }
    let active = true;
    listStorybookImageVariants(workspace.id, book.id, {
      targetType: "page_illustration",
      targetId: selectedPage.id,
    })
      .then((variants) => {
        if (active) setPageImageVariants(variants);
      })
      .catch(() => {
        if (active) setPageImageVariants([]);
      });
    return () => {
      active = false;
    };
  }, [book?.id, selectedPage?.id, selectedViewIsCover, workspace.id]);

  useEffect(() => {
    if (!book?.id) {
      setCoverImageVariants([]);
      return;
    }
    let active = true;
    listStorybookImageVariants(workspace.id, book.id, {
      targetType: "cover_illustration",
      targetId: book.id,
    })
      .then((variants) => { if (active) setCoverImageVariants(variants); })
      .catch(() => { if (active) setCoverImageVariants([]); });
    return () => {
      active = false;
    };
  }, [book?.id, workspace.id]);

  useEffect(() => {
    if (!book?.id || !selectedRole?.id) {
      setRoleImageVariants([]);
      return;
    }
    let active = true;
    listStorybookImageVariants(workspace.id, book.id, {
      targetType: "role_reference",
      targetId: selectedRole.id,
    })
      .then((variants) => {
        if (active) setRoleImageVariants(variants);
      })
      .catch(() => {
        if (active) setRoleImageVariants([]);
      });
    return () => {
      active = false;
    };
  }, [book?.id, selectedRole?.id, workspace.id]);

  useEffect(() => {
    if (selectedViewIsCover || !selectedPage) return;
    setPageForm({
      title: selectedPage.title,
      body: selectedPage.body,
      illustrationPrompt: selectedPage.illustrationPrompt,
    });
    setPageEditorOpen(false);
  }, [selectedPage?.id, selectedViewIsCover]);

  useEffect(() => {
    if (!book) return;
    setVisibilityValue(book.visibility);
    setMetaForm({
      title: book.title,
      ageGroup: book.ageGroup,
      useScene: book.useScene,
      teachingGoal: book.teachingGoal,
      coverTone: book.coverTone,
      pageAspectRatio: book.pageAspectRatio,
    });
  }, [book?.id, book?.title, book?.visibility, book?.ageGroup, book?.useScene, book?.teachingGoal, book?.coverTone, book?.pageAspectRatio]);

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

  async function refreshPageImageVariants(storybookId = book?.id, pageId = selectedPage?.id) {
    if (!storybookId || !pageId) {
      setPageImageVariants([]);
      return;
    }
    setPageImageVariants(await listStorybookImageVariants(workspace.id, storybookId, {
      targetType: "page_illustration",
      targetId: pageId,
    }));
  }

  async function refreshCoverImageVariants(storybookId = book?.id) {
    if (!storybookId) {
      setCoverImageVariants([]);
      return;
    }
    setCoverImageVariants(await listStorybookImageVariants(workspace.id, storybookId, {
      targetType: "cover_illustration",
      targetId: storybookId,
    }));
  }

  async function refreshRoleImageVariants(storybookId = book?.id, roleId = selectedRole?.id) {
    if (!storybookId || !roleId) {
      setRoleImageVariants([]);
      return;
    }
    setRoleImageVariants(await listStorybookImageVariants(workspace.id, storybookId, {
      targetType: "role_reference",
      targetId: roleId,
    }));
  }

  async function refreshStorybook(storybookId = book?.id) {
    if (!storybookId) return undefined;
    const updated = await getStorybook(workspace.id, storybookId);
    setRemoteBook(updated);
    setSelectedPageId((current) => current === COVER_PAGE_ID || (current && updated.pages.some((page) => page.id === current)) ? current : updated.pages[0]?.id || COVER_PAGE_ID);
    setSelectedRoleId((current) => current && updated.roles.some((role) => role.id === current) ? current : updated.roles[0]?.id);
    setVisibilityValue(updated.visibility);
    return updated;
  }

  const currentStoryPageId = selectedViewIsCover ? undefined : selectedPage?.id;
  const latestCoverImageJob = generationJobs.find((job) => job.jobType === "storybook_cover_image" && job.storybookId === book?.id && job.status === "succeeded");
  const activeCoverImageJob = generationJobs.find((job) => job.jobType === "storybook_cover_image" && job.storybookId === book?.id && isActiveJobStatus(job.status));
  const selectedCoverImageVariant = coverImageVariants.find((variant) => variant.isSelected);
  const selectedCoverImageUrl = selectedCoverImageVariant?.imageUrl;
  const selectedCoverImageJobId = (selectedCoverImageUrl ? generationJobIdFromImageUrl(selectedCoverImageUrl) : undefined) || selectedCoverImageVariant?.generationJobId;
  const fallbackCoverImage = extractImageResult(latestCoverImageJob?.output);
  const currentCoverImage = selectedCoverImageUrl
    ? {
      imageUrl: selectedCoverImageUrl,
      altText: `${book?.title || "绘本"}封面图`,
      prompt: selectedCoverImageVariant?.prompt || fallbackCoverImage?.prompt,
      styleNotes: selectedCoverImageVariant?.provider ? [selectedCoverImageVariant.provider] : fallbackCoverImage?.styleNotes || [],
    }
    : fallbackCoverImage;
  const currentCoverImageJobId = selectedCoverImageJobId || latestCoverImageJob?.id;
  const currentPageImageJob = latestPageImageJob(generationJobs, currentStoryPageId);
  const activeCurrentPageImageJob = activePageImageJob(generationJobs, currentStoryPageId);
  const selectedPageImageVariant = pageImageVariants.find((variant) => variant.isSelected)
    || pageImageVariants.find((variant) => variant.id === selectedPage?.selectedImageVariantId);
  const selectedPageImageUrl = selectedPage?.imageUrl || selectedPageImageVariant?.imageUrl;
  const selectedPageImageJobId = (selectedPageImageUrl ? generationJobIdFromImageUrl(selectedPageImageUrl) : undefined) || selectedPageImageVariant?.generationJobId;
  const fallbackPageImage = extractImageResult(currentPageImageJob?.output);
  const currentPageImage = selectedPageImageUrl
    ? {
      imageUrl: selectedPageImageUrl,
      altText: selectedPage?.title,
      prompt: selectedPageImageVariant?.prompt || fallbackPageImage?.prompt || selectedPage?.illustrationPrompt,
      styleNotes: selectedPageImageVariant?.provider ? [selectedPageImageVariant.provider] : fallbackPageImage?.styleNotes || [],
    }
    : fallbackPageImage;
  const currentPageImageJobId = selectedPageImageJobId || currentPageImageJob?.id;
  const imageActionBusy = imageGenerating || bulkImageGenerating || Boolean(activeCurrentPageImageJob);
  const shouldShowImageGenerationAction = Boolean(!selectedViewIsCover && selectedPage);
  const promptRewriting = promptRewritingPageId !== null && promptRewritingPageId === selectedPage?.id;
  const missingCoverImage = !currentCoverImage && !activeCoverImageJob;
  const missingPageImages = book?.pages.filter((page) => page.status !== "ready" && !activePageImageJob(generationJobs, page.id)) || [];
  const pendingRoleReferenceCount = book?.roles.filter(
    (role) => roleNeedsReference(book, role) && (role.referenceStatus !== "ready" || !role.referenceImageUrl),
  ).length ?? 0;
  const bulkImageTotal = pendingRoleReferenceCount + (missingCoverImage ? 1 : 0) + missingPageImages.length;
  const activeAnyImageJob = Boolean(activeCoverImageJob || activePageImageJobs > 0);
  const readyPageCount = book?.pages.filter((page) => page.status === "ready").length || 0;
  const issuePageCount = book?.pages.filter((page) => page.status === "failed" || page.status === "needs_regeneration").length || 0;
  const reviewMaterialLabels = book ? [
    book.useScene,
    book.ageGroup,
    book.teachingGoal,
    ...book.roles.slice(0, 3).map((role) => role.name),
  ].filter(Boolean) : [];
  const reviewPanelStatus = canDeliver
    ? "作品完成"
    : shouldShowBulkImageAction && bulkImageTotal > 0
      ? "待补插图"
      : issuePageCount > 0
        ? "需处理"
        : "验收中";
  const reviewPanelTone = canDeliver
    ? "good"
    : issuePageCount > 0
      ? "warn"
      : shouldShowBulkImageAction && bulkImageTotal > 0
        ? "info"
        : "neutral";

  useEffect(() => {
    if (!currentPageImage) {
      setCurrentImagePreviewUrl("");
      setCurrentImagePreviewError("");
      return;
    }
    if (!currentPageImageJobId) {
      setCurrentImagePreviewUrl(currentPageImage.imageUrl);
      setCurrentImagePreviewError("");
      return;
    }
    const jobId = currentPageImageJobId;
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
  }, [currentPageImage?.imageUrl, currentPageImageJobId, workspace.id]);

  useEffect(() => {
    if (!currentCoverImage) {
      setCoverImagePreviewUrl("");
      setCoverImagePreviewError("");
      return;
    }
    if (!currentCoverImageJobId) {
      setCoverImagePreviewUrl(currentCoverImage.imageUrl);
      setCoverImagePreviewError("");
      return;
    }
    const jobId = currentCoverImageJobId;
    const cached = getCachedImagePreview(jobId);
    if (cached) {
      setCoverImagePreviewUrl(cached);
      setCoverImagePreviewError("");
      return;
    }
    let active = true;
    setCoverImagePreviewUrl("");
    setCoverImagePreviewError("");
    downloadGenerationImageFile(workspace.id, jobId)
      .then((file) => {
        if (!active) return;
        const url = window.URL.createObjectURL(file);
        cacheImagePreview(jobId, url);
        setCoverImagePreviewUrl(url);
      })
      .catch((err) => {
        if (active) {
          setCoverImagePreviewUrl("");
          setCoverImagePreviewError(err instanceof Error ? err.message : "封面图文件读取失败");
        }
      });
    return () => {
      active = false;
    };
  }, [currentCoverImage?.imageUrl, currentCoverImageJobId, workspace.id]);

  useEffect(() => {
    if (!book?.id || !activeCoverImageJob) return;

    let active = true;
    pollGenerationJob(workspace.id, activeCoverImageJob, {
      timeoutMs: 300_000,
      onUpdate: (job) => {
        if (!active) return;
        setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
      },
    })
      .then(async (job) => {
        if (!active || isActiveJobStatus(job.status)) return;
        await refreshGenerationJobs(book.id);
        await refreshCoverImageVariants(book.id);
        await refreshStorybook(book.id);
        setSelectedPageId(COVER_PAGE_ID);
        if (job.status === "failed") {
          setRetryImageJob(job);
          setNotice({ title: "封面图生成失败", copy: `${generationErrorMessage(job)}。可以重新生成封面图。`, tone: "info" });
          return;
        }
        setRetryImageJob(null);
        setNotice({ title: "封面图已生成", copy: "封面页结果已刷新，可继续生成更多候选图。", tone: "good" });
      })
      .catch((err) => {
        if (active) {
          setNotice({ title: "封面图状态刷新失败", copy: err instanceof Error ? err.message : "请稍后手动刷新页面", tone: "info" });
        }
      });
    return () => {
      active = false;
    };
  }, [activeCoverImageJob?.id, book?.id, workspace.id]);

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
        await refreshPageImageVariants(book.id, currentPageId);
        setSelectedPageId(currentPageId);
        if (job.status === "failed") {
          setRetryImageJob(job);
          setNotice({ title: "插图生成失败", copy: `${generationErrorMessage(job)}。可以重新生成这一页。`, tone: "info" });
          return;
        }
        setRetryImageJob(null);
        setNotice({ title: "真实插图已生成", copy: `当前页结果已刷新。`, tone: "good" });
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
      setRoleReferencePreviewError("角色参考图暂时无法读取");
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

  async function persistCurrentPageForGeneration() {
    if (!selectedPage || !storybookId) return null;
    const updatedPage = await updateStorybookPage(workspace.id, storybookId, selectedPage.id, {
      title: pageForm.title,
      body: pageForm.body,
      illustrationPrompt: pageForm.illustrationPrompt,
    });
    const updatedBook = await getStorybook(workspace.id, storybookId);
    setRemoteBook(updatedBook);
    return { updatedPage, updatedBook };
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
      setNotice({ title: "角色设定已保存", copy: `${updated.name} 的外观设定已更新，后续画面会按新的设定生成。`, tone: "good" });
      setRetryImageJob(null);
    } catch (err) {
      setNotice({ title: "角色保存失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setRoleSaving(false);
    }
  }

  async function persistCurrentRoleForGeneration() {
    if (!book || !selectedRole) return null;
    const updatedRole = await updateStorybookRole(workspace.id, book.id, selectedRole.id, {
      name: roleForm.name,
      roleType: roleForm.roleType,
      appearance: cleanVisualAppearance(roleForm.appearance),
      storyFunction: roleForm.storyFunction,
      needsConsistency: roleForm.needsConsistency,
      referenceImagePrompt: buildRoleReferencePrompt(roleForm, book.coverTone),
    });
    const updatedBook = await getStorybook(workspace.id, book.id);
    setRemoteBook(updatedBook);
    return { updatedRole, updatedBook };
  }

  async function generateRoleReferenceImage() {
    if (!book || !selectedRole) return;
    if (!roleForm.needsConsistency || selectedRolePageCount < 2) {
      setNotice({ title: "无需生成参考图", copy: `${roleForm.name} 当前不需要跨页保持同一形象，不需要单独生成角色参考图。`, tone: "info" });
      return;
    }
    setRoleImageGenerating(true);
    try {
      const persisted = await persistCurrentRoleForGeneration();
      const persistedRole = persisted?.updatedRole || selectedRole;
      const job = await createRoleReferenceImageTask(workspace.id, book.id, persistedRole.id, {
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
          copy: `当前${generationStatusLabel(settledJob.status)}，完成前会持续显示为生成中。`,
          tone: "info",
        });
        return;
      }
      const updated = await getStorybook(workspace.id, book.id);
      setRemoteBook(updated);
      await refreshRoleImageVariants(book.id, persistedRole.id);
      const updatedRole = updated.roles.find((role) => role.id === persistedRole.id);
      if (settledJob.status === "failed") {
        setNotice({
          title: "角色参考图生成失败",
          copy: `${generationErrorMessage(settledJob)}。可以稍后重新生成参考图。`,
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
          copy: `当前${generationStatusLabel(settledJob.status)}，完成后可刷新查看。`,
          tone: "info",
        });
        return;
      }
      if (settledJob.status === "failed") {
        setNotice({
          title: "插图描述重写失败",
          copy: `${generationErrorMessage(settledJob)}。可以稍后重新调整这一页。`,
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
      const persisted = await persistCurrentPageForGeneration();
      const persistedPage = persisted?.updatedPage || selectedPage;
      const sourceBook = persisted?.updatedBook || book;
      const referenceRoles = sourceBook.roles.filter(
        (role) => role.needsConsistency && role.referenceStatus === "ready" && role.referenceImageUrl,
      );
      const job = await createPageImageTask(workspace.id, sourceBook.id, persistedPage.id, {
        prompt: pageForm.illustrationPrompt,
        referenceRoleIds: referenceRoles.map((role) => role.id),
        imageMode: referenceRoles.length ? "reference_image" : "text_to_image",
      });
      // 交给统一的轮询 effect 跟踪完成状态，避免这里再重复轮询。
      setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
      await refreshPageImageVariants(sourceBook.id, persistedPage.id);
      await refreshStorybook(sourceBook.id);
      await refreshGenerationJobs(sourceBook.id);
    } catch (err) {
      setNotice({ title: "插图生成失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setImageGenerating(false);
    }
  }

  async function generateCoverImage() {
    if (!book) return;
    setImageGenerating(true);
    setRetryImageJob(null);
    try {
      setNotice({
        title: "正在准备封面角色",
        copy: "先确认跨页角色的参考图，再生成封面，保证人物和正文保持一致。",
        tone: "info",
      });
      const sourceBook = await ensureCoverCharacterReferences(book);
      const job = await createCoverImageTask(workspace.id, sourceBook.id);
      setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
      await refreshCoverImageVariants(sourceBook.id);
      await refreshGenerationJobs(sourceBook.id);
      setNotice({
        title: "封面图生成已开始",
        copy: "封面页已开始生成，完成后这里会自动刷新。",
        tone: "info",
      });
    } catch (err) {
      setNotice({ title: "封面图生成失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setImageGenerating(false);
    }
  }

  async function generateAllImages() {
    if (!book) return;
    const coverNeeded = missingCoverImage;
    const pagesToGenerate = missingPageImages;
    const rolesNeedingReferences = book.roles.filter((role) => (
      roleNeedsReference(book, role) && (role.referenceStatus !== "ready" || !role.referenceImageUrl)
    ));
    const steps: BulkImageStep[] = [
      ...rolesNeedingReferences.map((role) => ({
        id: role.id,
        label: `${role.name} 角色参考图`,
        kind: "reference" as const,
        status: "pending" as const,
      })),
      ...(coverNeeded ? [{ id: COVER_PAGE_ID, label: "封面图", kind: "cover" as const, status: "pending" as const }] : []),
      ...pagesToGenerate.map((page) => ({
        id: page.id,
        label: `第 ${page.pageNumber} 页`,
        kind: "page" as const,
        status: "pending" as const,
      })),
    ];
    if (!steps.length) {
      setNotice({ title: "整本插图已完成", copy: "封面和所有分页都有可用插图，可以继续浏览验收并完成作品。", tone: "good" });
      return;
    }

    const updateStep = (id: string, patch: Partial<BulkImageStep>) => {
      setBulkImageSteps((current) => current.map((step) => step.id === id ? { ...step, ...patch } : step));
    };
    const waitForImageJob = (job: GenerationJob) => pollGenerationJob(workspace.id, job, {
      timeoutMs: 300_000,
      onUpdate: (current) => setGenerationJobs((jobs) => [current, ...jobs.filter((item) => item.id !== current.id)]),
    });

    setBulkImageGenerating(true);
    setRetryImageJob(null);
    setBulkImageSteps(steps);
    setNotice({
      title: "开始生成整本插图",
      copy: `将依次生成 ${steps.length} 张图片，完成后自动刷新绘本。`,
      tone: "info",
    });

    let latestBook = book;
    try {
      latestBook = await ensureCoverCharacterReferences(latestBook, (roleId, patch) => updateStep(roleId, patch));
      if (coverNeeded) {
        updateStep(COVER_PAGE_ID, { status: "running" });
        const job = await createCoverImageTask(workspace.id, latestBook.id);
        updateStep(COVER_PAGE_ID, { jobId: job.id });
        setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
        const settled = await waitForImageJob(job);
        if (settled.status === "failed") {
          updateStep(COVER_PAGE_ID, { status: "failed", error: generationErrorMessage(settled) });
          setRetryImageJob(settled);
          throw new Error(`封面图生成失败：${generationErrorMessage(settled)}`);
        }
        updateStep(COVER_PAGE_ID, { status: "done" });
        await refreshCoverImageVariants(latestBook.id);
        latestBook = await refreshStorybook(latestBook.id) || latestBook;
        await refreshGenerationJobs(latestBook.id);
      }

      for (const page of pagesToGenerate) {
        const currentPage = latestBook.pages.find((item) => item.id === page.id) || page;
        if (currentPage.status === "ready") {
          updateStep(page.id, { status: "skipped" });
          continue;
        }
        updateStep(page.id, { status: "running" });
        const referenceRoles = latestBook.roles.filter((role) => role.needsConsistency && role.referenceStatus === "ready" && role.referenceImageUrl);
        const job = await createPageImageTask(workspace.id, latestBook.id, currentPage.id, {
          prompt: currentPage.illustrationPrompt,
          referenceRoleIds: referenceRoles.map((role) => role.id),
          imageMode: referenceRoles.length ? "reference_image" : "text_to_image",
        });
        updateStep(page.id, { jobId: job.id });
        setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
        const settled = await waitForImageJob(job);
        if (settled.status === "failed") {
          updateStep(page.id, { status: "failed", error: generationErrorMessage(settled) });
          setRetryImageJob(settled);
          throw new Error(`第 ${currentPage.pageNumber} 页插图生成失败：${generationErrorMessage(settled)}`);
        }
        updateStep(page.id, { status: "done" });
        latestBook = await refreshStorybook(latestBook.id) || latestBook;
        await refreshGenerationJobs(latestBook.id);
      }

      await refreshCoverImageVariants(latestBook.id);
      if (selectedPage?.id) await refreshPageImageVariants(latestBook.id, selectedPage.id);
      setRetryImageJob(null);
      setNotice({
        title: "整本插图已生成",
        copy: "封面和分页插图已刷新，可以逐页检查并完成作品。",
        tone: "good",
      });
    } catch (err) {
      setNotice({
        title: "整本插图生成中断",
        copy: err instanceof Error ? err.message : "请稍后重试。",
        tone: "info",
      });
    } finally {
      setBulkImageGenerating(false);
    }
  }

  async function ensureCoverCharacterReferences(
    sourceBook: Storybook,
    onProgress?: (roleId: string, patch: Partial<BulkImageStep>) => void,
  ): Promise<Storybook> {
    let latestBook = sourceBook;
    const roles = sourceBook.roles.filter((role) => (
      roleNeedsReference(sourceBook, role) && (role.referenceStatus !== "ready" || !role.referenceImageUrl)
    ));
    for (const role of roles) {
      const latestRole = latestBook.roles.find((item) => item.id === role.id) || role;
      if (latestRole.referenceStatus === "ready" && latestRole.referenceImageUrl) {
        onProgress?.(role.id, { status: "skipped" });
        continue;
      }
      onProgress?.(role.id, { status: "running" });
      const activeJob = activeRoleReferenceJob(generationJobs, latestRole.id);
      const job = activeJob || await createRoleReferenceImageTask(workspace.id, latestBook.id, latestRole.id, {
        referenceImageUrls: [],
        imageMode: "text_to_image",
      });
      onProgress?.(role.id, { jobId: job.id });
      setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
      const settled = await pollGenerationJob(workspace.id, job, {
        timeoutMs: 300_000,
        onUpdate: (current) => setGenerationJobs((jobs) => [current, ...jobs.filter((item) => item.id !== current.id)]),
      });
      if (settled.status !== "succeeded") {
        const error = generationErrorMessage(settled);
        onProgress?.(role.id, { status: "failed", error });
        setRetryImageJob(settled);
        throw new Error(`${latestRole.name} 的角色参考图未完成：${error}`);
      }
      latestBook = await refreshStorybook(latestBook.id) || latestBook;
      const refreshedRole = latestBook.roles.find((item) => item.id === latestRole.id);
      if (!refreshedRole?.referenceImageUrl || refreshedRole.referenceStatus !== "ready") {
        const error = `${latestRole.name} 的角色参考图没有成功写回`;
        onProgress?.(role.id, { status: "failed", error });
        throw new Error(error);
      }
      onProgress?.(role.id, { status: "done" });
      await refreshGenerationJobs(latestBook.id);
    }
    return latestBook;
  }

  async function retryIllustration() {
    if (!book || !retryImageJob) return;
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
      setNotice({ title: "已取消生成", copy: "这次生成不会继续执行，可以按需重新发起生成。", tone: "good" });
    } catch (err) {
      setNotice({ title: "取消失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setCancelingJobId(null);
    }
  }

  async function selectImageVariant(variant: StorybookImageVariant) {
    if (!book) return;
    setSelectingVariantId(variant.id);
    try {
      await selectStorybookImageVariant(workspace.id, book.id, variant.id);
      await refreshStorybook(book.id);
      if (variant.targetType === "page_illustration") {
        await refreshPageImageVariants(book.id, variant.targetId);
      } else if (variant.targetType === "role_reference") {
        await refreshRoleImageVariants(book.id, variant.targetId);
      } else {
        await refreshCoverImageVariants(book.id);
      }
      setNotice({
        title: "已切换当前使用图",
        copy: variant.targetType === "page_illustration"
          ? "当前页插图已切换为所选候选图。"
          : variant.targetType === "role_reference"
            ? "角色参考图已切换为所选候选图。"
            : "封面图已切换为所选候选图。",
        tone: "good",
      });
    } catch (err) {
      setNotice({ title: "切换失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setSelectingVariantId(null);
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
      setNotice({ title: "还不能导出", copy: "请先完成编辑和整本验收，再创建 PDF 导出。", tone: "info" });
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
      setSelectedPageId(COVER_PAGE_ID);
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
      setNotice({ title: "暂不能完成作品", copy: deliveryBlockers.join("；"), tone: "info" });
      return;
    }
    setDeliverySaving(true);
    try {
      const updated = await updateStorybook(workspace.id, book.id, { status: "exportable" });
      setRemoteBook(updated);
      setNotice({ title: "作品已完成", copy: `《${updated.title}》现在可以导出 PDF、分享给家庭，也可作为定制绘本母本。`, tone: "good" });
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
        pageAspectRatio: metaForm.pageAspectRatio,
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
      setNotice({ title: "还不能分享", copy: "请先完成编辑和整本验收，再创建家庭分享链接。", tone: "info" });
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

  if (error || !book || (!selectedViewIsCover && !selectedPage)) {
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
  const retryImageJobActionLabel = retryImageJob?.jobType === "storybook_cover_image"
    ? "重新生成封面图"
    : retryImageJob?.jobType === "storybook_role_reference_image"
      ? "重新生成参考图"
      : "重新生成插图";

  return (
    <div className="page-stack">
      <PageHeader
        eyebrow={book.type === "plain" ? "专属故事验收" : "定制故事验收"}
        title={book.title}
        copy={`浏览整本作品，只编辑或重画不满意的页面。${storybookSourceLabel(book)}。归属：${workspace.name}`}
        actionClassName="storybook-detail-actions"
        className="storybook-detail-header"
        actions={
          <>
            {/* 主操作：按状态只保留一个 */}
            {shouldShowBulkImageAction && bulkImageTotal > 0 ? (
              <ActionButton className="button primary" disabled={bulkImageGenerating || imageGenerating || activeAnyImageJob} disabledHint={activeAnyImageJob ? "已有插图正在生成，请稍候" : undefined} onClick={generateAllImages}>
                {bulkImageGenerating ? "生成插图中..." : `一键生成插图${bulkImageTotal ? `（${bulkImageTotal} 张）` : ""}`}
              </ActionButton>
            ) : canDeliver ? (
              <ActionButton className="button primary" disabled={exporting || !canStartDelivery} disabledHint={qualityDeliveryBlocker || reviewDeliveryReminder || (exporting ? "导出进行中" : undefined)} onClick={exportPdf}><Download size={16} />{exporting ? "导出中..." : "导出 PDF"}</ActionButton>
            ) : (book.status === "editing" || book.status === "image_pending") ? (
              <ActionButton className="button primary" disabled={deliverySaving || !canMarkDeliverable} disabledHint={deliveryBlockers.join("；") || "请等待当前绘本加载完成"} onClick={markDeliverable}><CheckCircle2 size={16} />{deliverySaving ? "确认中..." : "全部看过，完成作品"}</ActionButton>
            ) : (
              <button className="button primary" type="button" onClick={scrollToWorkspace}>继续验收</button>
            )}
            {/* 次操作 */}
            {canDeliver ? (
              <ActionButton className="button secondary" disabled={!canStartDelivery} disabledHint={qualityDeliveryBlocker || reviewDeliveryReminder || undefined} onClick={() => setShareOpen(true)}><Send size={16} />分享链接</ActionButton>
            ) : (book.status === "editing" || book.status === "image_pending") ? (
              <button className="button secondary" type="button" onClick={scrollToWorkspace}>逐页检查</button>
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
          action={retryImageJob ? <button className="button secondary" type="button" disabled={imageGenerating} onClick={retryIllustration}>{retryImageJobActionLabel}</button> : visibleNotice.action}
        />
      )}

      {deleteOpen && (
        <DeleteStorybookModal
          title={book.title}
          deleting={deleting}
          onClose={() => !deleting && setDeleteOpen(false)}
          onConfirm={() => void confirmDeleteBook()}
        />
      )}

      {metaOpen && (
        <EditStorybookMetaModal
          form={metaForm}
          saving={metaSaving}
          onClose={() => setMetaOpen(false)}
          onChange={setMetaForm}
          onSubmit={saveMetadata}
        />
      )}

      {duplicateOpen && (
        <DuplicateStorybookModal
          title={duplicateTitle}
          duplicating={duplicating}
          onClose={() => setDuplicateOpen(false)}
          onTitleChange={setDuplicateTitle}
          onSubmit={duplicateCurrentStorybook}
        />
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
              <details className="section-tools">
                <summary>历史候选图和生成依据</summary>
                <ImageVariantStrip
                  workspaceId={workspace.id}
                  variants={roleImageVariants}
                  selectingVariantId={selectingVariantId}
                  emptyText="还没有历史参考图"
                  onSelect={selectImageVariant}
                  onZoom={(src) => setZoomedImage({ src, alt: `${selectedRole.name} 的候选参考图` })}
                />
                {selectedRoleNeedsReference ? (
                  <div className="reference-prompt-preview">
                    <div>
                      <strong>参考图生成依据</strong>
                      <span>由角色名称、视觉类型和外观设定自动生成；故事作用不参与参考图，避免把剧情动作画进角色标准照。</span>
                    </div>
                    <details className="prompt-details">
                      <summary>查看生成依据</summary>
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
              </details>
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

      <Card className="completion-review-panel">
        <div className="completion-review-head">
          <div>
            <Badge tone={reviewPanelTone}>{reviewPanelStatus}</Badge>
            <h2>{canDeliver ? "作品已准备好预览、导出或分享" : "浏览整本作品，只处理不满意的页面"}</h2>
            <p>建议至少预览封面和第 1 页，再完成作品、导出或分享。</p>
          </div>
        </div>
        <div className="completion-review-stats">
          <div>
            <span>插图完成</span>
            <strong>{readyPageCount} / {book.pages.length} 页</strong>
          </div>
          <div>
            <span>待处理</span>
            <strong>{bulkImageTotal + issuePageCount} 项</strong>
          </div>
          <div>
            <span>人工复核</span>
            <strong>{teacherReviewLabel(book.teacherReviewStatus)}</strong>
          </div>
        </div>
        <div className="completion-review-footer">
          <div className="completion-review-materials" aria-label="故事素材">
            <span>这本故事使用了</span>
            {reviewMaterialLabels.slice(0, 6).map((label) => (
              <Badge key={label} tone="neutral">{label}</Badge>
            ))}
          </div>
          {book.teacherReviewStatus !== "confirmed" && (
            <ActionButton className="button secondary" disabled={reviewSaving || quality?.status === "blocked"} disabledHint={quality?.status === "blocked" ? "质量检查仍有阻断项，请先修正" : undefined} onClick={() => saveTeacherReview("confirmed")}>
              <CheckCircle2 size={16} />{reviewSaving ? "记录中..." : "确认已人工复核"}
            </ActionButton>
          )}
        </div>
      </Card>

      <div className="workspace-section-head">
        <p className="eyebrow">验收工作台</p>
        <h2>逐页检查与调整</h2>
      </div>
      {(bulkImageSteps.length > 0 || (shouldShowBulkImageAction && bulkImageTotal > 0)) && (
        <Card className="bulk-image-progress-card">
          <div className="bulk-image-progress-head">
            <div>
              <Badge tone={bulkImageGenerating ? "info" : bulkImageSteps.some((step) => step.status === "failed") ? "danger" : bulkImageTotal > 0 ? "neutral" : "good"}>
                {bulkImageGenerating ? "生成中" : bulkImageSteps.some((step) => step.status === "failed") ? "需要处理" : bulkImageTotal > 0 ? "待生成" : "已完成"}
              </Badge>
              <h2>{bulkImageTotal > 0 ? "一键生成整本插图" : "整本插图已准备好"}</h2>
              <p>{bulkImageTotal > 0 ? "会先准备角色参考图，再依次生成封面和缺少插图的分页，保证人物形象一致。" : "封面和分页都有插图，可以继续验收、导出或分享。"}</p>
            </div>
          </div>
          {bulkImageSteps.length > 0 ? (
            <ol>
              {bulkImageSteps.map((step) => (
                <li key={step.id} className={step.status}>
                  <span>{bulkImageStepIcon(step.status)}</span>
                  <div>
                    <strong>{step.label}</strong>
                    <small>{bulkImageStepCopy(step)}</small>
                  </div>
                </li>
              ))}
            </ol>
          ) : (
            <p className="task-summary">待生成：{missingCoverImage ? "封面图" : ""}{missingCoverImage && missingPageImages.length ? "、" : ""}{missingPageImages.length ? `${missingPageImages.length} 页分页插图` : ""}。</p>
          )}
        </Card>
      )}
      <section className="detail-layout" id="page-workspace">
        <aside className="page-strip">
          <h2>页面</h2>
          <button type="button" className={`page-thumb cover-thumb ${selectedViewIsCover ? "active" : ""}`} onClick={() => setSelectedPageId(COVER_PAGE_ID)}>
            <span>封面</span>
            <strong>{book.title}</strong>
            <Badge tone="good">导出首页</Badge>
          </button>
          {book.pages.map((page) => (
            <button key={page.id} type="button" className={`page-thumb ${!selectedViewIsCover && selectedPage?.id === page.id ? "active" : ""}`} onClick={() => setSelectedPageId(page.id)}>
              <span>第 {page.pageNumber} 页</span>
              <strong>{page.title}</strong>
              <Badge tone={statusTone(page.status)}>{pageStatusLabel[page.status]}</Badge>
            </button>
          ))}
        </aside>
        <div className="storybook-workspace-main" ref={workspaceMainRef}>
          {selectedViewIsCover ? (
            <Card className="preview-panel cover-preview-panel">
              <div className="cover-page-preview">
                {coverImagePreviewUrl ? (
                  <button className="cover-image-zoom-trigger" type="button" style={{ aspectRatio: pageAspectCssRatio(book.pageAspectRatio) }} title="点击放大查看" onClick={() => setZoomedImage({ src: coverImagePreviewUrl, alt: currentCoverImage?.altText || `${book.title}封面图` })}>
                    <img src={coverImagePreviewUrl} alt={currentCoverImage?.altText || `${book.title}封面图`} />
                  </button>
                ) : coverImagePreviewError ? (
                  <p>封面图读取失败：{coverImagePreviewError}</p>
                ) : activeCoverImageJob ? (
                  <div className="cover-image-placeholder" style={{ aspectRatio: pageAspectCssRatio(book.pageAspectRatio) }}>
                    <strong>正在生成封面图</strong>
                    <small>任务{generationStatusLabel(activeCoverImageJob.status)}，编号：{activeCoverImageJob.id.slice(0, 8)}。</small>
                  </div>
                ) : (
                  <div className="cover-image-placeholder" style={{ aspectRatio: pageAspectCssRatio(book.pageAspectRatio) }}>
                    <strong>封面图待生成</strong>
                    <small>生成后会显示在{pageAspectLabel(book.pageAspectRatio)}主视觉位置。</small>
                  </div>
                )}
                <span>Kindleaf 绘本</span>
                <h2>{book.title}</h2>
                <p>{book.teachingGoal}</p>
                <div className="cover-page-meta">
                  <Badge tone="neutral">{book.ageGroup}</Badge>
                  <Badge tone="neutral">{book.useScene}</Badge>
                  <Badge tone="info">{book.coverTone}</Badge>
                </div>
              </div>
              <div className="cover-review-grid">
                <div>
                  <span>主要角色</span>
                  <strong>{book.roles.length ? book.roles.map((role) => role.name).join("、") : "待确认"}</strong>
                </div>
                <div>
                  <span>正文页数</span>
                  <strong>{book.pages.length} 页</strong>
                </div>
                <div>
                  <span>导出位置</span>
                  <strong>PDF 第 1 页</strong>
                </div>
              </div>
              {currentCoverImage?.prompt && (
                <details className="prompt-details">
                  <summary>查看封面生成依据</summary>
                  <p>{currentCoverImage.prompt}</p>
                </details>
              )}
              <ImageVariantStrip
                workspaceId={workspace.id}
                variants={coverImageVariants}
                selectingVariantId={selectingVariantId}
                emptyText="还没有历史封面图"
                aspectRatio={pageAspectCssRatio(book.pageAspectRatio)}
                onSelect={selectImageVariant}
                onZoom={(src) => setZoomedImage({ src, alt: `${book.title} 的候选封面图` })}
              />
              <div className="image-generation-action-bar">
                <div>
                  <strong>{currentCoverImage ? "对当前封面图不满意？" : "先生成封面图"}</strong>
                  <span>{currentCoverImage ? "会保留原图，并新增一张候选封面图。" : "封面图会使用绘本信息和角色参考图自动生成。"}</span>
                </div>
                <button
                  className="button primary"
                  type="button"
                  disabled={imageGenerating || Boolean(activeCoverImageJob)}
                  onClick={generateCoverImage}
                >
                  {imageGenerating || activeCoverImageJob ? "生成中..." : currentCoverImage ? "重新生成封面图" : "生成封面图"}
                </button>
              </div>
              <div className="reference-guard-callout">
                <Badge tone="good">封面已包含</Badge>
                <div>
                  <strong>导出 PDF 时会使用当前封面图</strong>
                  <span>正文分页从第 1 页开始；封面图、标题和绘本信息会一起出现在 PDF 首页。</span>
                </div>
                <button className="button secondary" type="button" onClick={() => setMetaOpen(true)}>编辑绘本信息</button>
              </div>
            </Card>
          ) : selectedPage && (
          <Card className="preview-panel">
            <div className="page-content-toolbar">
              <div className="page-content-meta">
                <span>第 {selectedPage.pageNumber} 页</span>
                <small>《{book.title}》 · {book.coverTone}</small>
              </div>
              {!pageEditorOpen && (
                <button className="button secondary" type="button" onClick={() => setPageEditorOpen(true)}>
                  编辑文字
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
                <details className="prompt-details prompt-details-compact">
                  <summary>
                    <span>插图描述</span>
                    <Badge tone="neutral">{illustrationShotLabel(selectedPage.illustrationPrompt)}</Badge>
                    <em>{compactPromptSummary(selectedPage.illustrationPrompt)}</em>
                  </summary>
                  <p>{selectedPage.illustrationPrompt}</p>
                </details>
              </>
            )}
            {activeCurrentPageImageJob ? (
              <div className="preview-image-block">
                <Badge tone="info">当前页插图生成中</Badge>
                <div className="image-placeholder-note">
                  <strong>正在生成真实插图</strong>
                  <span>
                    当前{generationStatusLabel(activeCurrentPageImageJob.status)}，请稍等。
                  </span>
                </div>
                <details className="prompt-details prompt-details-compact">
                  <summary>
                    <span>生成中的插图描述</span>
                    <Badge tone="neutral">{illustrationShotLabel(pageForm.illustrationPrompt || selectedPage.illustrationPrompt)}</Badge>
                    <em>{compactPromptSummary(pageForm.illustrationPrompt || selectedPage.illustrationPrompt)}</em>
                  </summary>
                  <p>{pageForm.illustrationPrompt || selectedPage.illustrationPrompt}</p>
                </details>
                <small>完成后这里会自动刷新为真实插图。</small>
              </div>
            ) : currentPageImage && (
              <div className="preview-image-block">
                <Badge tone="info">当前页插图结果</Badge>
                {currentImagePreviewUrl ? (
                  <button className="image-zoom-trigger current-image-preview" type="button" style={{ aspectRatio: pageAspectCssRatio(book.pageAspectRatio) }} title="点击放大查看" onClick={() => setZoomedImage({ src: currentImagePreviewUrl, alt: currentPageImage.altText || selectedPage.title })}>
                    <img src={currentImagePreviewUrl} alt={currentPageImage.altText || selectedPage.title} />
                  </button>
                ) : currentImagePreviewError ? (
                  <p>插图文件读取失败：{currentImagePreviewError}</p>
                ) : (
                  <p>正在读取当前登录态下的插图文件。</p>
                )}
                <details className="prompt-details">
                  <summary>查看生成依据</summary>
                  <p>{currentPageImage.prompt}</p>
                </details>
                <small>{currentPageImage.styleNotes.join(" · ")}</small>
              </div>
            )}
            <ImageVariantStrip
              workspaceId={workspace.id}
              variants={pageImageVariants}
              selectingVariantId={selectingVariantId}
              emptyText="还没有历史插图"
              aspectRatio={pageAspectCssRatio(book.pageAspectRatio)}
              onSelect={selectImageVariant}
              onZoom={(src) => setZoomedImage({ src, alt: `${selectedPage.title} 的候选插图` })}
            />
            {shouldShowImageGenerationAction && !pageEditorOpen && (
              <div className="image-generation-action-bar">
                <div>
                  <strong>{currentPageImage ? "对当前插图不满意？" : "本页还没有插图"}</strong>
                  <span>{pageImageReferenceBlocker ? "先补齐本页角色参考图，再重新生成插图。" : currentPageImage ? "会保留原图，并新增一张候选插图。" : "按当前插图描述生成第一张插图。"}</span>
                </div>
                <button
                  className="button primary"
                  type="button"
                  disabled={imageActionBusy || promptRewriting || Boolean(pageImageReferenceBlocker)}
                  title={pageImageReferenceBlocker || undefined}
                  onClick={generateIllustration}
                >
                  {pageImageActionLabel(selectedPage.status, imageActionBusy)}
                </button>
                <button className="button secondary mobile-inline-page-edit" type="button" onClick={() => setPageEditorOpen(true)}>
                  编辑文字
                </button>
              </div>
            )}
          </Card>
          )}
          {!selectedViewIsCover && selectedPage && (
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
              <details className="section-tools">
                <summary>插图修正工具</summary>
                <div className="rewrite-prompt-action">
                  <div>
                    <strong>想先优化画面描述？</strong>
                    <span>让 AI 根据本页正文重新整理插图描述，确认后再去插图区域重绘。</span>
                  </div>
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
              </details>
            )}
          </Card>
          </aside>
          )}
        </div>
      </section>

      {shareOpen && (
        <ShareLinksModal
          workspaceName={workspace.name}
          book={book}
          exportJobs={exportJobs}
          shareLinks={shareLinks}
          quality={quality}
          firstActionableQualityPage={firstActionableQualityPage}
          firstRoleNeedingReference={firstRoleNeedingReference}
          effectiveDeliveryBlocker={effectiveDeliveryBlocker}
          deliveryWarnings={deliveryWarnings}
          reviewDeliveryReminder={reviewDeliveryReminder}
          reviewSaving={reviewSaving}
          visibilityValue={visibilityValue}
          visibilitySaving={visibilitySaving}
          shareExpiry={shareExpiry}
          shareSaving={shareSaving}
          revokingShareId={revokingShareId}
          createdShareUrl={createdShareUrl}
          qualityDeliveryBlocker={qualityDeliveryBlocker}
          onClose={() => setShareOpen(false)}
          onVisibilityChange={setVisibilityValue}
          onSaveVisibility={saveVisibility}
          onShareExpiryChange={setShareExpiry}
          onCreateShare={createShare}
          onRevokeShare={revokeShare}
          onCopyShareUrl={copyShareUrl}
          onSaveTeacherReview={saveTeacherReview}
          onFocusQualityPage={(page) => { setShareOpen(false); focusQualityPage(page); }}
          onFocusRoleReference={(role) => { setShareOpen(false); focusRoleReference(role); }}
        />
      )}

      {zoomedImage && (
        <ImageLightbox src={zoomedImage.src} alt={zoomedImage.alt} onClose={() => setZoomedImage(null)} />
      )}
    </div>
  );
}

function bulkImageStepIcon(status: BulkImageStep["status"]) {
  if (status === "done" || status === "skipped") return "✓";
  if (status === "failed") return "!";
  if (status === "running") return "...";
  return "·";
}

function bulkImageStepCopy(step: BulkImageStep) {
  if (step.status === "done") return step.kind === "reference" ? "角色形象已锁定" : "已生成";
  if (step.status === "failed") return step.error || "生成失败，可重试";
  if (step.status === "skipped") return "已有插图，已跳过";
  if (step.status === "running") return "正在生成";
  return "等待生成";
}
