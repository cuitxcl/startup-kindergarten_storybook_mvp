import { ArrowRight, CheckCircle2, Copy, Download, Pencil, Send } from "lucide-react";
import { ChangeEvent, FormEvent, useEffect, useState } from "react";
import { Link, useLocation, useNavigate, useOutletContext, useParams } from "react-router-dom";
import {
  cancelGenerationJob,
  createPageImageTask,
  createRoleReferenceImageTask,
  createShareLink,
  createStorybookExport,
  downloadGenerationImageFile,
  downloadStorybookExportFile,
  duplicateStorybook,
  getStorybookExport,
  getGenerationJob,
  getStorybook,
  listShareLinksPage,
  listStorybookGenerationJobs,
  listStorybookExportsPage,
  revokeShareLink,
  retryGenerationJob,
  shouldUseApi,
  updateStorybook,
  updateStorybookPage,
  updateStorybookRole,
  type ExportJob,
  type GenerationJob,
  type ShareLink,
} from "../../api/client";
import { Badge, Card, EmptyState, Modal, Notice, PageHeader, statusTone } from "../../components/ui";
import { storybooks } from "../../data/mock";
import type { Storybook, StorybookQualityReport, StorybookRole, Workspace } from "../../types/domain";
import {
  generationJobNextAction,
  generationJobStatusLabel,
  generationJobTypeLabel,
  generationPrivacyAuditSummary,
  pageStatusLabel,
  storybookNextAction,
  storybookSourceLabel,
} from "../../utils/labels";

export function StorybookDetailPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const { storybookId } = useParams();
  const navigate = useNavigate();
  const location = useLocation();
  const fallbackBook = storybooks.find((item) => item.id === storybookId) || storybooks.find((item) => item.workspaceId === workspace.id) || storybooks[0];
  const [remoteBook, setRemoteBook] = useState<Storybook | null>(null);
  const [loading, setLoading] = useState(shouldUseApi);
  const [error, setError] = useState("");
  const book = shouldUseApi ? remoteBook : fallbackBook;
  const [selectedPageId, setSelectedPageId] = useState<string | undefined>(shouldUseApi ? undefined : fallbackBook.pages[0]?.id);
  const [pageForm, setPageForm] = useState({ title: "", body: "", illustrationPrompt: "" });
  const [notice, setNotice] = useState<{ title: string; copy: string; tone?: "good" | "info" } | null>(null);
  const [retryImageJob, setRetryImageJob] = useState<GenerationJob | null>(null);
  const [generationJobs, setGenerationJobs] = useState<GenerationJob[]>([]);
  const [cancelingJobId, setCancelingJobId] = useState<string | null>(null);
  const [exportJobs, setExportJobs] = useState<ExportJob[]>([]);
  const [shareOpen, setShareOpen] = useState(false);
  const [shareLinks, setShareLinks] = useState<ShareLink[]>([]);
  const [shareSaving, setShareSaving] = useState(false);
  const [revokingShareId, setRevokingShareId] = useState<string | null>(null);
  const [createdShareUrl, setCreatedShareUrl] = useState<string | null>(null);
  const [shareExpiry, setShareExpiry] = useState<"7d" | "30d" | "never">("7d");
  const [exporting, setExporting] = useState(false);
  const [duplicating, setDuplicating] = useState(false);
  const [duplicateOpen, setDuplicateOpen] = useState(false);
  const [duplicateTitle, setDuplicateTitle] = useState("");
  const [deliverySaving, setDeliverySaving] = useState(false);
  const [reviewSaving, setReviewSaving] = useState(false);
  const [metaOpen, setMetaOpen] = useState(false);
  const [metaSaving, setMetaSaving] = useState(false);
  const [metaForm, setMetaForm] = useState({
    title: shouldUseApi ? "" : fallbackBook.title,
    ageGroup: shouldUseApi ? "4-5 岁" : fallbackBook.ageGroup,
    useScene: shouldUseApi ? "" : fallbackBook.useScene,
    teachingGoal: shouldUseApi ? "" : fallbackBook.teachingGoal,
    coverTone: shouldUseApi ? "" : fallbackBook.coverTone,
  });
  const [imageGenerating, setImageGenerating] = useState(false);
  const [currentImagePreviewUrl, setCurrentImagePreviewUrl] = useState("");
  const [currentImagePreviewError, setCurrentImagePreviewError] = useState("");
  const [visibilitySaving, setVisibilitySaving] = useState(false);
  const [visibilityValue, setVisibilityValue] = useState<Storybook["visibility"]>(shouldUseApi ? "private" : fallbackBook.visibility);
  const [selectedRoleId, setSelectedRoleId] = useState<string | undefined>(shouldUseApi ? undefined : fallbackBook.roles[0]?.id);
  const [roleForm, setRoleForm] = useState<{
    name: string;
    roleType: StorybookRole["roleType"];
    appearance: string;
    storyFunction: string;
    needsConsistency: boolean;
    referenceImagePrompt: string;
  }>({ name: "", roleType: "teacher", appearance: "", storyFunction: "", needsConsistency: true, referenceImagePrompt: "" });
  const [roleSaving, setRoleSaving] = useState(false);
  const [roleImageGenerating, setRoleImageGenerating] = useState(false);
  const [roleReferencePreviewUrl, setRoleReferencePreviewUrl] = useState("");
  const [roleReferencePreviewError, setRoleReferencePreviewError] = useState("");
  const selectedPage = book?.pages.find((page) => page.id === selectedPageId) || book?.pages[0];
  const selectedRole = book?.roles.find((role) => role.id === selectedRoleId) || book?.roles[0];
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
  const canStartDelivery = canDeliver && !qualityDeliveryBlocker;
  const selectedPageQuality = selectedPage && quality
    ? quality.pages.find((page) => page.pageId === selectedPage.id)
    : undefined;
  const firstActionableQualityPage = quality?.pages.find((page) => page.status === "blocked")
    || quality?.pages.find((page) => page.status === "needs_review");
  const firstRoleNeedingReference = book?.roles.find((role) => role.needsConsistency && (role.referenceStatus !== "ready" || !role.referenceImageUrl));
  const selectedPageReferenceText = selectedPage
    ? `${pageForm.title || selectedPage.title} ${pageForm.body || selectedPage.body} ${pageForm.illustrationPrompt || selectedPage.illustrationPrompt}`
    : "";
  const selectedPageReferencedRoles = book?.roles.filter((role) => role.needsConsistency && selectedPageReferenceText.includes(role.name)) || [];
  const selectedPageReadyReferenceRoles = selectedPageReferencedRoles.filter((role) => role.referenceStatus === "ready" && role.referenceImageUrl);
  const selectedPageMissingReferenceRoles = selectedPageReferencedRoles.filter((role) => role.referenceStatus !== "ready" || !role.referenceImageUrl);
  const pageImageReferenceBlocker = selectedPageMissingReferenceRoles.length
    ? `本页提到了 ${selectedPageMissingReferenceRoles.map((role) => role.name).join("、")}，请先生成角色参考图再生成插图。`
    : "";
  const routeResultNotice = resultNoticeFromSearch(location.search);
  const visibleNotice = notice || routeResultNotice;

  useEffect(() => {
    if (!shouldUseApi || !storybookId) return;
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
          listStorybookGenerationJobs(workspace.id, item.id, { limit: 8 }),
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
      appearance: selectedRole.appearance,
      storyFunction: selectedRole.storyFunction,
      needsConsistency: selectedRole.needsConsistency,
      referenceImagePrompt: selectedRole.referenceImagePrompt || `${selectedRole.name}，${selectedRole.appearance}，儿童绘本角色参考图`,
    });
  }, [selectedRole?.id, selectedRole?.referenceImagePrompt]);

  function updatePageForm(event: ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) {
    setPageForm((current) => ({ ...current, [event.target.name]: event.target.value }));
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
    setNotice({
      title: `已定位到 ${role.name}`,
      copy: "请先检查角色设定并生成参考图，再继续分页插图和老师复核。",
      tone: "info",
    });
    window.setTimeout(() => document.getElementById("storybook-role-editor")?.scrollIntoView({ behavior: "smooth", block: "start" }), 0);
  }

  async function refreshShareLinks(storybookId = book?.id) {
    if (!shouldUseApi || !storybookId) return;
    setShareLinks((await listShareLinksPage(workspace.id, storybookId, { limit: 8 })).data);
  }

  async function refreshExportJobs(storybookId = book?.id) {
    if (!shouldUseApi || !storybookId) return;
    setExportJobs((await listStorybookExportsPage(workspace.id, storybookId, { limit: 8 })).data);
  }

  async function refreshGenerationJobs(storybookId = book?.id) {
    if (!shouldUseApi || !storybookId) return;
    setGenerationJobs(await listStorybookGenerationJobs(workspace.id, storybookId, { limit: 8 }));
  }

  async function refreshStorybook(storybookId = book?.id) {
    if (!shouldUseApi || !storybookId) return undefined;
    const updated = await getStorybook(workspace.id, storybookId);
    setRemoteBook(updated);
    setSelectedPageId((current) => current && updated.pages.some((page) => page.id === current) ? current : updated.pages[0]?.id);
    setSelectedRoleId((current) => current && updated.roles.some((role) => role.id === current) ? current : updated.roles[0]?.id);
    setVisibilityValue(updated.visibility);
    return updated;
  }

  const currentPageImageJob = latestPageImageJob(generationJobs, selectedPage?.id);
  const currentPageImage = extractImageResult(currentPageImageJob?.output);

  useEffect(() => {
    if (!currentPageImage) {
      setCurrentImagePreviewUrl("");
      setCurrentImagePreviewError("");
      return;
    }
    if (!shouldUseApi || !currentPageImageJob) {
      setCurrentImagePreviewUrl(currentPageImage.imageUrl);
      setCurrentImagePreviewError("");
      return;
    }
    let revokedUrl = "";
    let active = true;
    setCurrentImagePreviewUrl("");
    setCurrentImagePreviewError("");
    downloadGenerationImageFile(workspace.id, currentPageImageJob.id)
      .then((file) => {
        if (!active) return;
        revokedUrl = window.URL.createObjectURL(file);
        setCurrentImagePreviewUrl(revokedUrl);
      })
      .catch((err) => {
        if (active) {
          setCurrentImagePreviewUrl("");
          setCurrentImagePreviewError(err instanceof Error ? err.message : "插图文件读取失败");
        }
      });
    return () => {
      active = false;
      if (revokedUrl) window.URL.revokeObjectURL(revokedUrl);
    };
  }, [currentPageImage?.imageUrl, currentPageImageJob?.id, workspace.id]);

  useEffect(() => {
    if (!selectedRole?.referenceImageUrl) {
      setRoleReferencePreviewUrl("");
      setRoleReferencePreviewError("");
      return;
    }
    if (!shouldUseApi) {
      setRoleReferencePreviewUrl(selectedRole.referenceImageUrl);
      setRoleReferencePreviewError("");
      return;
    }
    const referenceJobId = generationJobIdFromImageUrl(selectedRole.referenceImageUrl);
    if (!referenceJobId) {
      setRoleReferencePreviewUrl("");
      setRoleReferencePreviewError("角色参考图地址缺少生成任务编号");
      return;
    }

    let revokedUrl = "";
    let active = true;
    setRoleReferencePreviewUrl("");
    setRoleReferencePreviewError("");
    downloadGenerationImageFile(workspace.id, referenceJobId)
      .then((file) => {
        if (!active) return;
        revokedUrl = window.URL.createObjectURL(file);
        setRoleReferencePreviewUrl(revokedUrl);
      })
      .catch((err) => {
        if (active) {
          setRoleReferencePreviewUrl("");
          setRoleReferencePreviewError(err instanceof Error ? err.message : "角色参考图读取失败");
        }
      });
    return () => {
      active = false;
      if (revokedUrl) window.URL.revokeObjectURL(revokedUrl);
    };
  }, [selectedRole?.id, selectedRole?.referenceImageUrl, workspace.id]);

  async function savePage() {
    if (!selectedPage || !storybookId) return;
    if (!shouldUseApi) {
      setNotice({ title: "当前页已保存", copy: `原型模式：第 ${selectedPage.pageNumber} 页修改已记录。`, tone: "good" });
      setRetryImageJob(null);
      return;
    }
    try {
      const updated = await updateStorybookPage(workspace.id, storybookId, selectedPage.id, {
        title: pageForm.title,
        body: pageForm.body,
        illustrationPrompt: pageForm.illustrationPrompt,
      });
      await refreshGenerationJobs(storybookId);
      await refreshStorybook(storybookId);
      setNotice({ title: "当前页已保存", copy: `第 ${updated.pageNumber} 页修改已写入后端。`, tone: "good" });
      setRetryImageJob(null);
    } catch (err) {
      setNotice({ title: "保存失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    }
  }

  async function saveVisibility() {
    if (!book) return;
    if (!shouldUseApi) {
      setNotice({ title: "共享设置已保存", copy: `当前绘本已设置为：${visibilityLabel(visibilityValue)}。`, tone: "good" });
      setRetryImageJob(null);
      return;
    }
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
    if (!shouldUseApi) {
      setNotice({ title: "角色设定已保存", copy: `《${book.title}》中的 ${roleForm.name} 已更新。`, tone: "good" });
      setRetryImageJob(null);
      return;
    }
    setRoleSaving(true);
    try {
      const updated = await updateStorybookRole(workspace.id, book.id, selectedRole.id, {
        name: roleForm.name,
        roleType: roleForm.roleType,
        appearance: roleForm.appearance,
        storyFunction: roleForm.storyFunction,
        needsConsistency: roleForm.needsConsistency,
        referenceImagePrompt: roleForm.referenceImagePrompt,
      });
      await refreshGenerationJobs(book.id);
      await refreshStorybook(book.id);
      setNotice({ title: "角色设定已保存", copy: `${updated.name} 的外观和故事作用已写入后端。`, tone: "good" });
      setRetryImageJob(null);
    } catch (err) {
      setNotice({ title: "角色保存失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setRoleSaving(false);
    }
  }

  async function generateRoleReferenceImage() {
    if (!book || !selectedRole) return;
    if (!shouldUseApi) {
      setNotice({ title: "角色参考图已生成", copy: "原型模式：角色参考图会用于后续分页插图。", tone: "good" });
      return;
    }
    setRoleImageGenerating(true);
    try {
      const job = await createRoleReferenceImageTask(workspace.id, book.id, selectedRole.id, {
        prompt: roleForm.referenceImagePrompt || `${roleForm.name}，${roleForm.appearance}，儿童绘本角色参考图`,
        referenceImageUrls: selectedRole.referenceImageUrl ? [selectedRole.referenceImageUrl] : [],
        imageMode: selectedRole.referenceImageUrl ? "edit_image" : "text_to_image",
        editInstruction: selectedRole.referenceImageUrl ? "保持角色核心形象一致，优化为更清晰稳定的角色参考图。" : undefined,
        strength: selectedRole.referenceImageUrl ? 0.45 : undefined,
      });
      setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
      const settledJob = await waitForGenerationJob(job);
      await refreshGenerationJobs(book.id);
      const updated = await getStorybook(workspace.id, book.id);
      setRemoteBook(updated);
      const updatedRole = updated.roles.find((role) => role.id === selectedRole.id);
      setNotice({
        title: settledJob.status === "failed" ? "角色参考图生成失败" : "角色参考图已生成",
        copy: settledJob.status === "failed"
          ? `${generationErrorMessage(settledJob)}。任务编号：${settledJob.id.slice(0, 8)}。`
          : `${updatedRole?.name || selectedRole.name} 的参考图已写回角色，后续插图会优先引用。`,
        tone: settledJob.status === "failed" ? "info" : "good",
      });
    } catch (err) {
      setNotice({ title: "角色参考图生成失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    } finally {
      setRoleImageGenerating(false);
    }
  }

  async function generateIllustration() {
    if (!book || !selectedPage) return;
    if (pageImageReferenceBlocker) {
      setNotice({ title: "先补齐角色参考图", copy: pageImageReferenceBlocker, tone: "info" });
      if (selectedPageMissingReferenceRoles[0]) focusRoleReference(selectedPageMissingReferenceRoles[0]);
      return;
    }
    if (!shouldUseApi) {
      setNotice({ title: "插图任务已完成", copy: "原型模式：图片生成任务会更新当前页插图状态。", tone: "good" });
      setRetryImageJob(null);
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
      setGenerationJobs((jobs) => [job, ...jobs.filter((item) => item.id !== job.id)]);
      const settledJob = await waitForGenerationJob(job);
      await refreshGenerationJobs(book.id);
      await handleImageJob(settledJob);
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
      const settledJob = await waitForGenerationJob(job);
      await refreshGenerationJobs(book.id);
      await handleImageJob(settledJob);
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

  async function handleImageJob(job: GenerationJob) {
    if (!selectedPage) return;
    const currentPageId = selectedPage.id;
    if (job.status === "failed") {
      setRetryImageJob(job);
      setNotice({ title: "插图生成失败", copy: `${generationErrorMessage(job)}。任务编号：${job.id.slice(0, 8)}。`, tone: "info" });
      return;
    }
    if (job.status === "queued" || job.status === "running") {
      setNotice({ title: "插图仍在生成", copy: `任务${generationStatusLabel(job.status)}，稍后会继续刷新结果。`, tone: "info" });
      return;
    }
    setRetryImageJob(null);
    await refreshStorybook(book?.id);
    setSelectedPageId(currentPageId);
    setNotice({ title: "插图任务已完成", copy: `任务${generationStatusLabel(job.status)}，当前页插图和质量检查已刷新。`, tone: "good" });
  }

  async function waitForGenerationJob(initialJob: GenerationJob) {
    let currentJob = initialJob;
    for (let attempt = 0; attempt < 16 && ["queued", "running"].includes(currentJob.status); attempt += 1) {
      await new Promise((resolve) => window.setTimeout(resolve, 700));
      currentJob = await getGenerationJob(workspace.id, currentJob.id);
      setGenerationJobs((jobs) => [currentJob, ...jobs.filter((job) => job.id !== currentJob.id)]);
    }
    return currentJob;
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
    if (!shouldUseApi) {
      setNotice({ title: "PDF 导出已开始", copy: "原型模式：导出任务会显示下载状态。", tone: "good" });
      setRetryImageJob(null);
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
    if (!shouldUseApi) {
      setNotice({ title: "副本已创建", copy: "原型模式：会复制分页、角色和编辑内容，并进入新的私有草稿。", tone: "good" });
      setDuplicateOpen(false);
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
    if (!shouldUseApi) {
      setNotice({ title: "绘本已标记可交付", copy: "原型模式：列表和工作台会把它视为可导出绘本。", tone: "good" });
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
    if (!shouldUseApi) {
      setNotice({
        title: status === "confirmed" ? "老师复核已确认" : "已重新设为待复核",
        copy: status === "confirmed" ? "原型模式：这本绘本已记录为老师复核通过。" : "原型模式：内容会回到待老师复核状态。",
        tone: "good",
      });
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
    if (!shouldUseApi) {
      setMetaOpen(false);
      setNotice({ title: "绘本信息已保存", copy: `原型模式：绘本信息会更新为《${metaForm.title.trim()}》。`, tone: "good" });
      return;
    }
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
    if (!shouldUseApi) {
      if (job.fileUrl) window.open(job.fileUrl, "_blank", "noopener,noreferrer");
      return;
    }
    try {
      const file = await downloadStorybookExportFile(workspace.id, book.id, job.id);
      const url = window.URL.createObjectURL(file);
      window.open(url, "_blank", "noopener,noreferrer");
      window.setTimeout(() => window.URL.revokeObjectURL(url), 60_000);
      setNotice({ title: "PDF 已打开", copy: "已通过当前登录态下载导出文件。", tone: "good" });
    } catch (err) {
      setNotice({ title: "PDF 打开失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "info" });
    }
  }

  async function waitForExportJob(storybookId: string, initialJob: ExportJob) {
    let currentJob = initialJob;
    setExportJobs((jobs) => [currentJob, ...jobs.filter((job) => job.id !== currentJob.id)]);
    for (let attempt = 0; attempt < 5 && ["queued", "running"].includes(currentJob.status); attempt += 1) {
      await new Promise((resolve) => window.setTimeout(resolve, 700));
      currentJob = await getStorybookExport(workspace.id, storybookId, currentJob.id);
      setExportJobs((jobs) => [currentJob, ...jobs.filter((job) => job.id !== currentJob.id)]);
    }
    return currentJob;
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
    if (!shouldUseApi) return;
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
    const fullUrl = absoluteShareUrl(link.url);
    setNotice({ title: "分享链接已准备复制", copy: fullUrl, tone: "good" });
    copyText(fullUrl).catch(() => undefined);
  }

  if (loading) {
    return <div className="page-stack"><Notice title="正在读取绘本" copy="正在从后端加载绘本详情。" tone="info" /></div>;
  }

  if (error || !book || !selectedPage) {
    return <div className="page-stack"><Notice title="绘本详情加载失败" copy={error || "当前绘本不存在"} tone="info" /></div>;
  }

  return (
    <div className="page-stack">
      <PageHeader
        eyebrow={book.type === "plain" ? "普通绘本详情" : "定制绘本详情"}
        title={book.title}
        copy={`${book.teachingGoal}。${storybookSourceLabel(book)}。${storybookNextAction(book)}。当前内容归属：${workspace.name}`}
        actions={
          <>
            {book.type === "plain" && <Link className="button secondary" to="customize">生成定制版<ArrowRight size={16} /></Link>}
            <button className="button secondary" type="button" onClick={() => setMetaOpen(true)}><Pencil size={16} />编辑信息</button>
            {(book.status === "editing" || book.status === "image_pending") && (
              <button className="button secondary" type="button" disabled={deliverySaving || !canMarkDeliverable} title={!canMarkDeliverable ? deliveryBlockers.join("；") || "请等待当前绘本加载完成" : undefined} onClick={markDeliverable}><CheckCircle2 size={16} />{deliverySaving ? "确认中..." : "标记可交付"}</button>
            )}
            <button className="button secondary" type="button" disabled={duplicating} onClick={() => { setDuplicateTitle(`${book.title} 副本`); setDuplicateOpen(true); }}><Copy size={16} />{duplicating ? "复制中..." : "复制副本"}</button>
            <button className="button secondary" type="button" disabled={!canStartDelivery} title={!canDeliver ? "请先标记可交付" : qualityDeliveryBlocker || reviewDeliveryReminder || undefined} onClick={() => setShareOpen(true)}><Send size={16} />分享</button>
            <button className="button primary" type="button" disabled={exporting || !canStartDelivery} title={!canDeliver ? "请先标记可交付" : qualityDeliveryBlocker || reviewDeliveryReminder || undefined} onClick={exportPdf}><Download size={16} />{exporting ? "导出中..." : "导出 PDF"}</button>
          </>
        }
      />
      {visibleNotice && (
        <Notice
          title={visibleNotice.title}
          copy={visibleNotice.copy}
          tone={retryImageJob ? "danger" : visibleNotice.tone || "good"}
          action={retryImageJob ? <button className="button secondary" type="button" disabled={imageGenerating} onClick={retryIllustration}>重新生成插图</button> : undefined}
        />
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

      <section className="metric-grid">
        <Card>
          <Badge tone={book.visibility === "private" ? "neutral" : "good"}>当前可见性</Badge>
          <strong>{visibilityLabel(book.visibility)}</strong>
          <p>{book.visibility === "private" ? "当前版本仅空间内可见，适合继续编辑。" : "当前版本可以在空间内共享或继续发给家长。"}</p>
        </Card>
        <Card>
          <Badge tone={exporting ? "warn" : exportJobs.length ? statusTone(exportJobs[0].status) : "neutral"}>最近导出</Badge>
          <strong>{exporting ? "导出中" : exportJobs.length ? exportStatusLabel(exportJobs[0].status) : "暂无记录"}</strong>
          <p>{exporting ? "正在生成 PDF 文件，完成后会出现下载入口。" : exportJobs.length && exportJobs[0].fileUrl ? "已生成可下载 PDF 文件。" : "点击导出后会生成 PDF 任务。"}</p>
        </Card>
        <Card>
          <Badge tone={shareLinks.length ? "good" : "neutral"}>分享状态</Badge>
          <strong>{shareLinks.length ? `${shareLinks.length} 个链接` : "尚未创建"}</strong>
          <p>{shareLinks.length ? "可直接打开分享页交付家长。" : "先创建分享链接，再打开家庭版页面。"}</p>
        </Card>
        <Card>
          <Badge tone={deliveryBlockers.length ? "warn" : deliveryWarnings.length ? "info" : "good"}>交付检查</Badge>
          <strong>{deliveryBlockers.length ? "还有阻断项" : deliveryWarnings.length ? "可交付，有提醒" : "可以交付"}</strong>
          <p>{deliveryBlockers[0] || deliveryWarnings[0] || (book.type === "custom" ? "定制版建议先确认隐私，再通过分享或导出发给家长。" : "普通版可先导出，再决定是否创建分享链接。")}</p>
        </Card>
        {quality && (
          <Card>
            <Badge tone={qualityTone(quality.status)}>生成质量</Badge>
            <strong>{qualityStatusLabel(quality.status)}</strong>
            <p>{quality.summary}</p>
          </Card>
        )}
        <Card>
          <Badge tone={book.teacherReviewStatus === "confirmed" ? "good" : "warn"}>老师复核</Badge>
          <strong>{teacherReviewLabel(book.teacherReviewStatus)}</strong>
          <p>{book.teacherReviewStatus === "confirmed" ? `已由老师确认。${book.teacherReviewedAt ? `确认时间：${book.teacherReviewedAt}。` : ""}` : "导出或分享前建议由老师最终阅读确认。"}</p>
        </Card>
        <Card>
          <Badge tone={book.source === "blank" ? "neutral" : "info"}>内容来源</Badge>
          <strong>{storybookSourceLabel(book)}</strong>
          <p>{book.source === "duplicate" ? "这是从已有绘本复制出的私有草稿，适合改成新的活动版本。" : book.source === "marketplace" ? "这是从市场复用的内容，可继续调整为本园版本。" : book.source === "derived" ? "这是基于普通绘本和儿童档案生成的定制副本。" : "这是从空白需求创建的原创绘本。"}</p>
        </Card>
      </section>

      {quality && (
        <Card className="quality-panel">
          <div className="section-head">
            <div>
              <p className="eyebrow">生成完成后检查</p>
              <h2>生成质量检查</h2>
            </div>
            <Badge tone={qualityTone(quality.status)}>{qualityStatusLabel(quality.status)}</Badge>
          </div>
          <p>{quality.summary}</p>
          {(firstActionableQualityPage || firstRoleNeedingReference) && (
            <div className="quality-action-callout">
              <div>
                <strong>{quality.status === "blocked" ? "先处理阻断项" : "建议先处理待复核项"}</strong>
                <span>
                  {firstActionableQualityPage
                    ? `第 ${firstActionableQualityPage.pageNumber} 页需要检查：${firstActionableQualityPage.issues[0] || firstActionableQualityPage.suggestions[0] || "请核对分页内容。"}`
                    : `${firstRoleNeedingReference?.name} 还没有可用参考图。`}
                </span>
              </div>
              <div className="inline-actions">
                {firstActionableQualityPage && (
                  <button className="button secondary" type="button" onClick={() => focusQualityPage(firstActionableQualityPage)}>
                    定位问题页
                  </button>
                )}
                {firstRoleNeedingReference && (
                  <button className="button secondary" type="button" onClick={() => focusRoleReference(firstRoleNeedingReference)}>
                    定位角色参考图
                  </button>
                )}
              </div>
            </div>
          )}
          <div className="review-confirm-row">
            <div>
              <strong>{teacherReviewLabel(book.teacherReviewStatus)}</strong>
              <span>{book.teacherReviewStatus === "confirmed" ? "后续修改分页或角色后会自动回到待复核。" : quality.status === "blocked" ? "生成质量检查仍有阻断项，请先修正后再记录老师复核。" : "老师确认故事、角色、插图和隐私边界都适合当前课堂或家庭场景后，再记录复核。"}</span>
            </div>
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
              <button className="quality-page-row" type="button" key={page.pageId} onClick={() => focusQualityPage(page)}>
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
        </Card>
      )}

      <section className="detail-layout">
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
        <Card className="preview-panel">
          <div className="storybook-preview-art"><span>{book.coverTone}</span><strong>{book.title}</strong></div>
          <h2>{selectedPage.title}</h2>
          <p>{selectedPage.body}</p>
          <div className="privacy-callout">插图描述：{selectedPage.illustrationPrompt}</div>
          {currentPageImage && (
            <div className="preview-image-block">
              <Badge tone="info">当前页插图结果</Badge>
              {currentImagePreviewUrl ? (
                <img src={currentImagePreviewUrl} alt={currentPageImage.altText || selectedPage.title} />
              ) : currentImagePreviewError ? (
                <p>插图文件读取失败：{currentImagePreviewError}</p>
              ) : (
                <p>正在读取当前登录态下的插图文件。</p>
              )}
              <p>{currentPageImage.prompt}</p>
              <small>{currentPageImage.styleNotes.join(" · ")}</small>
            </div>
          )}
        </Card>
        <aside className="editor-panel">
          <Card id="storybook-role-editor">
            <h2>角色与道具</h2>
            <div className="compact-list">
              {book.roles.map((role) => (
                <button className={`compact-row ${selectedRole?.id === role.id ? "active" : ""}`} type="button" key={role.id} onClick={() => setSelectedRoleId(role.id)}>
                  <div><strong>{role.name}</strong><span>{role.appearance}</span></div>
                  <div className="badge-stack">
                    <Badge>{roleLabelMap(role.roleType)}</Badge>
                    <Badge tone={role.referenceStatus === "ready" ? "good" : role.referenceStatus === "failed" ? "danger" : "neutral"}>{roleReferenceStatusLabel(role.referenceStatus)}</Badge>
                  </div>
                </button>
              ))}
            </div>
            {selectedRole && (
              <div className="form-stack">
                <div className="reference-preview">
                  {selectedRole.referenceImageUrl ? (
                    roleReferencePreviewUrl ? (
                      <img src={roleReferencePreviewUrl} alt={`${selectedRole.name} 的角色参考图`} />
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
                      {roleReferenceStatusLabel(selectedRole.referenceStatus)}
                    </Badge>
                    <p>先确认角色参考图，再生成分页插图，可以显著提高跨页形象一致性。</p>
                  </div>
                </div>
                <label>角色名称<input name="name" value={roleForm.name} onChange={updateRoleForm} /></label>
                <label>
                  类型
                  <select name="roleType" value={roleForm.roleType} onChange={updateRoleForm}>
                    <option value="protagonist">主角</option>
                    <option value="supporting">配角</option>
                    <option value="peer">同伴儿童</option>
                    <option value="teacher">老师形象</option>
                    <option value="prop">关键道具</option>
                  </select>
                </label>
                <label>外观设定<textarea name="appearance" rows={3} value={roleForm.appearance} onChange={updateRoleForm} /></label>
                <label>故事作用<textarea name="storyFunction" rows={3} value={roleForm.storyFunction} onChange={updateRoleForm} /></label>
                <label>参考图提示词<textarea name="referenceImagePrompt" rows={3} value={roleForm.referenceImagePrompt} onChange={updateRoleForm} /></label>
                <label className="check-row"><input type="checkbox" checked={roleForm.needsConsistency} onChange={(event) => setRoleForm((current) => ({ ...current, needsConsistency: event.target.checked }))} />跨页保持一致</label>
                <div className="inline-actions">
                  <button className="button secondary" type="button" disabled={roleSaving} onClick={saveRole}>{roleSaving ? "保存中..." : "保存角色设定"}</button>
                  <button className="button primary" type="button" disabled={roleImageGenerating} onClick={generateRoleReferenceImage}>
                    {roleImageGenerating ? "生成中..." : selectedRole.referenceImageUrl ? "重绘参考图" : "生成参考图"}
                  </button>
                </div>
              </div>
            )}
          </Card>
          <Card>
            <h2>共享设置</h2>
            <p>{book.type === "custom" ? "定制绘本通常包含儿童信息，建议保持私有，只通过受控分享链接交付。" : "普通绘本可以设为园所共享，作为后续定制绘本母本复用。"}</p>
            {shouldUseApi && <Badge tone={shareLinks.length ? "good" : "neutral"}>{shareLinks.length ? `${shareLinks.length} 个有效分享链接` : "暂无有效分享链接"}</Badge>}
            <label>
              可见范围
              <select value={visibilityValue} onChange={(event) => setVisibilityValue(event.target.value as Storybook["visibility"])}>
                <option value="private">仅当前空间私有</option>
                <option value="workspace">园所/空间内共享</option>
              </select>
            </label>
            <button className="button secondary" type="button" disabled={visibilitySaving || visibilityValue === book.visibility} onClick={saveVisibility}>
              {visibilitySaving ? "保存中..." : visibilityValue === book.visibility ? "已保存" : "保存共享设置"}
            </button>
            {shouldUseApi && (
              <div className="review-list">
                <span>最近导出</span>
                {exportJobs.length ? (
                  <div className="inline-actions">
                    <Badge tone={statusTone(exportJobs[0].status)}>{exportStatusLabel(exportJobs[0].status)}</Badge>
                    {exportJobs[0].fileUrl ? (
                      <button className="button secondary" type="button" onClick={() => openExportPdf(exportJobs[0])}>打开 PDF</button>
                    ) : (
                      <button className="button secondary" type="button" onClick={() => refreshExportJobs()}>刷新状态</button>
                    )}
                    {exportJobs[0].status === "failed" && <small>{exportFailureText(exportJobs[0])}</small>}
                  </div>
                ) : (
                  <p>还没有导出记录。</p>
                )}
              </div>
            )}
          </Card>
          {shouldUseApi && (
            <Card>
              <div className="section-head">
                <div>
                  <p className="eyebrow">Recent</p>
                  <h2>本书生成历史</h2>
                </div>
                <Badge tone={generationJobs.some((job) => job.status === "failed") ? "danger" : generationJobs.length ? "good" : "neutral"}>
                  {generationJobs.length ? `${generationJobs.length} 条` : "暂无记录"}
                </Badge>
              </div>
              {generationJobs.length === 0 ? (
                <EmptyState title="还没有生成记录" copy="故事方案、角色、分页、插图和定制方案完成后，会显示在这里。" />
              ) : (
                <div className="compact-list generation-job-list">
                  {generationJobs.slice(0, 4).map((job) => (
                    <div key={job.id} className="compact-row static">
                      <div>
                        <strong>{generationJobTitle(job)}</strong>
                        <span>{generationJobCopy(job)}</span>
                        <small>{generationJobNextAction(job)}</small>
                        <small>任务 {job.id.slice(0, 8)} · {generationJobTime(job)}</small>
                        {generationPrivacyAuditSummary(job.output) && <small>{generationPrivacyAuditSummary(job.output)}</small>}
                        {canCancelGenerationJob(job) && (
                          <button className="button secondary compact-action" type="button" disabled={cancelingJobId === job.id} onClick={() => cancelJob(job)}>
                            {cancelingJobId === job.id ? "取消中..." : "取消任务"}
                          </button>
                        )}
                      </div>
                      <Badge tone={statusTone(job.status)}>{generationJobStatusLabel[job.status] || job.status}</Badge>
                    </div>
                  ))}
                </div>
              )}
            </Card>
          )}
          <Card id="storybook-page-editor">
            <h2>编辑当前页</h2>
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
            <label>页面标题<input name="title" value={pageForm.title} onChange={updatePageForm} /></label>
            <label>正文<textarea name="body" rows={5} value={pageForm.body} onChange={updatePageForm} /></label>
            <label>插图描述<textarea name="illustrationPrompt" rows={4} value={pageForm.illustrationPrompt} onChange={updatePageForm} /></label>
            <div className={`reference-guard-callout ${pageImageReferenceBlocker ? "needs-reference" : ""}`}>
              <Badge tone={pageImageReferenceBlocker ? "warn" : selectedPageReadyReferenceRoles.length ? "good" : "neutral"}>插图参考图</Badge>
              <div>
                <strong>{pageImageReferenceBlocker ? "先补齐本页角色参考图" : selectedPageReadyReferenceRoles.length ? "本页会引用角色参考图" : "本页未识别到需固定形象的角色"}</strong>
                {selectedPageReadyReferenceRoles.length > 0 && (
                  <span>已就绪：{selectedPageReadyReferenceRoles.map((role) => role.name).join("、")}。</span>
                )}
                {selectedPageMissingReferenceRoles.length > 0 && (
                  <span>缺少参考图：{selectedPageMissingReferenceRoles.map((role) => role.name).join("、")}。</span>
                )}
                {!selectedPageReferencedRoles.length && (
                  <span>如果本页出现固定主角、老师或关键道具，请在插图描述中写出名称，系统才会带入对应参考图。</span>
                )}
              </div>
              {selectedPageMissingReferenceRoles[0] && (
                <button className="button secondary" type="button" onClick={() => focusRoleReference(selectedPageMissingReferenceRoles[0])}>
                  定位缺少的参考图
                </button>
              )}
            </div>
            <div className="inline-actions">
              <button className="button secondary" type="button" disabled={imageGenerating} onClick={generateIllustration}>
                {imageGenerating ? "生成中..." : pageImageReferenceBlocker ? "先生成参考图" : selectedPage.status === "needs_regeneration" ? "重新生成插图" : "生成插图"}
              </button>
              <button className="button primary" type="button" onClick={savePage}>保存本页</button>
            </div>
          </Card>
        </aside>
      </section>
      {shareOpen && (
        <Modal title="管理分享链接" onClose={() => setShareOpen(false)}>
          <p>分享范围：获得链接的人可查看当前绘本版本。</p>
          <p>当前空间：<strong>{workspace.name}</strong></p>
          <div className="review-list">
            <div><span>当前可见性</span><strong>{visibilityLabel(book.visibility)}</strong></div>
            <div><span>导出状态</span><strong>{exportJobs.length ? exportStatusLabel(exportJobs[0].status) : "暂无记录"}</strong></div>
            <div><span>分享链接</span><strong>{shareLinks.length ? `${shareLinks.length} 个有效链接` : "未创建"}</strong></div>
          </div>
          <div className="privacy-callout">
            分享前请确认不包含未授权儿童信息或家庭隐私。{reviewDeliveryReminder ? ` 分享弹窗提醒：${reviewDeliveryReminder}` : ""}
          </div>
          {shouldUseApi && (
            <div className="form-grid">
              <div>
                当前有效链接
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
                  <span>还没有有效分享链接。</span>
                )}
              </div>
              <label>
                链接有效期
                <select value={shareExpiry} onChange={(event) => setShareExpiry(event.target.value as "7d" | "30d" | "never")}>
                  <option value="7d">7 天有效</option>
                  <option value="30d">30 天有效</option>
                  <option value="never">不过期</option>
                </select>
              </label>
            </div>
          )}
          <div className="modal-actions">
            <button className="button secondary" type="button" onClick={() => setShareOpen(false)}>取消</button>
            {shouldUseApi ? (
              <>
                {createdShareUrl && <a className="button secondary" href={createdShareUrl} target="_blank" rel="noreferrer">打开最新分享页</a>}
                <button className="button primary" type="button" disabled={shareSaving || Boolean(qualityDeliveryBlocker)} title={qualityDeliveryBlocker || undefined} onClick={createShare}>
                  {shareSaving ? "处理中..." : "创建新的分享链接"}
                </button>
              </>
            ) : (
              <Link className="button primary" to="/link/share/demo-token">确认并打开分享页</Link>
            )}
          </div>
        </Modal>
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
  const consistencyRoles = book.roles.filter((role) => role.needsConsistency);
  const checks: StorybookQualityReport["checks"] = [
    {
      key: "structure",
      label: "内容结构",
      status: book.pages.length && book.roles.length ? "passed" : "blocked",
      message: book.pages.length && book.roles.length ? "已包含分页内容和角色/道具设定。" : "分页或角色设定不完整。",
    },
  ];
  const missingReferences = consistencyRoles.filter((role) => role.referenceStatus !== "ready" || !role.referenceImageUrl);
  checks.push({
    key: "role_references",
    label: "角色参考图",
    status: missingReferences.length ? "needs_review" : "passed",
    message: missingReferences.length
      ? `以下角色/道具还需要先生成或确认参考图：${missingReferences.map((role) => role.name).join("、")}。`
      : "需要跨页一致的角色/道具都已有参考图。",
  });

  let blockedPages = 0;
  let reviewPages = 0;
  const pages = book.pages.map((page) => {
    const issues: string[] = [];
    const suggestions: string[] = [];
    const promptHasConfirmedRole = consistencyRoles.length === 0 || consistencyRoles.some((role) => page.illustrationPrompt.includes(role.name));
    if (!promptHasConfirmedRole) issues.push("插图描述没有明确带入已确认角色/道具名称。");
    if (page.status === "generating") issues.push("插图仍在生成中。");
    if (page.status === "failed") issues.push("插图生成失败，需要重新生成。");
    if (page.status === "needs_regeneration") suggestions.push("当前页标记为需重绘，建议重新生成插图。");
    consistencyRoles.forEach((role) => {
      const pageText = `${page.title} ${page.body}`;
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

function roleLabelMap(roleType: string) {
  return {
    protagonist: "主角",
    supporting: "配角",
    peer: "同伴儿童",
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

function absoluteShareUrl(path: string) {
  if (/^https?:\/\//i.test(path)) return path;
  return `${window.location.origin}${path.startsWith("/") ? path : `/${path}`}`;
}

async function copyText(value: string) {
  if (navigator.clipboard?.writeText) {
    try {
      await Promise.race([
        navigator.clipboard.writeText(value),
        new Promise((_, reject) => window.setTimeout(() => reject(new Error("clipboard timeout")), 300)),
      ]);
      return;
    } catch {
      // Continue with the textarea fallback for browsers that expose clipboard but block it.
    }
  }
  const textArea = document.createElement("textarea");
  textArea.value = value;
  textArea.setAttribute("readonly", "true");
  textArea.style.position = "fixed";
  textArea.style.opacity = "0";
  document.body.appendChild(textArea);
  textArea.focus();
  textArea.select();
  const copied = document.execCommand("copy");
  document.body.removeChild(textArea);
  if (!copied) {
    throw new Error("浏览器没有允许复制，请打开分享页后手动复制地址。");
  }
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

function roleReferenceStatusLabel(status?: string) {
  return {
    not_started: "未生成参考图",
    generating: "参考图生成中",
    ready: "参考图已确认",
    needs_regeneration: "需要重绘",
    failed: "生成失败",
  }[status || "not_started"] || "参考图待确认";
}

function extractPageId(output: unknown) {
  const value = output as { image?: { page_id?: string; target_id?: string; target_type?: string } } | undefined;
  if (value?.image?.page_id) return value.image.page_id;
  return value?.image?.target_type === "page" ? value.image.target_id : undefined;
}

function extractImageResult(output: unknown): { imageUrl: string; altText?: string; prompt?: string; styleNotes: string[] } | null {
  const value = output as {
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
  };
}

function latestPageImageJob(jobs: GenerationJob[], pageId?: string) {
  if (!pageId) return undefined;
  return jobs
    .filter((job) => job.jobType === "storybook_page_image" && job.output && extractPageId(job.output) === pageId)
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

function resultNoticeFromSearch(search: string): { title: string; copy: string; tone: "good" } | null {
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
