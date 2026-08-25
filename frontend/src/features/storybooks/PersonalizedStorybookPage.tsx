import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation, useNavigate, useOutletContext } from "react-router-dom";
import { SlidersHorizontal, Trash2, X } from "lucide-react";
import {
  abandonStorybookCreationSession,
  abandonStorybookCustomizationRunItem,
  buildStorybookCustomizationPlan,
  cancelStorybookCustomizationRun,
  confirmStorybookVisualReference,
  createStorybookCreationSession,
  deriveCustomStorybooksBatch,
  generateStorybookCreationOutline,
  generateStorybookDirections,
  generateStorybookFromCreationSession,
  generateStorybookVisualReference,
  getStorybook,
  getStorybookAssetUploadPolicy,
  getStorybookCustomizationRun,
  getLatestStorybookCreationSession,
  getStorybookCreationSession,
  isApiClientError,
  revokeStorybookAssetReference,
  retryStorybookCustomizationRunItem,
  listChildrenPage,
  listStorybooksPage,
  patchStorybookCreationMaterials,
  refreshStorybookCreationUnderstanding,
  requestProtectedResourceBlob,
  selectStorybookCreationDirection,
  uploadStorybookCreationAsset,
  updateStorybookAssetReference,
  type PaginationMeta,
  type CreationMaterial,
  type ResponseWarning,
  type StoryDirection,
  type StorybookAssetReference,
  type StorybookAssetUploadPolicy,
  type StorybookCreationSession,
  type StorybookCustomizationRun,
  updateStorybookCreationOutlinePage,
  updateStorybookCreationSession,
  updateStorybookCreativeSettings,
} from "../../api/client";
import { ActionButton, Badge, Card, EmptyState, ImageLightbox, Modal, Notice, PageHeader, ProgressSteps, SkeletonBlock } from "../../components/ui";
import type { ChildProfile, Storybook, Workspace } from "../../types/domain";
import { customizationBlockerFor } from "./detail/helpers";
import { STORY_STYLE_PRESETS, STYLE_PRESETS } from "./new/presets";
import { PAGE_ASPECT_OPTIONS } from "../../utils/pageAspect";

const steps = ["对象与素材", "故事预览", "制作", "修改与交付"];
const DEFAULT_PHOTO_LIMIT = 5;
const SOURCE_PAGE_SIZE = 8;
const DEFAULT_ACCEPTED_PHOTO_TYPES = ["image/jpeg", "image/png", "image/webp"];

type EntryType = "direct_create" | "from_storybook";
type PhotoKind = "person" | "object" | "scene";
type VisualReferenceStatus = "awaiting_usage" | "generating" | "awaiting_reference" | "awaiting_confirmation" | "ready" | "failed" | "unused" | "revoked";
type SourceBatchResult = {
  sourceStorybookId: string;
  runId?: string;
  requestedCount: number;
  createdCount: number;
  storybooks: Storybook[];
  items: Array<{
    childId: string;
    runItemId?: string;
    status: string;
    storybook?: Storybook;
    storybookLoadFailed?: boolean;
    failureReason?: string;
  }>;
};

type SourceCustomizationPagePlan = {
  source_page_id?: string;
  page_number?: number;
  decision?: string;
  title?: string;
  reason?: string;
  asset_reference_ids?: string[];
  character_reference_ids?: string[];
  prop_reference_ids?: string[];
  scene_reference_ids?: string[];
};

type SourcePhotoReference = {
  asset_reference_id?: string;
  display_name?: string;
  reference_type?: string;
  reference_type_label?: string;
  kind?: PhotoKind;
  usage?: string;
  planned_pages?: Array<{ page_number?: number; title?: string }>;
  unplaced_reason?: string | null;
  placement_scope?: "page";
};

type SourceCustomizationPlan = {
  entry_type?: string;
  mode?: "single" | "batch";
  source_snapshot?: unknown;
  page_plan?: SourceCustomizationPagePlan[];
  optional_keep_page_ids?: string[];
  confirmed_photo_reference_ids?: string[];
  confirmed_photo_references?: SourcePhotoReference[];
};

type PhotoMaterial = {
  id: string;
  visualReferenceId?: string;
  visualReferencePreviewUrl?: string;
  name: string;
  displayName: string;
  fileName: string;
  previewUrl: string;
  kind: PhotoKind;
  usage: string;
  referenceStatus: VisualReferenceStatus;
  failureReason?: string;
};

type ZoomedImage = { src: string; alt: string };
type PhotoUploadBatchItem = {
  file: File;
  idempotencyKey: string;
  status: "pending" | "succeeded" | "failed";
};

type SourceBatchResultItem = SourceBatchResult["items"][number];

function useProtectedResourceUrl(src: string) {
  const [objectUrl, setObjectUrl] = useState("");
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    let nextUrl = "";
    setObjectUrl("");
    setFailed(false);
    requestProtectedResourceBlob(src)
      .then((blob) => {
        if (!active) return;
        nextUrl = URL.createObjectURL(blob);
        setObjectUrl(nextUrl);
      })
      .catch(() => {
        if (active) setFailed(true);
      });
    return () => {
      active = false;
      if (nextUrl) URL.revokeObjectURL(nextUrl);
    };
  }, [src]);

  return { objectUrl, failed };
}

function ProtectedPhotoThumbnail({ src, alt, kind, muted = false, onPreview }: { src: string; alt: string; kind: PhotoKind; muted?: boolean; onPreview: (image: ZoomedImage) => void }) {
  const { objectUrl, failed } = useProtectedResourceUrl(src);

  if (!objectUrl) {
    return <div className={`photo-thumb placeholder-thumb${muted ? " muted-thumb" : ""}`} aria-label={failed ? "照片预览加载失败" : "正在加载照片预览"}>{failed ? "!" : photoKindLabel(kind).slice(0, 1)}</div>;
  }
  return <button className="image-zoom-trigger photo-image-zoom-trigger" type="button" title="点击放大查看" onClick={() => onPreview({ src: objectUrl, alt })}><img className={`photo-thumb${muted ? " muted-thumb" : ""}`} src={objectUrl} alt={alt} /></button>;
}

function ProtectedVisualReferencePreview({ src, alt, onPreview }: { src: string; alt: string; onPreview: (image: ZoomedImage) => void }) {
  const { objectUrl, failed } = useProtectedResourceUrl(src);
  if (!objectUrl) {
    return <div className="visual-reference-preview ready-preview" aria-label={failed ? "同画风参考图加载失败" : "正在加载同画风参考图"}>{failed ? "!" : "参考"}</div>;
  }
  return <button className="image-zoom-trigger reference-image-zoom-trigger" type="button" title="点击放大查看" onClick={() => onPreview({ src: objectUrl, alt })}><img className="visual-reference-preview ready-preview" src={objectUrl} alt={alt} /></button>;
}

function batchResultItemPresentation(item: SourceBatchResultItem) {
  if (item.storybookLoadFailed) {
    return { label: "作品待加载", tone: "info" as const, detail: "作品已生成，暂时无法读取详情" };
  }
  if (item.status === "created") {
    return { label: "已创建", tone: "good" as const, detail: item.storybook?.title || "已创建专属绘本" };
  }
  if (item.status === "canceled") {
    return { label: "已排除", tone: "neutral" as const, detail: "已从本次批量制作中排除" };
  }
  if (["queued", "retrying"].includes(item.status)) {
    return { label: "排队中", tone: "info" as const, detail: "已进入制作队列，页面会自动更新进度" };
  }
  if (item.status === "running") {
    return { label: "制作中", tone: "info" as const, detail: "正在生成定制绘本" };
  }
  const reason = item.failureReason || "";
  const needsMaterial = /素材|照片|参考|主素材|asset|material|reference|primary/i.test(reason);
  return {
    label: needsMaterial ? "需补素材" : "失败",
    tone: needsMaterial ? "warn" as const : "danger" as const,
    detail: reason || (needsMaterial ? "需要补齐素材后再重试" : "等待处理"),
  };
}

function batchReviewUrl(workspaceId: string, storybookId: string, runItemId?: string) {
  const params = new URLSearchParams({ result: "batch-custom" });
  if (runItemId) params.set("run_item_id", runItemId);
  return `/app/${workspaceId}/storybooks/${storybookId}/review?${params.toString()}`;
}

function storybookStatusReason(book: Storybook) {
  const blocker = customizationBlockerFor(book, book.quality);
  if (!blocker) return { label: "可定制", tone: "good" as const, blocker: "" };
  if (book.type !== "plain") return { label: "不是普通绘本", tone: "neutral" as const, blocker };
  if (book.pages.some((page) => page.status === "generating") || ["draft", "plan_pending", "roles_pending", "editing", "image_pending"].includes(book.status)) {
    return { label: "仍在制作", tone: "warn" as const, blocker };
  }
  return { label: "需要处理", tone: "danger" as const, blocker };
}

function photoKindLabel(kind: PhotoKind) {
  if (kind === "person") return "人物";
  if (kind === "object") return "玩具/物品/宠物";
  return "场景";
}

function referenceStatusLabel(status: VisualReferenceStatus, kind?: PhotoKind) {
  const referenceLabel = kind ? referenceTypeLabel(kind) : "同画风参考";
  switch (status) {
    case "awaiting_usage": return "待确认用途";
    case "generating": return `正在生成${referenceLabel}`;
    case "awaiting_reference": return `等待生成${referenceLabel}`;
    case "awaiting_confirmation": return `待确认${referenceLabel}`;
    case "ready": return `${referenceLabel}已确认`;
    case "failed": return "参考生成失败";
    case "unused": return "本次不使用";
    case "revoked": return "已移除";
  }
}

function photoUsageOptions(kind: PhotoKind) {
  if (kind === "person") return ["主角", "故事里的朋友", "只保留名字", "不使用"];
  if (kind === "object") return ["把它写进故事", "不使用"];
  return ["用作故事场景", "不使用"];
}

function referenceTypeLabel(kind: PhotoKind) {
  if (kind === "person") return "角色参考";
  if (kind === "object") return "道具参考";
  return "场景参考";
}

function photoNamePrompt(kind: PhotoKind) {
  if (kind === "person") return "这是谁？";
  if (kind === "object") return "这是什么？";
  return "这是哪里？";
}

function photoNamePlaceholder(kind: PhotoKind) {
  if (kind === "person") return "例如：爸爸";
  if (kind === "object") return "例如：小汽车";
  return "例如：幼儿园门口";
}

function fallbackReferenceName(photo: PhotoMaterial) {
  const base = photo.displayName.trim() || `${referenceTypeLabel(photo.kind).replace("参考", "")}${photo.name.match(/\d+$/)?.[0] || ""}`;
  return `${base}的${referenceTypeLabel(photo.kind)}`;
}

function generationStages(session: StorybookCreationSession) {
  const { textStatus, imageStatus } = session.generationSummary;
  const textDone = textStatus === "succeeded";
  const imageActive = ["pending", "queued", "running", "generating"].includes(imageStatus);
  const imageDone = imageStatus === "succeeded" || imageStatus === "skipped";
  return [
    { label: "准备故事", state: "done" },
    { label: "完成文字", state: textDone ? "done" : "active" },
    { label: "绘制画面", state: imageDone ? "done" : imageActive ? "active" : "pending" },
    { label: "等待检查", state: session.status === "storybook_ready" ? "done" : "pending" },
  ];
}

function newIdempotencyKey() {
  return `creation-${crypto.randomUUID()}`;
}

function ideaValidationMessage(value: string) {
  const length = value.trim().length;
  if (length >= 8) return "";
  return length === 0
    ? "先写一句故事想法，至少 8 个有效字符。"
    : `还差 ${8 - length} 个字符。可以补充人物、物品或想解决的小问题。`;
}

function usageCodeForLabel(kind: PhotoKind, usage: string) {
  if (usage === "不使用") return "unused";
  if (kind === "person" && usage === "主角") return "main_character";
  if (kind === "person" && usage === "故事里的朋友") return "story_friend";
  if (kind === "person" && usage === "只保留名字") return "name_only";
  if (kind === "scene") return "background_scene";
  return "story_object";
}

function usageLabelForCode(kind: PhotoKind, usage?: string) {
  if (!usage) return "";
  if (usage === "unused") return "不使用";
  if (usage === "name_only") return "只保留名字";
  if (usage === "main_character") return "主角";
  if (usage === "story_friend") return "故事里的朋友";
  if (usage === "background_scene") return "用作故事场景";
  if (kind === "object") return "把它写进故事";
  return usage;
}

function statusForAssetReference(reference: StorybookAssetReference): VisualReferenceStatus {
  if (reference.status === "awaiting_reference" && ["queued", "generating"].includes(reference.visualReference?.status || "")) {
    return "generating";
  }
  if (reference.status === "awaiting_confirmation") return "awaiting_confirmation";
  if (reference.status === "ready") return "ready";
  if (reference.status === "failed") return "failed";
  if (reference.status === "unused") return "unused";
  if (reference.status === "revoked") return "revoked";
  if (reference.status === "awaiting_reference") return "awaiting_reference";
  return "awaiting_usage";
}

function sourcePlanFromApi(plan: unknown): SourceCustomizationPlan {
  if (!plan || typeof plan !== "object") return {};
  const value = plan as SourceCustomizationPlan;
  return {
    ...value,
    page_plan: Array.isArray(value.page_plan) ? value.page_plan : [],
    optional_keep_page_ids: Array.isArray(value.optional_keep_page_ids) ? value.optional_keep_page_ids : [],
    confirmed_photo_reference_ids: Array.isArray(value.confirmed_photo_reference_ids) ? value.confirmed_photo_reference_ids : [],
  };
}

function preserveSourceReferencePlacements(next: SourceCustomizationPlan, previous: SourceCustomizationPlan | null) {
  if (!previous?.page_plan?.length || !next.page_plan?.length) return next;
  const confirmedIds = new Set((next.confirmed_photo_references || []).map((reference) => reference.asset_reference_id).filter(Boolean));
  const pagePlan = next.page_plan.map((page) => {
    const prior = previous.page_plan?.find((item) => item.source_page_id === page.source_page_id);
    if (!prior || (page.decision !== "personalize" && page.decision !== "redraw_required")) return page;
    const keepKnownIds = (ids: string[] | undefined) => (ids || []).filter((id) => confirmedIds.has(id));
    return {
      ...page,
      character_reference_ids: [...new Set([...(page.character_reference_ids || []), ...keepKnownIds(prior.character_reference_ids)])],
      prop_reference_ids: [...new Set([...(page.prop_reference_ids || []), ...keepKnownIds(prior.prop_reference_ids)])],
      scene_reference_ids: [...new Set([...(page.scene_reference_ids || []), ...keepKnownIds(prior.scene_reference_ids)])],
    };
  });
  const confirmedPhotoReferences = (next.confirmed_photo_references || []).map((reference) => {
    const field = reference.reference_type === "character_reference"
      ? "character_reference_ids"
      : reference.reference_type === "scene_reference"
        ? "scene_reference_ids"
        : "prop_reference_ids";
    const plannedPages = pagePlan
      .filter((page) => Boolean(reference.asset_reference_id && page[field]?.includes(reference.asset_reference_id)))
      .map((page) => ({ page_number: page.page_number, title: page.title }));
    return { ...reference, placement_scope: "page" as const, planned_pages: plannedPages, unplaced_reason: plannedPages.length ? null : "page_selection_required" };
  });
  return { ...next, page_plan: pagePlan, confirmed_photo_references: confirmedPhotoReferences };
}

function sourceDecisionLabel(decision?: string) {
  if (decision === "keep") return "保持";
  if (decision === "prefer_keep") return "尽量保持";
  if (decision === "redraw_required") return "必须重绘";
  return "变成对象版本";
}

function sourceDecisionTone(decision?: string) {
  if (decision === "keep" || decision === "prefer_keep") return "good" as const;
  if (decision === "redraw_required") return "warn" as const;
  return "info" as const;
}

function photoMaterialFromAssetReference(reference: StorybookAssetReference, index: number): PhotoMaterial {
  return {
    id: reference.id,
    visualReferenceId: reference.visualReference?.id,
    visualReferencePreviewUrl: reference.visualReference?.previewUrl,
    name: `${photoKindLabel(reference.kind)}照片 ${index + 1}`,
    displayName: reference.displayName,
    fileName: `${photoKindLabel(reference.kind)}照片`,
    previewUrl: reference.previewUrl || "",
    kind: reference.kind,
    usage: usageLabelForCode(reference.kind, reference.usage),
    referenceStatus: statusForAssetReference(reference),
    failureReason: reference.visualReference?.failureReason,
  };
}

export function PersonalizedStorybookPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const location = useLocation();
  const navigate = useNavigate();
  const query = new URLSearchParams(location.search);
  const sourceStorybookId = query.get("sourceStorybookId");
  const sourceRunId = query.get("sourceRunId");
  const initialChildId = query.get("childId") || "";
  const [entryType, setEntryType] = useState<EntryType | null>(sourceStorybookId ? "from_storybook" : null);
  const [session, setSession] = useState<StorybookCreationSession | null>(null);
  const [latestDraft, setLatestDraft] = useState<StorybookCreationSession | null>(null);
  const [sourceBooks, setSourceBooks] = useState<Storybook[]>([]);
  const [sourceMeta, setSourceMeta] = useState<PaginationMeta | null>(null);
  const [selectedSource, setSelectedSource] = useState<Storybook | null>(null);
  const [children, setChildren] = useState<ChildProfile[]>([]);
  const [recipientMode, setRecipientMode] = useState<"single" | "batch">("single");
  const [selectedChildId, setSelectedChildId] = useState(initialChildId);
  const [singleMaterialChoice, setSingleMaterialChoice] = useState("");
  const [selectedBatchIds, setSelectedBatchIds] = useState<string[]>([]);
  const [batchMaterialChoices, setBatchMaterialChoices] = useState<Record<string, string>>({});
  const [sourceBatchResult, setSourceBatchResult] = useState<SourceBatchResult | null>(null);
  const [sourcePlan, setSourcePlan] = useState<SourceCustomizationPlan | null>(null);
  const [retryingRunItemId, setRetryingRunItemId] = useState<string | null>(null);
  const [reloadingRunItemId, setReloadingRunItemId] = useState<string | null>(null);
  const [abandoningRunItemId, setAbandoningRunItemId] = useState<string | null>(null);
  const [cancelingSourceRun, setCancelingSourceRun] = useState(false);
  const [cancelDirectCreationConfirmOpen, setCancelDirectCreationConfirmOpen] = useState(false);
  const [zoomedImage, setZoomedImage] = useState<ZoomedImage | null>(null);
  const [sourcePreviewReady, setSourcePreviewReady] = useState(false);
  const [sourceStep, setSourceStep] = useState(0);
  const [sourceMaxUnlockedStep, setSourceMaxUnlockedStep] = useState(0);
  const [sourceKeepPageIds, setSourceKeepPageIds] = useState<string[]>([]);
  const [sourceLoading, setSourceLoading] = useState(false);
  const [sourceLoadFailed, setSourceLoadFailed] = useState(false);
  const [idea, setIdea] = useState("");
  const [recipientName, setRecipientName] = useState("");
  const [editingIdea, setEditingIdea] = useState(false);
  const [editingMaterials, setEditingMaterials] = useState(false);
  const [directViewStep, setDirectViewStep] = useState<number | null>(null);
  const [newMaterial, setNewMaterial] = useState("");
  const [photoMaterials, setPhotoMaterials] = useState<PhotoMaterial[]>([]);
  const [photoKind, setPhotoKind] = useState<PhotoKind | null>(null);
  const [photoUploadModalOpen, setPhotoUploadModalOpen] = useState(false);
  const [photoUploadBatch, setPhotoUploadBatch] = useState<PhotoUploadBatchItem[]>([]);
  const [photoUploadError, setPhotoUploadError] = useState("");
  const [photoUploadInProgress, setPhotoUploadInProgress] = useState(false);
  const [photoPendingRemoval, setPhotoPendingRemoval] = useState<PhotoMaterial | null>(null);
  const [photoRemovalError, setPhotoRemovalError] = useState("");
  const [materialPendingRemoval, setMaterialPendingRemoval] = useState<CreationMaterial | null>(null);
  const [materialRemovalError, setMaterialRemovalError] = useState("");
  const [uploadPolicy, setUploadPolicy] = useState<StorybookAssetUploadPolicy | null>(null);
  const photoInputRef = useRef<HTMLInputElement>(null);
  const photoMaterialsRef = useRef<PhotoMaterial[]>([]);
  const restoredSourceRunRef = useRef<string | null>(null);
  const [selectedDirection, setSelectedDirection] = useState<StoryDirection | null>(null);
  const [pageInstruction, setPageInstruction] = useState<Record<number, string>>({});
  const [busy, setBusy] = useState<"loading" | "directions" | "outline" | "generating" | "source-generating" | `page-${number}` | null>("loading");
  const [error, setError] = useState("");
  const [creationWarnings, setCreationWarnings] = useState<ResponseWarning[]>([]);
  const [assetRecoveryNotice, setAssetRecoveryNotice] = useState<{ name: string } | null>(null);
  const [creativeSettingsSaving, setCreativeSettingsSaving] = useState(false);
  const [creativeSettingsOpen, setCreativeSettingsOpen] = useState(false);
  const [creativeSettingsNotice, setCreativeSettingsNotice] = useState("");
  const [pageCountDraft, setPageCountDraft] = useState("");
  const [storyStylesExpanded, setStoryStylesExpanded] = useState(false);
  const [visualStylesExpanded, setVisualStylesExpanded] = useState(false);
  const [advancedSettingsOpen, setAdvancedSettingsOpen] = useState(false);
  const [pendingVisualStyleChange, setPendingVisualStyleChange] = useState<{ styleId: string; affectedCount: number } | null>(null);

  function applyServerSession(next: StorybookCreationSession) {
    setSession((current) => {
      if (!current || current.id !== next.id) return next;
      return Date.parse(next.updatedAt) >= Date.parse(current.updatedAt) ? next : current;
    });
  }

  const showingRestoredBatchResult = Boolean(sourceRunId && sourceBatchResult);
  useEffect(() => {
    if (entryType === "from_storybook") {
      setSourceMaxUnlockedStep((current) => Math.max(current, sourceStep));
    }
  }, [entryType, sourceStep]);
  const directMaxUnlockedStep = session?.status === "storybook_ready" ? 3
    : session?.status === "generating" ? 2
      : session?.outline || session?.directions.length ? 1 : 0;
  const activeStep = entryType === "from_storybook"
    ? showingRestoredBatchResult ? 2 : sourceStep
    : directViewStep !== null && directViewStep <= directMaxUnlockedStep
      ? directViewStep
      : editingMaterials ? 0 : directMaxUnlockedStep;
  const maxUnlockedStep = entryType === "from_storybook"
    ? showingRestoredBatchResult ? 2 : sourceMaxUnlockedStep
    : directMaxUnlockedStep;

  useEffect(() => {
    if (entryType !== "direct_create") return;
    if (editingMaterials) setDirectViewStep(0);
  }, [editingMaterials, entryType]);

  useEffect(() => {
    if (entryType !== "direct_create" || directViewStep === null || directViewStep <= directMaxUnlockedStep) return;
    setDirectViewStep(directMaxUnlockedStep);
  }, [directMaxUnlockedStep, directViewStep, entryType]);

  const handleStepClick = (index: number) => {
    if (entryType === "from_storybook") {
      if (index === 0 && activeStep > 0) {
        returnToSourceMaterials();
      } else if (index === 1 && index <= sourceMaxUnlockedStep) {
        setSourceBatchResult(null);
        setSourceStep(1);
      } else if (index === 2 && sourceBatchResult) {
        setSourceStep(2);
      }
      return;
    }
    if (index > directMaxUnlockedStep) return;
    setDirectViewStep(index);
    setEditingMaterials(index === 0);
  };
  const materials = session?.materials || [];
  const directions = session?.directions || [];
  const outline = session?.outline;
  const photoUploadSucceededCount = photoUploadBatch.filter((item) => item.status === "succeeded").length;
  const photoUploadPendingCount = photoUploadBatch.length - photoUploadSucceededCount;

  useEffect(() => {
    let mounted = true;
    getLatestStorybookCreationSession(workspace.id)
      .then((draft) => { if (mounted) setLatestDraft(draft); })
      .catch(() => { if (mounted) setLatestDraft(null); })
      .finally(() => { if (mounted) setBusy(null); });
    return () => { mounted = false; };
  }, [workspace.id]);

  useEffect(() => {
    photoMaterialsRef.current = photoMaterials;
  }, [photoMaterials]);

  useEffect(() => {
    setSourcePlan(null);
    setSourcePreviewReady(false);
    if (sourceStep === 1) setSourceStep(0);
  }, [selectedSource?.id, recipientMode, selectedChildId, singleMaterialChoice, selectedBatchIds.join(",")]);

  useEffect(() => {
    setSourceMaxUnlockedStep(0);
  }, [entryType, selectedSource?.id]);

  useEffect(() => {
    setSingleMaterialChoice("");
  }, [selectedChildId]);

  useEffect(() => {
    if (!session) {
      setPhotoMaterials([]);
      setUploadPolicy(null);
      return;
    }
    setPhotoMaterials(
      session.assetReferences
        .filter((reference) => reference.status !== "revoked")
        .map(photoMaterialFromAssetReference),
    );
  }, [session]);

  useEffect(() => {
    if (!session) return;
    let mounted = true;
    getStorybookAssetUploadPolicy(workspace.id, session.id)
      .then((policy) => { if (mounted) setUploadPolicy(policy); })
      .catch(() => undefined);
    return () => { mounted = false; };
  }, [session?.id, workspace.id]);

  useEffect(() => {
    if (entryType !== "from_storybook") return;
    let mounted = true;
    setSourceLoading(true);
    Promise.all([
      listStorybooksPage(workspace.id, { limit: SOURCE_PAGE_SIZE, offset: 0 }),
      listChildrenPage(workspace.id, { limit: 30, offset: 0 }).catch(() => ({ data: [], meta: null })),
      sourceStorybookId ? getStorybook(workspace.id, sourceStorybookId).catch(() => null) : Promise.resolve(null),
    ])
      .then(([bookPage, childPage, source]) => {
        if (!mounted) return;
        const rows = source && !bookPage.data.some((book) => book.id === source.id)
          ? [source, ...bookPage.data]
          : bookPage.data;
        setSourceBooks(rows);
        setSourceMeta(bookPage.meta);
        setChildren(childPage.data);
        if (source) {
          setSelectedSource(source);
          setSourceLoadFailed(false);
        } else if (sourceStorybookId) {
          setSelectedSource(null);
          setSourceLoadFailed(true);
          setError("原来源绘本不存在、无权限或暂时无法读取。请重新选择一本可定制的普通绘本。");
        } else {
          setSourceLoadFailed(false);
        }
        if (sourceRunId) {
          restoreSourceRunFromServer(sourceRunId).catch((err) => {
            if (mounted) setError(err instanceof Error ? err.message : "无法恢复批量制作结果。");
          });
        }
      })
      .catch((err) => {
        if (mounted) setError(err instanceof Error ? err.message : "无法读取可定制绘本。");
      })
      .finally(() => {
        if (mounted) setSourceLoading(false);
      });
    return () => { mounted = false; };
  }, [entryType, sourceStorybookId, workspace.id]);

  useEffect(() => {
    if (entryType !== "from_storybook" || !sourceRunId || restoredSourceRunRef.current === sourceRunId) return;
    let mounted = true;
    setSourceLoading(true);
    restoreSourceRunFromServer(sourceRunId)
      .catch((err) => {
        if (!mounted) return;
        restoredSourceRunRef.current = null;
        setError(err instanceof Error ? err.message : "无法恢复批量制作结果。");
      })
      .finally(() => {
        if (mounted) setSourceLoading(false);
      });
    return () => { mounted = false; };
  }, [entryType, sourceRunId, workspace.id]);

  useEffect(() => {
    if (entryType !== "from_storybook" || !selectedSource) return;
    if (session?.entryType === "from_storybook_assets" && session.sourceStorybookId === selectedSource.id) return;
    let mounted = true;
    createStorybookCreationSession(workspace.id, {
      quickIdea: `基于《${selectedSource.title}》补充本次专属绘本照片素材`,
      entryType: "from_storybook_assets",
      sourceStorybookId: selectedSource.id,
      pageCount: selectedSource.pages.length || 6,
      useScene: selectedSource.useScene,
      ageGroup: selectedSource.ageGroup,
    })
      .then((assetSession) => {
        if (mounted) setSession(assetSession);
      })
      .catch((err) => {
        if (mounted) setError(err instanceof Error ? err.message : "来源书照片上传入口没有准备好，请稍后重试。");
      });
    return () => { mounted = false; };
  }, [entryType, selectedSource?.id, session?.entryType, session?.sourceStorybookId, workspace.id]);

  useEffect(() => {
    if (!session || session.status !== "generating") return;
    const timer = window.setInterval(() => {
      getStorybookCreationSession(workspace.id, session.id)
        .then((next) => {
          applyServerSession(next);
          if (next.storybookId && next.status === "storybook_ready") {
            navigate(`/app/${workspace.id}/storybooks/${next.storybookId}/review?result=personalized`, { replace: true });
          }
        })
        .catch(() => setError("生成状态暂时无法刷新，请检查网络后刷新页面继续查看。"));
    }, 2000);
    return () => window.clearInterval(timer);
  }, [navigate, session, workspace.id]);

  useEffect(() => {
    if (!session || session.status === "generating") return;
    const hasRunningVisualReference = session.assetReferences.some((reference) =>
      reference.status === "awaiting_reference" && ["queued", "generating"].includes(reference.visualReference?.status || ""),
    );
    if (!hasRunningVisualReference) return;
    const timer = window.setInterval(() => {
      getStorybookCreationSession(workspace.id, session.id)
        .then(applyServerSession)
        .catch(() => setError("照片参考状态暂时无法刷新，请检查网络后稍后重试。"));
    }, 2500);
    return () => window.clearInterval(timer);
  }, [session, workspace.id]);

  const selectedMaterialLabels = useMemo(() => materials.filter((item) => item.locked).map((item) => item.label), [materials]);
  const lockedMaterialCount = selectedMaterialLabels.length;
  const lockedMaterials = useMemo(() => materials.filter((item) => item.locked), [materials]);
  const selectedDirectionMaterialIds = selectedDirection?.materialIds || [];
  const missingDirectionMaterials = lockedMaterials.filter((item) => !selectedDirectionMaterialIds.includes(item.id));
  const missingOutlineMaterials = lockedMaterials.filter((material) => !outline?.pages.some((page) => page.materialIds.includes(material.id)));
  const activePhotoReferences = photoMaterials.filter((photo) => photo.referenceStatus !== "unused");
  const awaitingPhotoReferences = activePhotoReferences.filter((photo) => photo.referenceStatus !== "ready");
  const activePhotoCount = activePhotoReferences.length;
  const maxPhotoFiles = uploadPolicy?.maxFiles ?? DEFAULT_PHOTO_LIMIT;
  const remainingPhotoSlots = uploadPolicy?.remainingSlots ?? Math.max(0, maxPhotoFiles - activePhotoCount);
  const acceptedPhotoTypes = uploadPolicy?.acceptedContentTypes.length ? uploadPolicy.acceptedContentTypes : DEFAULT_ACCEPTED_PHOTO_TYPES;
  const photoAcceptAttr = acceptedPhotoTypes.join(",");
  const maxPhotoFileSizeBytes = uploadPolicy?.maxFileSizeBytes ?? 0;
  const selectedChild = children.find((child) => child.id === selectedChildId) || null;
  const latestReadyStorybookId = latestDraft?.status === "storybook_ready" ? latestDraft.storybookId : undefined;
  const batchSelectionsReady = selectedBatchIds.length > 0 && selectedBatchIds.every((childId) => Boolean(batchMaterialChoices[childId]));
  const singleSelectionReady = Boolean(selectedSource && selectedChild && singleMaterialChoice);
  const sourceBlocker = selectedSource ? storybookStatusReason(selectedSource).blocker : "";
  const hasPendingVisualReferences = Boolean(session?.assetReferences.some((reference) => !["ready", "unused", "revoked"].includes(reference.status)));

  useEffect(() => {
    if (!creativeSettingsNotice.includes("原参考图") || hasPendingVisualReferences) return;
    setCreativeSettingsNotice("");
  }, [creativeSettingsNotice, hasPendingVisualReferences]);

  const visualStylePreset = STYLE_PRESETS.find((preset) => preset.styleId === session?.visualStyleId) || STYLE_PRESETS[2];
  const storyStylePreset = STORY_STYLE_PRESETS.find((preset) => preset.storyStyleId === session?.storyStyleId) || STORY_STYLE_PRESETS[0];
  async function saveCreativeSettings(patch: { storyStyleId?: string; visualStyleId?: string; pageCount?: number; pageAspectRatio?: StorybookCreationSession["visualPreferences"]["pageAspectRatio"]; visualComplexity?: "simple" | "standard" | "rich"; characterConsistency?: "auto" | "speed" | "confirm_character"; confirmReferenceRegeneration?: boolean }): Promise<boolean> {
    if (!session) return false;
    setCreativeSettingsSaving(true);
    setError("");
    try {
      const response = await updateStorybookCreativeSettings(workspace.id, session.id, patch);
      const confirmedSession = patch.pageCount === undefined
        ? response.session
        : await getStorybookCreationSession(workspace.id, session.id);
      if (patch.pageCount !== undefined && confirmedSession.pageCount !== patch.pageCount) {
        throw new Error("页数没有保存成功，请重试。");
      }
      applyServerSession(confirmedSession);
      const notices = [
        response.effects.referencesInvalidated ? "已切换绘本风格，原参考图已回到待处理，请按新风格重新生成并确认。" : "",
        response.effects.requiresDirectionRefresh ? "故事风格已更新，原故事走向和大纲已清空，请重新整理故事。" : "",
        patch.pageCount !== undefined && response.effects.requiresOutlineRefresh ? "页数已更新，当前大纲已清空，请按所选故事重新生成。" : "",
        response.effects.requiresStorybookRegeneration ? "创作设定已更新，当前绘本已过期，请重新制作绘本分页。" : "",
      ].filter(Boolean);
      if (entryType === "from_storybook" && (response.effects.referencesInvalidated || response.effects.requiresDirectionRefresh)) {
        setSourcePlan(null);
        setSourcePreviewReady(false);
        setSourceStep(0);
        notices.push("请重新预览变化范围后再开始制作。");
      }
      setCreativeSettingsNotice(notices.join(" "));
      return true;
    } catch (err) {
      if (isApiClientError(err) && err.code === "style_change_requires_reference_regeneration") {
        const details = err.details as { affected_count?: number } | undefined;
        if (patch.visualStyleId) setPendingVisualStyleChange({ styleId: patch.visualStyleId, affectedCount: details?.affected_count || 0 });
      } else {
        setError(err instanceof Error ? err.message : "创作设定没有保存，请重试。");
        try { applyServerSession(await getStorybookCreationSession(workspace.id, session.id)); } catch { /* retain the last known server state */ }
      }
      return false;
    } finally { setCreativeSettingsSaving(false); }
  }

  useEffect(() => {
    setPageCountDraft(session ? String(session.pageCount) : "");
  }, [session?.id, session?.pageCount]);

  const creativeSettingsDrawer = session && creativeSettingsOpen ? (
    <Modal title="创作设定" className="creative-settings-drawer" onClose={() => !creativeSettingsSaving && setCreativeSettingsOpen(false)}>
      <div className="drawer-status"><span>风格即时保存，页数确认后保存</span><Badge tone={creativeSettingsSaving ? "warn" : "good"}>{creativeSettingsSaving ? "保存中" : "已保存"}</Badge></div>
      <div className="creative-settings-group"><span className="field-label">故事风格</span><div className="story-style-chips">{(storyStylesExpanded ? STORY_STYLE_PRESETS : STORY_STYLE_PRESETS.slice(0, 5)).map((preset) => <button key={preset.storyStyleId} type="button" className={`story-style-chip ${session.storyStyleId === preset.storyStyleId ? "active" : ""}`} disabled={creativeSettingsSaving} onClick={() => void saveCreativeSettings({ storyStyleId: preset.storyStyleId })}><strong>{preset.label}</strong><span>{preset.tag}</span></button>)}</div><button className="button ghost compact" type="button" onClick={() => setStoryStylesExpanded((value) => !value)}>{storyStylesExpanded ? "收起故事风格" : "更多故事风格"}</button></div>
      <div className="creative-settings-group"><span className="field-label">绘本风格</span><div className="style-preset-grid compact-style-grid">{(visualStylesExpanded ? STYLE_PRESETS : STYLE_PRESETS.slice(0, 4)).map((preset) => <button key={preset.styleId} type="button" className={`style-preset ${session.visualStyleId === preset.styleId ? "active" : ""}`} disabled={creativeSettingsSaving} onClick={() => void saveCreativeSettings({ visualStyleId: preset.styleId })}><img src={preset.image} alt={preset.label} /><span className="style-preset-caption"><strong>{preset.label}</strong><em>{preset.tag}</em></span></button>)}</div><button className="button ghost compact" type="button" onClick={() => setVisualStylesExpanded((value) => !value)}>{visualStylesExpanded ? "收起绘本风格" : "更多绘本风格"}</button></div>
      <div className="creative-settings-group"><span className="field-label">绘本页数</span>{entryType === "from_storybook" ? <p className="form-helper">沿用来源绘本：{session.pageCount} 页。定制版保留原书的页数与阅读节奏。</p> : <div className="inline-form"><label className="field-label compact-field" htmlFor="creative-page-count">页数（4-32 页）<input id="creative-page-count" type="number" min={4} max={32} step={1} value={pageCountDraft} disabled={creativeSettingsSaving} onChange={(event) => setPageCountDraft(event.target.value)} /></label><ActionButton className="button secondary" disabled={creativeSettingsSaving || !Number.isInteger(Number(pageCountDraft)) || Number(pageCountDraft) < 4 || Number(pageCountDraft) > 32 || Number(pageCountDraft) === session.pageCount} disabledHint={!Number.isInteger(Number(pageCountDraft)) || Number(pageCountDraft) < 4 || Number(pageCountDraft) > 32 ? "请输入 4 到 32 之间的整数" : "页数未变化或正在保存"} onClick={() => void saveCreativeSettings({ pageCount: Number(pageCountDraft) })}>保存页数</ActionButton></div>}</div>
      <div className="creative-settings-group"><span className="field-label">绘本比例</span><div className="page-aspect-options compact">{PAGE_ASPECT_OPTIONS.map((option) => <button key={option.value} type="button" className={`page-aspect-option ${session.visualPreferences.pageAspectRatio === option.value ? "active" : ""}`} disabled={creativeSettingsSaving} onClick={() => void saveCreativeSettings({ pageAspectRatio: option.value })}><span className="page-aspect-shape" style={{ aspectRatio: option.cssRatio }} /><strong>{option.label}</strong></button>)}</div></div>
      <details open={advancedSettingsOpen} onToggle={(event) => setAdvancedSettingsOpen((event.target as HTMLDetailsElement).open)} className="section-tools"><summary>高级设置</summary><div className="story-style-chips"><button className={`story-style-chip ${session.visualPreferences.visualComplexity === "simple" ? "active" : ""}`} type="button" onClick={() => void saveCreativeSettings({ visualComplexity: "simple" })}>简洁画面</button><button className={`story-style-chip ${session.visualPreferences.visualComplexity === "standard" ? "active" : ""}`} type="button" onClick={() => void saveCreativeSettings({ visualComplexity: "standard" })}>标准丰富度</button><button className={`story-style-chip ${session.visualPreferences.visualComplexity === "rich" ? "active" : ""}`} type="button" onClick={() => void saveCreativeSettings({ visualComplexity: "rich" })}>丰富细节</button></div><div className="story-style-chips"><button className={`story-style-chip ${session.visualPreferences.characterConsistency === "speed" ? "active" : ""}`} type="button" onClick={() => void saveCreativeSettings({ characterConsistency: "speed" })}>快速生成</button><button className={`story-style-chip ${session.visualPreferences.characterConsistency === "auto" ? "active" : ""}`} type="button" onClick={() => void saveCreativeSettings({ characterConsistency: "auto" })}>自动保持一致</button><button className={`story-style-chip ${session.visualPreferences.characterConsistency === "confirm_character" ? "active" : ""}`} type="button" onClick={() => void saveCreativeSettings({ characterConsistency: "confirm_character" })}>优先角色一致</button></div></details>
    </Modal>
  ) : null;
  const creativeSettingsSummary = session ? (
    <div className="creative-settings-summary-row">
      <span>创作设定：<strong>{storyStylePreset.label} · {visualStylePreset.label} · {PAGE_ASPECT_OPTIONS.find((item) => item.value === session.visualPreferences.pageAspectRatio)?.label || "竖版 4:5"} · {session.pageCount} 页</strong></span>
      <button className="button ghost compact" type="button" onClick={() => setCreativeSettingsOpen(true)}><SlidersHorizontal size={16} aria-hidden="true" />调整设定</button>
    </div>
  ) : null;
  const pendingVisualStyleChangeModal = pendingVisualStyleChange ? (
    <Modal title="切换绘本风格？" onClose={() => !creativeSettingsSaving && setPendingVisualStyleChange(null)}>
      <p>{pendingVisualStyleChange.affectedCount ? `当前 ${pendingVisualStyleChange.affectedCount} 张照片的参考图会失效并回到待处理。` : "当前已生成的照片参考图会失效并回到待处理。"}开始制作会被阻止，直到按新风格重新生成并确认。</p>
      <div className="modal-actions"><button className="button secondary" type="button" disabled={creativeSettingsSaving} onClick={() => setPendingVisualStyleChange(null)}>保留当前风格</button><ActionButton className="button danger" disabled={creativeSettingsSaving} disabledHint="正在保存设定" onClick={() => void saveCreativeSettings({ visualStyleId: pendingVisualStyleChange.styleId, confirmReferenceRegeneration: true }).then((saved) => { if (saved) setPendingVisualStyleChange(null); })}>确认切换并重新生成</ActionButton></div>
    </Modal>
  ) : null;

  async function applySourceRunResult(run: StorybookCustomizationRun) {
    const outputBooks = await Promise.all(
      run.items
        .filter((item) => item.outputStorybookId)
        .map((item) => getStorybook(workspace.id, item.outputStorybookId as string).catch(() => null)),
    );
    const books = outputBooks.filter((book): book is Storybook => Boolean(book));
    const bookById = new Map(books.map((book) => [book.id, book]));
    if (run.sourceStorybookId) {
      getStorybook(workspace.id, run.sourceStorybookId)
        .then((book) => {
          setSelectedSource(book);
          setSourceBooks((current) => current.some((item) => item.id === book.id) ? current : [book, ...current]);
        })
        .catch(() => undefined);
    }
    setRecipientMode(run.mode === "batch" ? "batch" : "single");
    setSourceBatchResult({
      sourceStorybookId: run.sourceStorybookId,
      runId: run.id,
      requestedCount: run.requestedCount,
      createdCount: run.succeededCount,
      storybooks: books,
      items: run.items.map((item) => ({
        childId: item.targetChildId,
        runItemId: item.id,
        status: item.status === "succeeded" ? "created" : item.status,
        storybook: item.outputStorybookId ? bookById.get(item.outputStorybookId) : undefined,
        storybookLoadFailed: Boolean(item.outputStorybookId && !bookById.has(item.outputStorybookId)),
        failureReason: item.failureReason,
      })),
    });
    setSourceStep(2);
  }

  async function abandonSourceBatchItem(runItemId: string) {
    const runId = sourceBatchResult?.runId || sourceRunId;
    if (!runId) return;
    setAbandoningRunItemId(runItemId);
    setError("");
    try {
      const run = await abandonStorybookCustomizationRunItem(workspace.id, runId, runItemId);
      await applySourceRunResult(run);
      restoredSourceRunRef.current = run.id;
    } catch (err) {
      setError(err instanceof Error ? err.message : "失败项没有放弃成功，请稍后再试。");
    } finally {
      setAbandoningRunItemId(null);
    }
  }

  async function cancelSourceRun() {
    const runId = sourceBatchResult?.runId || sourceRunId;
    if (!runId) return;
    setCancelingSourceRun(true);
    setError("");
    try {
      const run = await cancelStorybookCustomizationRun(workspace.id, runId);
      await applySourceRunResult(run);
      restoredSourceRunRef.current = run.id;
    } catch (err) {
      setError(err instanceof Error ? err.message : "本次制作没有取消成功，请稍后再试。");
    } finally {
      setCancelingSourceRun(false);
    }
  }

  async function restoreSourceRunFromServer(runId: string) {
    const run = await getStorybookCustomizationRun(workspace.id, runId);
    await applySourceRunResult(run);
    restoredSourceRunRef.current = runId;
  }

  useEffect(() => {
    const runId = sourceBatchResult?.runId || sourceRunId;
    const hasActiveItems = sourceBatchResult?.items.some((item) => ["queued", "running", "retrying"].includes(item.status));
    if (entryType !== "from_storybook" || !runId || !hasActiveItems) return;
    const timer = window.setInterval(() => {
      restoreSourceRunFromServer(runId).catch(() => setError("批量制作状态暂时无法刷新，请检查网络后刷新页面继续查看。"));
    }, 2000);
    return () => window.clearInterval(timer);
  }, [entryType, sourceBatchResult?.runId, sourceBatchResult?.items, sourceRunId]);

  async function retrySourceBatchItem(runItemId: string) {
    const runId = sourceBatchResult?.runId || sourceRunId;
    if (!runId) return;
    setRetryingRunItemId(runItemId);
    setError("");
    try {
      const run = await retryStorybookCustomizationRunItem(workspace.id, runId, runItemId);
      await applySourceRunResult(run);
      restoredSourceRunRef.current = run.id;
      if (!sourceRunId) {
        navigate(`/app/${workspace.id}/storybooks/personalized/new?sourceStorybookId=${run.sourceStorybookId}&sourceRunId=${run.id}`, { replace: true });
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "失败项没有重试成功，请稍后再试。");
    } finally {
      setRetryingRunItemId(null);
    }
  }

  async function reloadSourceOutputStorybook(runItemId: string) {
    const runId = sourceBatchResult?.runId || sourceRunId;
    if (!runId) return;
    setReloadingRunItemId(runItemId);
    setError("");
    try {
      await restoreSourceRunFromServer(runId);
    } catch (err) {
      setError(err instanceof Error ? err.message : "作品详情暂时无法读取，请稍后重试。");
    } finally {
      setReloadingRunItemId(null);
    }
  }

  function returnToSourceMaterials() {
    setSourceBatchResult(null);
    setSourceStep(0);
    if (sourceStorybookId) {
      navigate(`/app/${workspace.id}/storybooks/personalized/new?sourceStorybookId=${sourceStorybookId}`, { replace: true });
    }
  }

  useEffect(() => {
    if (!selectedSource) {
      setSourceKeepPageIds([]);
      return;
    }
    const visiblePageIds = new Set(selectedSource.pages.map((page) => page.id));
    setSourceKeepPageIds((current) => current.filter((id) => visiblePageIds.has(id)));
    if (!sourceRunId) {
      setSourceBatchResult(null);
    }
  }, [selectedSource, sourceRunId]);

  function materialLabels(ids: string[]) {
    return ids
      .map((id) => materials.find((material) => material.id === id)?.label)
      .filter((label): label is string => Boolean(label));
  }

  function selectEntry(nextEntryType: EntryType) {
    setEntryType(nextEntryType);
    setError("");
    if (nextEntryType === "from_storybook") {
      setBusy(null);
    }
  }

  function openPhotoUploadModal() {
    if (!session) {
      setError(entryType === "from_storybook" ? "照片上传入口正在准备，请稍后再试。" : "先写下故事想法并创建草稿，再上传照片。");
      return;
    }
    if (remainingPhotoSlots <= 0) {
      setError(`最多添加 ${maxPhotoFiles} 张使用中的照片；可以先从本次创作移除一张或改为不使用再继续。`);
      return;
    }
    if (!photoUploadBatch.some((item) => item.status !== "succeeded")) {
      setPhotoKind(null);
      setPhotoUploadBatch([]);
      setPhotoUploadError("");
    }
    setPhotoUploadModalOpen(true);
  }

  function openPhotoPicker() {
    if (!photoKind || photoUploadInProgress) return;
    photoInputRef.current?.click();
  }

  function closePhotoUploadModal() {
    if (photoUploadInProgress) return;
    setPhotoUploadModalOpen(false);
  }

  function clearPhotoInput() {
    if (photoInputRef.current) {
      photoInputRef.current.value = "";
    }
  }

  async function refreshCurrentSession(sessionId = session?.id) {
    if (!sessionId) return null;
    const next = await getStorybookCreationSession(workspace.id, sessionId);
    applyServerSession(next);
    getStorybookAssetUploadPolicy(workspace.id, sessionId)
      .then(setUploadPolicy)
      .catch(() => undefined);
    return next;
  }

  async function handlePhotoFiles(files: FileList | null) {
    if (!session) {
      setPhotoUploadError(entryType === "from_storybook" ? "照片上传入口正在准备，请稍后再试。" : "先写下故事想法并创建草稿，再上传照片。");
      clearPhotoInput();
      return;
    }
    if (!files?.length) return;
    const selectedPhotoKind = photoKind;
    if (!selectedPhotoKind) {
      setPhotoUploadError("请先选择这批照片的类型。");
      clearPhotoInput();
      return;
    }
    if (remainingPhotoSlots <= 0) {
      setPhotoUploadError(`最多添加 ${maxPhotoFiles} 张使用中的照片；可以先从本次创作移除一张或改为不使用再继续。`);
      clearPhotoInput();
      return;
    }
    const selectedFiles = Array.from(files);
    const supportedFiles = selectedFiles.filter((file) => acceptedPhotoTypes.includes(file.type));
    if (supportedFiles.length !== selectedFiles.length) {
      setPhotoUploadError("请选择 JPG、PNG 或 WebP 图片。");
      clearPhotoInput();
      return;
    }
    const oversizedFile = maxPhotoFileSizeBytes > 0
      ? supportedFiles.find((file) => file.size > maxPhotoFileSizeBytes)
      : undefined;
    if (oversizedFile) {
      setPhotoUploadError(`“${oversizedFile.name}”超过上传大小限制，请换一张更小的照片。`);
      clearPhotoInput();
      return;
    }
    const acceptedFiles = supportedFiles.slice(0, remainingPhotoSlots);
    const batch = acceptedFiles.map((file) => ({ file, idempotencyKey: newIdempotencyKey(), status: "pending" as const }));
    setPhotoUploadBatch(batch);
    await uploadPhotoBatch(batch, selectedPhotoKind, selectedFiles.length > acceptedFiles.length);
  }

  async function uploadPhotoBatch(batch: PhotoUploadBatchItem[], kind: PhotoKind, limitedBySlots = false) {
    if (!session || photoUploadInProgress) return;
    setPhotoUploadInProgress(true);
    setBusy("loading");
    setPhotoUploadError("");
    try {
      for (const item of batch.filter((candidate) => candidate.status !== "succeeded")) {
        try {
          await uploadStorybookCreationAsset(workspace.id, session.id, {
            file: item.file,
            kind,
            idempotencyKey: item.idempotencyKey,
          });
          setPhotoUploadBatch((current) => current.map((candidate) => candidate.idempotencyKey === item.idempotencyKey ? { ...candidate, status: "succeeded" } : candidate));
        } catch (err) {
          setPhotoUploadBatch((current) => current.map((candidate) => candidate.idempotencyKey === item.idempotencyKey ? { ...candidate, status: "failed" } : candidate));
          setPhotoUploadError(err instanceof Error ? err.message : "照片没有上传成功，请重试剩余照片。");
          await refreshCurrentSession(session.id).catch(() => undefined);
          return;
        }
      }
      await refreshCurrentSession(session.id);
      setPhotoUploadModalOpen(false);
      setPhotoUploadBatch([]);
      setPhotoKind(null);
      if (limitedBySlots) {
        setError(`本次只添加了 ${batch.length} 张照片，最多保留 ${maxPhotoFiles} 张使用中的照片。`);
      }
    } catch (err) {
      setPhotoUploadError(err instanceof Error ? `照片已上传，但素材列表刷新失败：${err.message}` : "照片已上传，但素材列表刷新失败。请重试读取结果。");
    } finally {
      setBusy(null);
      setPhotoUploadInProgress(false);
      clearPhotoInput();
    }
  }

  function retryPendingPhotoUploads() {
    if (!photoKind || photoUploadInProgress) return;
    void uploadPhotoBatch(photoUploadBatch.filter((item) => item.status !== "succeeded"), photoKind);
  }

  async function updatePhotoUsage(photoId: string, usage: string) {
    if (!session || busy) return;
    const photo = photoMaterials.find((item) => item.id === photoId);
    if (!photo) return;
    const displayName = photo.displayName.trim();
    if (usage !== "不使用" && !displayName) {
      setError(`先填写“${photoNamePrompt(photo.kind)}”，再确认用途。`);
      return;
    }
    setBusy("loading");
    setError("");
    try {
      const usageCode = usageCodeForLabel(photo.kind, usage);
      await updateStorybookAssetReference(workspace.id, session.id, photoId, {
        kind: photo.kind,
        displayName,
        usage: usageCode,
      });
      if (!["unused", "name_only"].includes(usageCode)) {
        await generateStorybookVisualReference(workspace.id, session.id, photoId, newIdempotencyKey());
      }
      await refreshCurrentSession(session.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : "照片用途没有保存成功。");
    } finally {
      setBusy(null);
    }
  }

  function updatePhotoDisplayName(photoId: string, displayName: string) {
    setPhotoMaterials((current) => current.map((photo) => photo.id === photoId ? { ...photo, displayName } : photo));
  }

  async function markVisualReferenceReady(photoId: string) {
    if (!session || busy) return;
    setBusy("loading");
    setError("");
    try {
      await generateStorybookVisualReference(workspace.id, session.id, photoId, newIdempotencyKey());
      await refreshCurrentSession(session.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : "同画风参考没有开始生成。");
    } finally {
      setBusy(null);
    }
  }

  async function confirmVisualReference(photoId: string) {
    if (!session || busy) return;
    const photo = photoMaterials.find((item) => item.id === photoId);
    if (!photo?.visualReferenceId) {
      setError("参考图还没有生成完成，请稍后刷新。");
      return;
    }
    setBusy("loading");
    setError("");
    try {
      await confirmStorybookVisualReference(workspace.id, session.id, photo.visualReferenceId);
      await refreshCurrentSession(session.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : "同画风参考没有确认成功。");
    } finally {
      setBusy(null);
    }
  }

  async function removePhotoMaterial(photoId: string) {
    if (!session || busy) return;
    const photo = photoMaterials.find((item) => item.id === photoId);
    setBusy("loading");
    setPhotoRemovalError("");
    setError("");
    try {
      await revokeStorybookAssetReference(workspace.id, session.id, photoId);
      setPhotoMaterials((current) => current.filter((item) => item.id !== photoId));
      setSession((current) => current ? { ...current, assetReferences: current.assetReferences.filter((reference) => reference.id !== photoId) } : current);
      setAssetRecoveryNotice({ name: photo?.displayName.trim() || photo?.name || "这张照片" });
      setPhotoPendingRemoval(null);
      await refreshCurrentSession(session.id).catch(() => {
        setError("照片已移除，但素材列表暂时无法刷新；刷新页面后可确认最新结果。");
      });
    } catch (err) {
      setPhotoRemovalError(err instanceof Error ? err.message : "照片没有移除成功。");
    } finally {
      setBusy(null);
    }
  }

  function requestPhotoRemoval(photo: PhotoMaterial) {
    setPhotoRemovalError("");
    setPhotoPendingRemoval(photo);
  }

  function photoRemoveTrigger(photo: PhotoMaterial) {
    return (
      <button className="icon-button photo-remove-trigger" type="button" disabled={busy !== null} aria-label={`从本次创作移除${photo.displayName.trim() || photo.name}`} title="从本次创作移除" onClick={() => requestPhotoRemoval(photo)}>
        <Trash2 size={17} aria-hidden="true" />
      </button>
    );
  }

  async function recoverAfterAssetRevocation(action: "repreview" | "continue_without_photo" | "cancel") {
    if (action === "cancel") {
      setAssetRecoveryNotice(null);
      navigate(`/app/${workspace.id}/storybooks`);
      return;
    }
    if (action === "continue_without_photo") {
      setAssetRecoveryNotice(null);
      return;
    }
    setAssetRecoveryNotice(null);
    if (entryType === "from_storybook") {
      if (!selectedSource || busy) return;
      await confirmSourceMaterials();
      return;
    }
    if (session && !busy) {
      await refreshDirections();
    }
  }

  async function startFromIdea() {
    const value = idea.trim();
    if (value.length < 8) {
      setError("再补充一点人物、物品或目的，故事会更贴近你。");
      return;
    }
    setBusy("directions");
    setError("");
    try {
      const created = await createStorybookCreationSession(workspace.id, { quickIdea: value, pageCount: 10 });
      const materialResponse = recipientName.trim()
        ? await patchStorybookCreationMaterials(workspace.id, created.id, [{
          op: "add",
          label: recipientName.trim(),
          type: "character",
          locked: true,
        }])
        : { materials: created.materials };
      const restored = await getStorybookCreationSession(workspace.id, created.id);
      setSession({ ...restored, materials: materialResponse.materials, directions: [], status: "understanding_ready", requiresDirectionRefresh: true });
      setLatestDraft(null);
      setEditingMaterials(true);
      setCreationWarnings([]);
    } catch (err) {
      setError(err instanceof Error ? err.message : "故事方向没有生成成功，请重试。");
    } finally {
      setBusy(null);
    }
  }

  async function resumeDraft() {
    if (!latestDraft) return;
    setBusy("loading");
    try {
      const restored = await getStorybookCreationSession(workspace.id, latestDraft.id);
      if (restored.status === "storybook_ready" && restored.storybookId) {
        setLatestDraft(null);
        navigate(`/app/${workspace.id}/storybooks/${restored.storybookId}/review?result=personalized`);
        return;
      }
      setSession(restored);
      setIdea(restored.quickIdea);
      setRecipientName(restored.materials.find((item) => item.locked && item.type === "character")?.label || "");
      setSelectedDirection(restored.directions.find((item) => item.id === restored.selectedDirectionId) || null);
      setCreationWarnings([]);
      setLatestDraft(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "无法恢复上次内容。");
    } finally {
      setBusy(null);
    }
  }

  async function abandonDraft() {
    if (!latestDraft) return;
    setBusy("loading");
    try {
      await abandonStorybookCreationSession(workspace.id, latestDraft.id);
      setLatestDraft(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "无法放弃上次内容。");
    } finally {
      setBusy(null);
    }
  }

  async function cancelDirectCreationRun() {
    if (!session || busy) return;
    setBusy("loading");
    setError("");
    try {
      await abandonStorybookCreationSession(workspace.id, session.id);
      setCancelDirectCreationConfirmOpen(false);
      setSession((current) => current ? { ...current, status: "abandoned" } : current);
      navigate(`/app/${workspace.id}/storybooks`, { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : "本次制作没有取消成功，请刷新后查看结果。");
    } finally {
      setBusy(null);
    }
  }

  async function addMaterial() {
    if (!session || !newMaterial.trim()) return;
    if (lockedMaterialCount >= 3) {
      setError("最多保留 3 个专属素材；可以先移除一个再添加。");
      return;
    }
    setBusy("loading");
    try {
      const response = await patchStorybookCreationMaterials(workspace.id, session.id, [{ op: "add", label: newMaterial.trim(), type: "custom", locked: true }]);
      setSession({ ...session, materials: response.materials, directions: [], outline: undefined, status: response.status });
      setSelectedDirection(null);
      setEditingMaterials(true);
      setNewMaterial("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "没有添加成功。");
    } finally {
      setBusy(null);
    }
  }

  async function removeMaterial(material: CreationMaterial) {
    if (!session) return;
    setBusy("loading");
    setMaterialRemovalError("");
    try {
      const response = await patchStorybookCreationMaterials(workspace.id, session.id, [{ op: "remove", id: material.id }]);
      setSession({ ...session, materials: response.materials, directions: [], outline: undefined, status: response.status });
      setSelectedDirection(null);
      setEditingMaterials(true);
      setMaterialPendingRemoval(null);
    } catch (err) {
      setMaterialRemovalError(err instanceof Error ? err.message : "没有移除成功。");
    } finally {
      setBusy(null);
    }
  }

  async function chooseDirection(direction: StoryDirection) {
    if (!session || busy) return;
    setBusy("loading");
    setError("");
    try {
      await selectStorybookCreationDirection(workspace.id, session.id, direction.id);
      setSelectedDirection(direction);
      setSession((current) => current ? { ...current, selectedDirectionId: direction.id } : current);
    } catch (err) {
      setError(err instanceof Error ? err.message : "故事走向没有选中，可以重试。");
    } finally {
      setBusy(null);
    }
  }

  async function generateOutline() {
    if (!session || !selectedDirection || busy) return;
    setBusy("outline");
    setError("");
    try {
      const nextOutline = await generateStorybookCreationOutline(workspace.id, session.id, session.pageCount);
      setCreationWarnings(nextOutline.warnings || []);
      setSession((current) => current ? { ...current, outline: nextOutline, status: "outline_ready", requiresOutlineRefresh: false } : current);
      setDirectViewStep(1);
    } catch (err) {
      setError(err instanceof Error ? err.message : "故事大纲还没有整理完成，可以重试。");
    } finally {
      setBusy(null);
    }
  }

  async function refreshDirections() {
    if (!session || busy) return;
    setBusy("directions");
    setError("");
    try {
      const next = await generateStorybookDirections(
        workspace.id,
        session.id,
        session.directions.length > 0 ? "user_clicked_refresh" : "initial",
      );
      setCreationWarnings(next.warnings || []);
      setSession((current) => current ? {
        ...current,
        directions: next,
        selectedDirectionId: undefined,
        outline: undefined,
        status: "directions_ready",
      } : current);
      setSelectedDirection(null);
      setEditingMaterials(false);
      setDirectViewStep(1);
      setCreativeSettingsNotice("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "没有生成新的故事方向。");
    } finally {
      setBusy(null);
    }
  }

  async function saveIdeaChanges() {
    if (!session) return;
    const value = idea.trim();
    if (value.length < 8) {
      setError("再补充一点人物、物品或目的，故事会更贴近你。");
      return;
    }
    setBusy("loading");
    setError("");
    try {
      await updateStorybookCreationSession(workspace.id, session.id, { quickIdea: value });
      const refreshed = await refreshStorybookCreationUnderstanding(workspace.id, session.id);
      setSession(refreshed);
      setSelectedDirection(null);
      setEditingIdea(false);
      setEditingMaterials(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : "想法没有更新成功，请重试。");
    } finally {
      setBusy(null);
    }
  }

  async function updateOutlinePage(pageNumber: number) {
    const instruction = pageInstruction[pageNumber]?.trim();
    if (!session || !instruction) return;
    setBusy(`page-${pageNumber}`);
    try {
      const page = await updateStorybookCreationOutlinePage(workspace.id, session.id, pageNumber, instruction);
      setSession((current) => current?.outline
        ? { ...current, outline: { ...current.outline, pages: current.outline.pages.map((item) => item.pageNumber === pageNumber ? page : item) } }
        : current);
      setPageInstruction((current) => ({ ...current, [pageNumber]: "" }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "这一页没有更新成功，可以重试。");
    } finally {
      setBusy(null);
    }
  }

  async function generateBook() {
    if (!session) return;
    setBusy("generating");
    setError("");
    try {
      const result = await generateStorybookFromCreationSession(workspace.id, session.id, newIdempotencyKey());
      const next = await getStorybookCreationSession(workspace.id, session.id);
      setSession(next);
      if (next.status === "generating") setDirectViewStep(2);
      if (result.storybookId && next.status === "storybook_ready") {
        setDirectViewStep(3);
        navigate(`/app/${workspace.id}/storybooks/${result.storybookId}/review?result=personalized`, { replace: true });
      }
    } catch (err) {
      try {
        const recovered = await refreshCurrentSession(session.id);
        if (recovered?.storybookId && recovered.status === "storybook_ready") {
          navigate(`/app/${workspace.id}/storybooks/${recovered.storybookId}/review?result=personalized`, { replace: true });
          return;
        }
        if (recovered?.status === "generating") {
          setError("");
          return;
        }
      } catch {
        // Keep the original request failure when the recovery read also fails.
      }
      setError(err instanceof Error ? err.message : "绘本没有开始制作，可以重试。");
    } finally {
      setBusy(null);
    }
  }

  async function requestSourceCustomizationPlan(nextKeepPageIds = sourceKeepPageIds) {
    if (!selectedSource) return null;
    const mode = recipientMode;
    const plan = preserveSourceReferencePlacements(sourcePlanFromApi(await buildStorybookCustomizationPlan(workspace.id, selectedSource.id, {
      mode,
      targetChildId: mode === "single" ? selectedChild?.id : undefined,
      targetChildIds: mode === "batch" ? selectedBatchIds : [],
      primaryMaterial: mode === "single" ? singleMaterialChoice : undefined,
      optionalKeepPageIds: nextKeepPageIds,
      confirmedPhotoReferenceIds: confirmedPhotoMaterials.map((photo) => photo.id),
    })), sourcePlan);
    setSourcePlan(plan);
    setSourceKeepPageIds(plan.optional_keep_page_ids || nextKeepPageIds);
    return plan;
  }

  async function confirmSourceMaterials() {
    setBusy("source-generating");
    setError("");
    try {
      const plan = await requestSourceCustomizationPlan();
      if (!plan) return;
      setSourcePreviewReady(true);
      setSourceStep(1);
      setCreativeSettingsNotice("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "变化计划没有生成成功，可以重试。");
    } finally {
      setBusy(null);
    }
  }

  async function toggleSourceKeepPage(pageId: string, checked: boolean) {
    const nextKeepPageIds = checked
      ? [...new Set([...sourceKeepPageIds, pageId])]
      : sourceKeepPageIds.filter((id) => id !== pageId);
    setSourceKeepPageIds(nextKeepPageIds);
    setBusy("source-generating");
    setError("");
    try {
      await requestSourceCustomizationPlan(nextKeepPageIds);
    } catch (err) {
      setError(err instanceof Error ? err.message : "这一页的保持偏好没有更新成功，可以重试。");
    } finally {
      setBusy(null);
    }
  }

  function toggleSourceReferencePage(reference: SourcePhotoReference, page: SourceCustomizationPagePlan, checked: boolean) {
    const referenceId = reference.asset_reference_id;
    const field = reference.reference_type === "character_reference"
      ? "character_reference_ids"
      : reference.reference_type === "scene_reference"
        ? "scene_reference_ids"
        : "prop_reference_ids";
    if (!referenceId || !page.page_number) return;
    setSourcePlan((current) => {
      if (!current?.page_plan) return current;
      const pagePlan = current.page_plan.map((item) => {
        const ids = (item[field] || []).filter((id) => id !== referenceId);
        if (item.page_number === page.page_number && checked) ids.push(referenceId);
        return { ...item, [field]: ids };
      });
      const plannedPages = pagePlan
        .filter((item) => (item[field] || []).includes(referenceId))
        .map((item) => ({ page_number: item.page_number, title: item.title }));
      return {
        ...current,
        page_plan: pagePlan,
        confirmed_photo_references: current.confirmed_photo_references?.map((item) => item.asset_reference_id === referenceId
          ? { ...item, placement_scope: "page", planned_pages: plannedPages, unplaced_reason: plannedPages.length ? null : "page_selection_required" }
          : item),
      };
    });
  }

  async function generateFromSourceStorybook() {
    if (!selectedSource) return;
    if (recipientMode === "single" && (!selectedChild || !singleMaterialChoice)) return;
    if (recipientMode === "batch" && !batchSelectionsReady) return;
    if (awaitingPhotoReferences.length > 0) {
      setError("先处理照片用途和同画风参考，再开始制作定制绘本。");
      return;
    }
    if ((sourcePlan?.confirmed_photo_references || []).some((reference) => !reference.planned_pages?.length)) {
      setError("请先为每张已确认参考选择实际使用页面。");
      return;
    }
    setBusy("source-generating");
    setError("");
    setSourceBatchResult(null);
    try {
      const plan = sourcePlan || await requestSourceCustomizationPlan();
      if (!plan) {
        setError("先生成并确认变化计划，再开始制作定制绘本。");
        return;
      }
      if (recipientMode === "batch") {
        const result = await deriveCustomStorybooksBatch(workspace.id, selectedSource.id, {
          childIds: selectedBatchIds,
          intensity: "quick",
          materialChoices: batchMaterialChoices,
          customizationPlan: plan,
          creationSessionId: session?.id,
        });
        setSourceBatchResult(result);
        setSourceStep(2);
        if (result.runId) {
          restoredSourceRunRef.current = result.runId;
          navigate(`/app/${workspace.id}/storybooks/personalized/new?sourceStorybookId=${selectedSource.id}&sourceRunId=${result.runId}`, { replace: true });
        }
        return;
      }
      const childForSingle = selectedChild;
      if (!childForSingle) return;
      const result = await deriveCustomStorybooksBatch(workspace.id, selectedSource.id, {
        childIds: [childForSingle.id],
        intensity: "standard",
        materialChoices: { [childForSingle.id]: singleMaterialChoice },
        customizationPlan: plan,
        creationSessionId: session?.id,
      });
      setSourceBatchResult(result);
      setSourceStep(2);
      if (result.runId) {
        restoredSourceRunRef.current = result.runId;
        navigate(`/app/${workspace.id}/storybooks/personalized/new?sourceStorybookId=${selectedSource.id}&sourceRunId=${result.runId}`, { replace: true });
      }
    } catch (err) {
      if (isApiClientError(err) && err.code === "source_revision_conflict") {
        setSourcePreviewReady(false);
        setSourceStep(0);
        setSourceKeepPageIds([]);
        setSourcePlan(null);
        try {
          const refreshed = await getStorybook(workspace.id, selectedSource.id);
          setSelectedSource(refreshed);
          setSourceBooks((current) => current.map((book) => book.id === refreshed.id ? refreshed : book));
        } catch {
          setSourceLoadFailed(true);
        }
        setError("来源绘本已有更新，请重新确认对象、素材和变化范围后再制作。");
        return;
      }
      setError(err instanceof Error ? err.message : "定制绘本没有开始制作，可以重试。");
    } finally {
      setBusy(null);
    }
  }

  const unresolvedPhotoMaterials = photoMaterials.filter((photo) => photo.referenceStatus !== "ready" && photo.referenceStatus !== "unused");
  const confirmedPhotoMaterials = photoMaterials.filter((photo) => photo.referenceStatus === "ready");
  const ignoredPhotoMaterials = photoMaterials.filter((photo) => photo.referenceStatus === "unused");
  const nextPhotoIssue = unresolvedPhotoMaterials[0];
  const sourceAssetSessionPreparing = entryType === "from_storybook" && !session;
  const photoUploadUnavailableReason = !session
    ? entryType === "from_storybook"
      ? "照片上传入口正在准备；准备好后可以添加人物、物品或场景照片。"
      : "先写下故事想法并创建草稿，再上传照片。"
    : "";
  const canUploadPhotos = !photoUploadUnavailableReason;
  const photoMaterialSection = (
    <>
    <Card className="photo-material-card">
      <div className="section-head">
        <div>
          <p className="eyebrow">照片素材</p>
          <h2>把真实照片转成绘本参考</h2>
          <p>照片只用于本次创作，会先转成同画风视觉参考，再进入故事计划。</p>
        </div>
        <Badge tone={activePhotoCount >= maxPhotoFiles ? "warn" : "info"}>{activePhotoCount}/{maxPhotoFiles}</Badge>
      </div>
      {session && <p className="form-helper">当前参考图风格：<strong>{visualStylePreset.label}</strong></p>}
      {photoMaterials.length > 0 && (
        <div className={unresolvedPhotoMaterials.length ? "photo-summary-bar needs-work" : "photo-summary-bar ready"}>
          <strong>{unresolvedPhotoMaterials.length ? `还差 ${unresolvedPhotoMaterials.length} 张照片需要处理` : "照片素材已准备好"}</strong>
          <span>{unresolvedPhotoMaterials.length ? "可以先继续整理故事；开始制作前需要处理完用途和同画风参考。" : "已确认的参考会进入本次创作计划。"}</span>
        </div>
      )}
      {photoUploadUnavailableReason && (
        <Notice
          title={entryType === "from_storybook" ? "正在准备照片上传" : "先创建故事草稿"}
          copy={photoUploadUnavailableReason}
          tone="info"
        />
      )}
      {assetRecoveryNotice && (
        <Notice
          title="照片已从本次创作移除"
          copy={`${assetRecoveryNotice.name} 不会继续进入后续页面或新的生成输入；已经完成的外部绘制不能回收。请选择下一步恢复动作。`}
          tone="warn"
          action={(
            <span className="inline-actions">
              <button className="button secondary compact" type="button" disabled={busy !== null} onClick={() => void recoverAfterAssetRevocation("repreview")}>重新预览</button>
              <button className="button secondary compact" type="button" onClick={() => void recoverAfterAssetRevocation("continue_without_photo")}>不用这张照片继续</button>
              <button className="button danger compact" type="button" onClick={() => void recoverAfterAssetRevocation("cancel")}>取消本次制作</button>
            </span>
          )}
        />
      )}
      <div className="photo-material-toolbar">
        <input
          ref={photoInputRef}
          className="visually-hidden"
          type="file"
          accept={photoAcceptAttr}
          multiple
          disabled={!canUploadPhotos || remainingPhotoSlots <= 0 || photoUploadInProgress}
          onChange={(event) => handlePhotoFiles(event.target.files)}
        />
        <ActionButton
          className="button secondary"
          disabled={!canUploadPhotos || remainingPhotoSlots <= 0 || photoUploadInProgress}
          disabledHint={photoUploadInProgress ? "正在上传照片" : photoUploadUnavailableReason || `最多添加 ${maxPhotoFiles} 张使用中的照片；可以先从本次创作移除一张或改为不使用`}
          onClick={openPhotoUploadModal}
        >
          {remainingPhotoSlots <= 0 ? "管理照片" : "添加照片"}
        </ActionButton>
      </div>
      {remainingPhotoSlots <= 0 && <Notice title="照片已达到上限" copy={`本次创作最多添加 ${maxPhotoFiles} 张使用中的真实照片；可以先从本次创作移除一张或改为不使用再继续。`} tone="warn" />}
      {!photoMaterials.length ? (
        <EmptyState
          title={sourceAssetSessionPreparing ? "照片上传入口准备中" : entryType === "from_storybook" ? "可以补充照片素材" : "可以不上传照片"}
          copy={sourceAssetSessionPreparing
            ? "准备好后可以上传人物、玩具、物品、宠物或场景照片。"
            : entryType === "from_storybook"
              ? "上传人物、玩具、物品、宠物或场景照片后，会先生成同画风参考，再进入变化计划和制作输入。"
            : "没有照片也能完成专属绘本。需要更强专属感时，再添加人物、玩具、物品、宠物或场景照片。"}
        />
      ) : (
        <div className="photo-material-groups">
          {unresolvedPhotoMaterials.length > 0 && (
            <section className="photo-material-group">
              <div className="photo-group-title"><strong>待处理</strong><Badge tone="warn">{unresolvedPhotoMaterials.length}</Badge></div>
              <div className="photo-material-list">
                {unresolvedPhotoMaterials.map((photo) => (
                  <div className="photo-material-row needs-work" key={photo.id}>
                    {photo.previewUrl ? <ProtectedPhotoThumbnail src={photo.previewUrl} alt={photo.displayName.trim() || photo.fileName || photo.name} kind={photo.kind} onPreview={setZoomedImage} /> : <div className="photo-thumb placeholder-thumb" aria-hidden="true">{photoKindLabel(photo.kind).slice(0, 1)}</div>}
                    <div className="photo-material-body">
                      <div className="photo-row-head">
                        <div>
                          <strong>{photo.displayName.trim() || photo.name}</strong>
                          <p>{photo.usage ? `用途：${photo.usage} · ${referenceStatusLabel(photo.referenceStatus, photo.kind)}` : `${photo.fileName} · ${referenceStatusLabel(photo.referenceStatus, photo.kind)}`}</p>
                        </div>
                        {photoRemoveTrigger(photo)}
                      </div>
                      {(photo.referenceStatus === "awaiting_usage" || photo.referenceStatus === "failed") && (
                        <label className="photo-name-field">
                          <span>{photoNamePrompt(photo.kind)}</span>
                          <input
                            value={photo.displayName}
                            onChange={(event) => updatePhotoDisplayName(photo.id, event.target.value)}
                            placeholder={photoNamePlaceholder(photo.kind)}
                          />
                        </label>
                      )}
                      {photo.referenceStatus === "awaiting_usage" && (
                        <div className="inline-actions photo-usage-actions">
                          {photoUsageOptions(photo.kind).map((option) => (
                            <button
                              key={option}
                              className={photo.usage === option ? "chip-button active" : "chip-button"}
                              type="button"
                              onClick={() => updatePhotoUsage(photo.id, option)}
                            >
                              {option}
                            </button>
                          ))}
                        </div>
                      )}
                      {photo.referenceStatus === "generating" && (
                        <div className="visual-reference-inline compact">
                          <SkeletonBlock className="visual-reference-preview" />
                          <div>
                            <strong>正在生成{fallbackReferenceName(photo)}</strong>
                            <p>生成完成后再确认是否用于本次创作。</p>
                          </div>
                        </div>
                      )}
                      {photo.referenceStatus === "failed" && (
                        <div className="visual-reference-inline compact">
                          <div className="visual-reference-preview warning-preview" aria-hidden="true">失败</div>
                          <div>
                            <strong>{fallbackReferenceName(photo)}生成失败</strong>
                            <p>{photo.failureReason || "可以重试生成，或把这张照片改为不使用。"}</p>
                            <div className="inline-actions">
                              <button className="button primary compact" type="button" onClick={() => markVisualReferenceReady(photo.id)}>重试生成</button>
                              <button className="button secondary compact" type="button" onClick={() => updatePhotoUsage(photo.id, "不使用")}>不使用</button>
                            </div>
                          </div>
                        </div>
                      )}
                      {photo.referenceStatus === "awaiting_reference" && (
                        <div className="visual-reference-inline compact">
                          <div className="visual-reference-preview ready-preview" aria-hidden="true">参考</div>
                          <div>
                            <strong>还没有生成{fallbackReferenceName(photo)}</strong>
                            <p>可以重新发起生成，或把这张照片改为不使用。</p>
                            <div className="inline-actions">
                              <button className="button primary compact" type="button" onClick={() => markVisualReferenceReady(photo.id)}>生成{referenceTypeLabel(photo.kind)}</button>
                              <button className="button secondary compact" type="button" onClick={() => updatePhotoUsage(photo.id, "不使用")}>不使用</button>
                            </div>
                          </div>
                        </div>
                      )}
                      {photo.referenceStatus === "awaiting_confirmation" && (
                        <div className="visual-reference-inline">
                          {photo.visualReferencePreviewUrl
                            ? <ProtectedVisualReferencePreview src={photo.visualReferencePreviewUrl} alt={fallbackReferenceName(photo)} onPreview={setZoomedImage} />
                            : <div className="visual-reference-preview ready-preview" aria-hidden="true">参考</div>}
                          <div>
                            <strong>{fallbackReferenceName(photo)}待确认</strong>
                            <p>确认后，{photo.displayName.trim() || "这张照片"}才会进入故事计划。{photo.kind === "person" ? "人物照片背景不会作为故事场景使用。" : ""}</p>
                            <div className="inline-actions">
                              <button className="button primary compact" type="button" onClick={() => confirmVisualReference(photo.id)}>确认{referenceTypeLabel(photo.kind)}</button>
                              <button className="button secondary compact" type="button" onClick={() => markVisualReferenceReady(photo.id)}>重新生成{referenceTypeLabel(photo.kind)}</button>
                              <button className="button secondary compact" type="button" onClick={() => updatePhotoUsage(photo.id, "不使用")}>不使用</button>
                            </div>
                          </div>
                        </div>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </section>
          )}
          {confirmedPhotoMaterials.length > 0 && (
            <section className="photo-material-group">
              <div className="photo-group-title"><strong>已确认</strong><Badge tone="good">{confirmedPhotoMaterials.length}</Badge></div>
              <div className="photo-material-list compact-list">
                {confirmedPhotoMaterials.map((photo) => (
                  <div className="photo-material-row compact-row" key={photo.id}>
                    {photo.visualReferencePreviewUrl
                      ? <ProtectedVisualReferencePreview src={photo.visualReferencePreviewUrl} alt={fallbackReferenceName(photo)} onPreview={setZoomedImage} />
                      : <div className="visual-reference-preview warning-preview" aria-label="已确认参考图加载失败">!</div>}
                    <div className="photo-material-body">
                      <div className="photo-row-head compact-photo-row-head">
                        <div><strong>{photo.displayName.trim() || photo.name}</strong><p>{photo.usage || photoKindLabel(photo.kind)} · 已确认</p></div>
                      </div>
                    </div>
                    {photoRemoveTrigger(photo)}
                  </div>
                ))}
              </div>
            </section>
          )}
          {ignoredPhotoMaterials.length > 0 && (
            <section className="photo-material-group">
              <div className="photo-group-title"><strong>本次不使用</strong><Badge tone="neutral">{ignoredPhotoMaterials.length}</Badge></div>
              <div className="photo-material-list compact-list">
                {ignoredPhotoMaterials.map((photo) => (
                  <div className="photo-material-row compact-row" key={photo.id}>
                    {photo.previewUrl ? <ProtectedPhotoThumbnail src={photo.previewUrl} alt={photo.displayName.trim() || photo.fileName || photo.name} kind={photo.kind} muted onPreview={setZoomedImage} /> : <div className="photo-thumb muted-thumb placeholder-thumb" aria-hidden="true">{photoKindLabel(photo.kind).slice(0, 1)}</div>}
                    <div className="photo-material-body">
                      <div className="photo-row-head compact-photo-row-head">
                        <div><strong>{photo.displayName.trim() || photo.name}</strong><p>不会进入本次创作计划</p></div>
                      </div>
                    </div>
                    {photoRemoveTrigger(photo)}
                  </div>
                ))}
              </div>
            </section>
          )}
          {nextPhotoIssue && <p className="form-helper">下一步需要处理：{nextPhotoIssue.displayName.trim() || nextPhotoIssue.name} · {referenceStatusLabel(nextPhotoIssue.referenceStatus, nextPhotoIssue.kind)}</p>}
        </div>
      )}
    </Card>
    {photoUploadModalOpen && (
      <Modal title="添加照片" className="photo-upload-modal" onClose={closePhotoUploadModal}>
        <p className="form-helper">先选择这批照片的类型，再选择照片上传。每次上传的照片会按同一种类型生成绘本参考。</p>
        {photoUploadError && <Notice title="照片上传未完成" copy={photoUploadError} tone="danger" />}
        {photoUploadBatch.length > 0 && (
          <p className="form-helper photo-upload-progress" role="status">
            {photoUploadInProgress ? `正在上传：已完成 ${photoUploadSucceededCount}/${photoUploadBatch.length} 张。` : photoUploadPendingCount ? `本批已完成 ${photoUploadSucceededCount}/${photoUploadBatch.length} 张，还可继续上传 ${photoUploadPendingCount} 张。` : `本批 ${photoUploadBatch.length} 张照片已上传，正在读取素材列表。`}
          </p>
        )}
        <div className="photo-kind-option-grid" aria-label="这批照片的类型">
          <label className={photoKind === "person" ? "photo-kind-option selected" : "photo-kind-option"}>
            <input className="visually-hidden" type="radio" name="photo-upload-kind" value="person" checked={photoKind === "person"} disabled={photoUploadInProgress || photoUploadBatch.length > 0} onChange={() => setPhotoKind("person")} />
            <strong>人物</strong>
            <span>生成正面站立、完整四肢的角色参考图；不会使用照片背景。</span>
          </label>
          <label className={photoKind === "object" ? "photo-kind-option selected" : "photo-kind-option"}>
            <input className="visually-hidden" type="radio" name="photo-upload-kind" value="object" checked={photoKind === "object"} disabled={photoUploadInProgress || photoUploadBatch.length > 0} onChange={() => setPhotoKind("object")} />
            <strong>玩具、物品或宠物</strong>
            <span>生成单个主体的道具参考图，保留关键外观特征。</span>
          </label>
          <label className={photoKind === "scene" ? "photo-kind-option selected" : "photo-kind-option"}>
            <input className="visually-hidden" type="radio" name="photo-upload-kind" value="scene" checked={photoKind === "scene"} disabled={photoUploadInProgress || photoUploadBatch.length > 0} onChange={() => setPhotoKind("scene")} />
            <strong>场景</strong>
            <span>生成可用于故事画面的环境参考图。</span>
          </label>
        </div>
        <div className="modal-actions">
          <button className="button secondary" type="button" disabled={photoUploadInProgress} onClick={closePhotoUploadModal}>取消</button>
          {photoUploadBatch.length > 0 && !photoUploadInProgress && <button className="button secondary" type="button" onClick={() => { setPhotoUploadBatch([]); setPhotoUploadError(""); }}>重新选择照片</button>}
          {photoUploadBatch.length > 0
            ? <ActionButton className="button primary" disabled={photoUploadInProgress} disabledHint="正在上传照片" onClick={retryPendingPhotoUploads}>{photoUploadPendingCount ? `继续上传剩余 ${photoUploadPendingCount} 张` : "重新读取结果"}</ActionButton>
            : <ActionButton className="button primary" disabled={!photoKind || photoUploadInProgress} disabledHint={photoUploadInProgress ? "正在上传照片" : "先选择照片类型"} onClick={openPhotoPicker}>{photoUploadInProgress ? "正在上传..." : "选择照片"}</ActionButton>}
        </div>
      </Modal>
    )}
    {photoPendingRemoval && (
      <Modal title="从本次创作移除照片？" onClose={() => { if (busy === null) { setPhotoRemovalError(""); setPhotoPendingRemoval(null); } }}>
        <p>“{photoPendingRemoval.displayName.trim() || photoPendingRemoval.name}”将不再进入后续故事计划或新的生成输入。</p>
        {photoPendingRemoval.referenceStatus === "ready" && <p className="form-helper">已完成的外部绘制不会回收；移除后可在下一步选择重新预览或继续制作。</p>}
        {photoRemovalError && <p className="form-helper warn" role="alert">{photoRemovalError}</p>}
        <div className="modal-actions">
          <button className="button secondary" type="button" disabled={busy !== null} onClick={() => { setPhotoRemovalError(""); setPhotoPendingRemoval(null); }}>保留照片</button>
          <ActionButton className="button danger" disabled={busy !== null} disabledHint="正在移除照片" onClick={() => void removePhotoMaterial(photoPendingRemoval.id)}>确认移除</ActionButton>
        </div>
      </Modal>
    )}
    {zoomedImage && <ImageLightbox src={zoomedImage.src} alt={zoomedImage.alt} onClose={() => setZoomedImage(null)} />}
    </>
  );

  if (!entryType) {
    return (
      <div className="page-stack personalized-flow">
        <PageHeader
          eyebrow="专属绘本创作"
          title="创建专属绘本"
          copy="选择一个故事起点。你可以从一句想法开始，也可以基于已有普通绘本创作专属版本。"
          actions={<Link className="button secondary" to={`/app/${workspace.id}/storybooks`}>查看已有绘本</Link>}
        />
        {latestDraft && (
          <Notice
            title={latestReadyStorybookId ? "上次专属绘本已完成" : "发现上次创作"}
            copy={latestReadyStorybookId ? `“${latestDraft.quickIdea.slice(0, 30)}${latestDraft.quickIdea.length > 30 ? "..." : ""}”已生成，可以直接进入绘本检查。` : `“${latestDraft.quickIdea.slice(0, 30)}${latestDraft.quickIdea.length > 30 ? "..." : ""}”仍可继续。`}
            tone="info"
            action={latestReadyStorybookId ? <span className="inline-actions"><button className="button secondary" type="button" onClick={() => { setLatestDraft(null); setEntryType("direct_create"); }}>新建专属绘本</button><Link className="button primary" to={`/app/${workspace.id}/storybooks/${latestReadyStorybookId}/review?result=personalized`}>查看已完成绘本</Link></span> : <span className="inline-actions"><button className="button secondary" type="button" onClick={abandonDraft}>放弃并新建</button><button className="button primary" type="button" onClick={() => { setEntryType("direct_create"); resumeDraft(); }}>继续上次创作</button></span>}
          />
        )}
        <div className="entry-picker-grid">
          <button className="entry-picker-card" type="button" onClick={() => selectEntry("direct_create")}>
            <Badge tone="good">从想法开始</Badge>
            <strong>我有一个想做给孩子的故事</strong>
            <span>填写一句想法、对象称呼和想留下的元素，系统会生成 3 个方向和默认 10 页走向。</span>
          </button>
          <button className="entry-picker-card" type="button" onClick={() => selectEntry("from_storybook")}>
            <Badge tone="info">基于已有绘本</Badge>
            <strong>我想把普通绘本做成专属版本</strong>
            <span>选择一本普通绘本，确认对象和素材，再查看保持、变化和重绘范围。</span>
          </button>
        </div>
      </div>
    );
  }

  if (entryType === "from_storybook") {
    const canConfirmMaterials = recipientMode === "batch"
      ? Boolean(selectedSource && batchSelectionsReady && !sourceBlocker)
      : Boolean(singleSelectionReady && !sourceBlocker);
    const canPreview = canConfirmMaterials;

    return (
      <div className="page-stack personalized-flow">
        <PageHeader
          eyebrow="专属绘本创作"
          title={sourceStep === 0 ? "对象与素材" : sourceStep === 1 ? "确认变化计划" : "正在制作专属绘本"}
          copy={sourceStep === 0 ? "选择对象和要保留的照片素材。来源绘本的主线与节奏会保持不变。" : sourceStep === 1 ? "确认哪些页面保持、变化或重绘，再开始制作。" : "正在根据确认的变化计划制作专属版本。"}
          actions={!sourceStorybookId || sourceLoadFailed ? <button className="button secondary" type="button" onClick={() => setEntryType(null)}>重新选择起点</button> : undefined}
        />
        <ProgressSteps steps={steps} active={activeStep} maxUnlockedStep={maxUnlockedStep} onStepClick={handleStepClick} />
        {creativeSettingsSummary}
        {error && <Notice title="暂时无法继续" copy={error} tone="danger" />}
        {creativeSettingsNotice && <Notice title="创作设定已更新" copy={creativeSettingsNotice} tone="warn" />}

        {sourceStep === 0 && (
          <section className="personalized-workspace-grid">
            <div className="page-stack">
              {sourceLoadFailed && <Notice title="需要重新选择来源绘本" copy="原链接中的来源绘本无法读取，下面可以直接选择其他可定制普通绘本继续。" tone="warn" />}
              {(!sourceStorybookId || sourceLoadFailed) && (
                <Card>
                  <div className="section-head">
                    <div><p className="eyebrow">来源书</p><h2>选择一本普通绘本</h2><p>列表会说明哪些绘本可定制，哪些仍在制作或需要先处理。</p></div>
                    {sourceMeta && <Badge tone="neutral">{sourceMeta.total} 本</Badge>}
                  </div>
                  {sourceLoading ? <SkeletonBlock lines={4} /> : (
                    <div className="source-storybook-list">
                      {sourceBooks.map((book) => {
                        const status = storybookStatusReason(book);
                        const selected = selectedSource?.id === book.id;
                        return (
                          <button
                            key={book.id}
                            className={`source-storybook-row ${selected ? "selected" : ""}`}
                            type="button"
                            disabled={Boolean(status.blocker)}
                            title={status.blocker || undefined}
                            onClick={() => setSelectedSource(book)}
                          >
                            <div className="source-cover" aria-hidden="true">{book.title.slice(0, 1)}</div>
                            <div>
                              <strong>{book.title}</strong>
                              <span>{book.pages.length} 页 · 更新于 {new Date(book.updatedAt).toLocaleDateString()}</span>
                              {status.blocker && <small>{status.blocker}</small>}
                            </div>
                            <Badge tone={status.tone}>{status.label}</Badge>
                          </button>
                        );
                      })}
                    </div>
                  )}
                </Card>
              )}

              {selectedSource && (
                <Card>
                  <div className="section-head">
                    <div><p className="eyebrow">来源摘要</p><h2>《{selectedSource.title}》</h2><p>会保留原书主线、{selectedSource.pages.length} 页结构和阅读节奏；母本不会被修改。</p></div>
                    <Badge tone={sourceBlocker ? "warn" : "good"}>{sourceBlocker ? "暂不可制作" : "可定制"}</Badge>
                  </div>
                  {sourceBlocker && <Notice title="这本绘本暂不能定制" copy={sourceBlocker} tone="warn" />}
                </Card>
              )}

              <Card>
                <div className="section-head">
                  <div><p className="eyebrow">对象</p><h2>选择定制对象</h2><p>批量制作只在已有绘本起点开放，每位对象都要确认主素材或仅使用称呼。</p></div>
                  <div className="segmented-control" role="group" aria-label="制作模式">
                    <button className={recipientMode === "single" ? "active" : ""} type="button" onClick={() => setRecipientMode("single")}>单人</button>
                    <button className={recipientMode === "batch" ? "active" : ""} type="button" onClick={() => setRecipientMode("batch")}>为多人制作</button>
                  </div>
                </div>
                {!children.length ? (
                  <EmptyState title="还没有可选对象" copy="先维护孩子资料，或后续由后端支持仅使用称呼的临时对象。" action={<Link className="button secondary" to={`/app/${workspace.id}/children`}>维护孩子资料</Link>} />
                ) : recipientMode === "single" ? (
                  <>
                    <div className="recipient-grid">
                      {children.map((child) => (
                        <button key={child.id} className={selectedChildId === child.id ? "recipient-card selected" : "recipient-card"} type="button" onClick={() => setSelectedChildId(child.id)}>
                          <strong>{child.nickname}</strong>
                          <span>{child.ageGroup}{child.classroom ? ` · ${child.classroom}` : ""}</span>
                          <small>{child.focus}</small>
                        </button>
                      ))}
                    </div>
                    {selectedChild && (
                      <label className="field-label compact-field" htmlFor="single-material-choice">
                        主素材
                        <select id="single-material-choice" value={singleMaterialChoice} onChange={(event) => setSingleMaterialChoice(event.target.value)}>
                          <option value="">确认主素材</option>
                          <option value="profile">使用儿童档案</option>
                          <option value="name_only">仅使用称呼</option>
                        </select>
                      </label>
                    )}
                  </>
                ) : (
                  <div className="batch-recipient-list">
                    {children.map((child) => {
                      const selected = selectedBatchIds.includes(child.id);
                      return (
                        <div className={selected ? "batch-recipient-row selected" : "batch-recipient-row"} key={child.id}>
                          <label>
                            <input
                              type="checkbox"
                              checked={selected}
                              disabled={!selected && selectedBatchIds.length >= 30}
                              onChange={(event) => setSelectedBatchIds((current) => event.target.checked ? [...current, child.id] : current.filter((id) => id !== child.id))}
                            />
                            <span><strong>{child.nickname}</strong><small>{child.ageGroup}{child.classroom ? ` · ${child.classroom}` : ""}</small></span>
                          </label>
                          {selected && (
                            <select value={batchMaterialChoices[child.id] || ""} onChange={(event) => setBatchMaterialChoices((current) => ({ ...current, [child.id]: event.target.value }))}>
                              <option value="">确认主素材</option>
                              <option value="profile">使用儿童档案</option>
                              <option value="name_only">仅使用称呼</option>
                            </select>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </Card>

              {photoMaterialSection}

              {awaitingPhotoReferences.length > 0 && <Notice title="制作前需要处理照片" copy={`还有 ${awaitingPhotoReferences.length} 张照片未确认用途或同画风参考；可以先继续预览变化，开始制作前需要处理。`} tone="warn" />}
              <div className="creation-action-bar">
                <ActionButton
                  className="button primary"
                  disabled={!canPreview || busy !== null}
                  disabledHint={!selectedSource
                    ? "先选择来源绘本"
                    : sourceBlocker
                      ? sourceBlocker
                      : recipientMode === "single" && selectedChild && !singleMaterialChoice
                        ? "确认主素材或仅使用称呼"
                        : recipientMode === "batch"
                          ? "每位对象都要确认主素材或仅使用称呼"
                          : busy ? "正在生成变化计划" : "先选择对象"}
                  onClick={confirmSourceMaterials}
                >
                  {busy === "source-generating" ? "正在生成变化计划..." : "确认对象与素材"}
                </ActionButton>
              </div>
            </div>
            <aside className="creation-summary-panel">
              <p className="eyebrow">本次创作摘要</p>
              <dl>
                <div><dt>起点</dt><dd>基于已有绘本</dd></div>
                <div><dt>来源</dt><dd>{selectedSource?.title || "未选择"}</dd></div>
                <div><dt>对象</dt><dd>{recipientMode === "batch" ? `${selectedBatchIds.length} 人` : selectedChild?.nickname || "未选择"}</dd></div>
                <div><dt>照片</dt><dd>{activePhotoCount}/{maxPhotoFiles}</dd></div>
              </dl>
            </aside>
          </section>
        )}

        {sourceStep === 1 && sourcePreviewReady && selectedSource && (
          <section className="page-stack">
            <Card>
              <div className="section-head">
                <div><p className="eyebrow">故事预览</p><h2>确认这些变化</h2><p>变化范围由后端根据来源快照、对象和保持偏好生成；制作时会冻结这份计划。</p></div>
                <Badge tone="info">{sourcePlan?.page_plan?.length || 0} 页计划</Badge>
              </div>
              <div className="change-preview-grid">
                {(sourcePlan?.page_plan || []).map((item) => {
                  const pageId = item.source_page_id || "";
                  const preferKeep = Boolean(pageId && sourceKeepPageIds.includes(pageId));
                  const pageReferenceCount = (item.character_reference_ids?.length || 0)
                    + (item.prop_reference_ids?.length || 0)
                    + (item.scene_reference_ids?.length || 0)
                    + (item.asset_reference_ids?.length || 0);
                  return (
                    <div className="change-preview-card" key={pageId || item.page_number}>
                      <Badge tone={sourceDecisionTone(item.decision)}>{sourceDecisionLabel(item.decision)}</Badge>
                      <strong>第 {item.page_number || "-"} 页</strong>
                      <span>{item.title || "页面内容"}</span>
                      {item.reason && <small>{item.reason}</small>}
                      {pageReferenceCount ? <small>本页使用 {pageReferenceCount} 个已确认参考</small> : null}
                      {item.decision !== "redraw_required" && item.decision !== "keep" && pageId && (
                        <label className="toggle-row">
                          <input
                            type="checkbox"
                            checked={preferKeep}
                            disabled={busy !== null}
                            onChange={(event) => toggleSourceKeepPage(pageId, event.target.checked)}
                          />
                          尽量保持这一页
                        </label>
                      )}
                    </div>
                  );
                })}
              </div>
              {(sourcePlan?.confirmed_photo_references || []).length > 0 && (
                <div className="source-reference-list" aria-label="已确认的照片参考">
                  {sourcePlan?.confirmed_photo_references?.map((reference) => (
                    <div className="source-reference-row" key={reference.asset_reference_id}>
                      <strong>{reference.display_name || "照片参考"}</strong>
                      <span>{reference.reference_type_label || reference.reference_type || "绘本参考"}</span>
                      <small>{reference.planned_pages?.length
                        ? `计划使用于第 ${reference.planned_pages.map((page) => page.page_number).filter(Boolean).join("、")} 页`
                        : "请选择实际使用页面"}</small>
                      {reference.usage !== "main_character" && (
                        <div className="inline-actions photo-usage-actions">
                          {(sourcePlan?.page_plan || [])
                            .filter((page) => page.decision === "personalize" || page.decision === "redraw_required")
                            .map((page) => {
                              const ids = reference.reference_type === "character_reference"
                                ? page.character_reference_ids
                                : reference.reference_type === "scene_reference"
                                  ? page.scene_reference_ids
                                  : page.prop_reference_ids;
                              return (
                                <label className="toggle-row" key={page.page_number}>
                                  <input
                                    type="checkbox"
                                    checked={Boolean(reference.asset_reference_id && ids?.includes(reference.asset_reference_id))}
                                    disabled={busy !== null}
                                    onChange={(event) => toggleSourceReferencePage(reference, page, event.target.checked)}
                                  />
                                  第 {page.page_number} 页
                                </label>
                              );
                            })}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </Card>
            <div className="creation-action-bar">
              <button className="button secondary" type="button" onClick={() => setSourceStep(0)}>返回对象与素材</button>
              <ActionButton
                className="button primary"
                disabled={(recipientMode === "single" ? !selectedChild || !singleMaterialChoice : !batchSelectionsReady) || awaitingPhotoReferences.length > 0 || busy !== null}
                disabledHint={awaitingPhotoReferences.length > 0
                    ? "先处理照片用途和同画风参考"
                    : recipientMode === "single" && !selectedChild
                      ? "先选择对象"
                      : recipientMode === "single" && !singleMaterialChoice
                        ? "确认主素材或仅使用称呼"
                      : recipientMode === "batch" && !batchSelectionsReady
                        ? "每位对象都要确认主素材或仅使用称呼"
                        : "正在创建定制绘本"}
                onClick={generateFromSourceStorybook}
              >
                {busy === "source-generating" ? "正在制作定制绘本..." : recipientMode === "batch" ? "开始批量制作" : "开始制作定制绘本"}
              </ActionButton>
            </div>
          </section>
        )}

        {(sourceStep === 2 || sourceRunId) && sourceBatchResult && (
          <section className="page-stack">
            <Card>
              <div className="section-head">
                <div>
                  <p className="eyebrow">{sourceBatchResult.requestedCount > 1 ? "批量制作" : "制作进度"}</p>
                  <h2>已完成 {sourceBatchResult.createdCount}/{sourceBatchResult.requestedCount} 位对象的定制制作</h2>
                  <p>制作会在后台继续进行；成功的作品可进入修改与交付，失败项保留原因，后续可补素材或重试。</p>
                </div>
                <Badge tone={sourceBatchResult.createdCount === sourceBatchResult.requestedCount ? "good" : "warn"}>{sourceBatchResult.items.length} 项</Badge>
              </div>
              <div className="batch-result-list">
                {sourceBatchResult.items.map((item) => {
                  const child = children.find((row) => row.id === item.childId);
                  const isFailedItem = item.status === "failed";
                  const requiresAssetRefresh = item.failureReason?.includes("照片素材已被移除");
                  const presentation = batchResultItemPresentation(item);
                  return (
                    <div className="batch-result-row" key={item.childId || item.storybook?.id || item.failureReason}>
                      <Badge tone={presentation.tone}>{presentation.label}</Badge>
                      <div>
                        <strong>{child?.nickname || item.storybook?.title || "对象"}</strong>
                        <span>{presentation.detail}</span>
                      </div>
                      {item.storybook && (
                        <Link className="button secondary compact" to={batchReviewUrl(workspace.id, item.storybook.id, item.runItemId)}>
                          修改与交付
                        </Link>
                      )}
                      {!item.storybook && item.runItemId && item.storybookLoadFailed && (
                        <button
                          className="button secondary compact"
                          type="button"
                          disabled={reloadingRunItemId === item.runItemId}
                          onClick={() => reloadSourceOutputStorybook(item.runItemId as string)}
                        >
                          {reloadingRunItemId === item.runItemId ? "加载中..." : "重新加载作品"}
                        </button>
                      )}
                      {!item.storybook && item.runItemId && isFailedItem && !requiresAssetRefresh && (
                        <span className="inline-actions">
                          <button
                            className="button secondary compact"
                            type="button"
                            disabled={retryingRunItemId === item.runItemId || abandoningRunItemId === item.runItemId}
                            onClick={() => retrySourceBatchItem(item.runItemId as string)}
                          >
                            {retryingRunItemId === item.runItemId ? "重试中..." : "重试"}
                          </button>
                          <button
                            className="button ghost compact"
                            type="button"
                            disabled={retryingRunItemId === item.runItemId || abandoningRunItemId === item.runItemId}
                            onClick={() => abandonSourceBatchItem(item.runItemId as string)}
                          >
                            {abandoningRunItemId === item.runItemId ? "放弃中..." : "放弃"}
                          </button>
                        </span>
                      )}
                    </div>
                  );
                })}
              </div>
              <div className="creation-action-bar">
                {sourceBatchResult.items.some((item) => item.failureReason?.includes("照片素材已被移除")) && (
                  <button className="button secondary" type="button" onClick={returnToSourceMaterials}>
                    调整素材并重新预览
                  </button>
                )}
                <button className="button secondary" type="button" onClick={() => setSourceStep(1)}>返回变化预览</button>
                {sourceBatchResult.items.some((item) => ["queued", "running", "retrying"].includes(item.status)) && (
                  <button className="button ghost" type="button" disabled={cancelingSourceRun} onClick={cancelSourceRun}>
                    {cancelingSourceRun ? "正在取消..." : "取消本次制作"}
                  </button>
                )}
                {sourceBatchResult.storybooks[0] && (
                  <Link
                    className="button primary"
                    to={batchReviewUrl(
                      workspace.id,
                      sourceBatchResult.storybooks[0].id,
                      sourceBatchResult.items.find((item) => item.storybook?.id === sourceBatchResult.storybooks[0]?.id)?.runItemId,
                    )}
                  >
                    检查第一本
                  </Link>
                )}
              </div>
            </Card>
          </section>
        )}
        {creativeSettingsDrawer}
        {pendingVisualStyleChangeModal}
      </div>
    );
  }

  if (busy === "loading" && !session && !latestDraft) {
    return <EmptyState title="正在准备专属创作" copy="正在检查可恢复的故事草稿。" />;
  }

  if (!session) {
    return (
      <div className="page-stack">
        <PageHeader eyebrow="专属绘本创作" title="想做一本怎样的专属绘本？" copy="说一句就可以。你可以写一个人、一件喜欢的东西，或最近发生的小事。" actions={<button className="button secondary" type="button" onClick={() => setEntryType(null)}>重新选择起点</button>} />
        {error && <Notice title="暂时无法继续" copy={error} tone="danger" />}
        {latestDraft && (
          <Notice
            title={latestReadyStorybookId ? "上次专属绘本已完成" : "发现上次创作"}
            copy={latestReadyStorybookId ? `“${latestDraft.quickIdea.slice(0, 30)}${latestDraft.quickIdea.length > 30 ? "..." : ""}”已生成，可以直接进入绘本检查。` : `“${latestDraft.quickIdea.slice(0, 30)}${latestDraft.quickIdea.length > 30 ? "..." : ""}”仍可继续。`}
            tone="info"
            action={latestReadyStorybookId ? <span className="inline-actions"><button className="button secondary" type="button" onClick={() => setLatestDraft(null)}>新建专属绘本</button><Link className="button primary" to={`/app/${workspace.id}/storybooks/${latestReadyStorybookId}/review?result=personalized`}>查看已完成绘本</Link></span> : <span className="inline-actions"><button className="button secondary" type="button" onClick={abandonDraft}>放弃并新建</button><button className="button primary" type="button" onClick={resumeDraft}>继续上次创作</button></span>}
          />
        )}
        <Card className="personalized-idea-card">
          <label className="field-label" htmlFor="personalized-recipient">这本绘本送给谁（可选）</label>
          <input id="personalized-recipient" value={recipientName} onChange={(event) => setRecipientName(event.target.value)} placeholder="例如：乐乐" />
          <label className="field-label" htmlFor="personalized-idea">故事想法</label>
          <textarea id="personalized-idea" value={idea} onChange={(event) => setIdea(event.target.value)} placeholder="给 4 岁的乐乐做一本关于蓝色积木车和轮流的小故事。" rows={5} />
          <p className={ideaValidationMessage(idea) ? "form-helper warn" : "form-helper"}>
            {ideaValidationMessage(idea) || "想法长度足够了，下一步会整理对象、素材和故事方向。"}
          </p>
          <div className="inline-actions personalized-examples">
            {["成长小事", "特别纪念", "自由创作"].map((example) => <button key={example} className="chip-button" type="button" onClick={() => setIdea(example === "成长小事" ? "想做一本帮助孩子理解分享和等待的小故事。" : example === "特别纪念" ? "记录孩子生日那天最开心的一件小事。" : "")}>{example}</button>)}
          </div>
              <div className="creation-action-bar">
            <span className="form-helper">下一步可以补充想留在故事里的真实物品、地点或一句话。</span>
            <ActionButton
              className="button primary"
              disabled={busy !== null || idea.trim().length < 8}
              disabledHint="先写至少一句完整的故事想法"
              onClick={startFromIdea}
            >
              {busy === "directions" ? "正在整理故事..." : "看看故事怎么讲"}
            </ActionButton>
          </div>
        </Card>
        {photoMaterialSection}
      </div>
    );
  }

  return (
    <div className="page-stack personalized-flow">
      <PageHeader eyebrow="专属绘本创作" title={activeStep === 0 ? "对象与素材" : activeStep === 1 ? "故事预览" : "正在制作专属绘本"} copy={activeStep === 0 ? "补充要留在故事里的对象、真实细节和照片。" : activeStep === 1 ? "选一个喜欢的讲法，再继续完成故事。" : "可以离开页面，完成后会保留在作品列表中。"} actions={activeStep < 2 ? <button className="button secondary" type="button" onClick={() => { setIdea(session.quickIdea); setEditingIdea((value) => !value); }}>{editingIdea ? "收起修改" : "修改想法"}</button> : undefined} />
      <ProgressSteps steps={steps} active={activeStep} maxUnlockedStep={maxUnlockedStep} onStepClick={handleStepClick} />
      {creativeSettingsSummary}
      {error && <Notice title="这一步没有完成" copy={error} tone="danger" />}
      {creativeSettingsNotice && <Notice title="创作设定已更新" copy={creativeSettingsNotice} tone="warn" />}
      {creationWarnings.map((warning) => (
        <Notice
          key={`${warning.code}-${warning.asset_reference_ids?.join("-") || "all"}`}
          title="制作前需要处理照片"
          copy={warning.message}
          tone="warn"
        />
      ))}
      {session.status === "failed" && <Notice title="制作没有完成" copy={session.generationSummary.qualityNotice || "已保留你的故事大纲和素材。回到故事预览后可重新开始制作。"} tone="danger" />}
      {activeStep < 2 && (
        <div className="personalized-context" aria-label="本次专属内容">
          {editingIdea ? <><label className="field-label" htmlFor="personalized-edit-idea">你的想法</label><textarea id="personalized-edit-idea" value={idea} onChange={(event) => setIdea(event.target.value)} rows={3} /><p className={ideaValidationMessage(idea) ? "form-helper warn" : "form-helper"}>{ideaValidationMessage(idea) || "想法长度足够了，可以重新整理故事。"}</p><div className="inline-actions"><ActionButton className="button primary" disabled={busy !== null || idea.trim().length < 8} disabledHint="先写至少一句完整的故事想法" onClick={saveIdeaChanges}>保存并重新整理</ActionButton><button className="button secondary" type="button" onClick={() => { setIdea(session.quickIdea); setEditingIdea(false); }}>取消</button></div></> : <p><strong>你的想法：</strong>{session.quickIdea}</p>}
          <div className="inline-actions"><span>本次专属内容：</span>{selectedMaterialLabels.length ? selectedMaterialLabels.map((label) => <Badge key={label} tone="info">{label}</Badge>) : <span className="muted">暂未额外添加</span>}</div>
        </div>
      )}

      {activeStep === 0 && (
        <section className="page-stack">
          <Card>
            <div className="section-head"><div><p className="eyebrow">对象与素材</p><h2>想把什么留在故事里？</h2><p>{session.understanding.summary}</p></div></div>
            <div className="material-chip-list">
              {materials.map((material) => (
                <span className={material.source === "user_added" ? "material-chip confirmed" : "material-chip"} key={material.id}>
                  {material.label}<small>{material.source === "user_added" ? material.type === "character" ? "制作对象" : "已确认" : "来自想法"}</small>{!(material.source === "user_added" && material.type === "character") && <button className="material-chip-remove" type="button" aria-label={`移除${material.label}`} title={`移除${material.label}`} onClick={() => { setMaterialRemovalError(""); setMaterialPendingRemoval(material); }}><X size={14} aria-hidden="true" /></button>}
                </span>
              ))}
            </div>
            <div className="inline-form">
              <input value={newMaterial} onChange={(event) => setNewMaterial(event.target.value)} placeholder="添加一个想保留的真实细节" />
              <ActionButton className="button secondary" disabled={!newMaterial.trim() || busy !== null || lockedMaterialCount >= 3} disabledHint={lockedMaterialCount >= 3 ? "最多保留 3 个专属素材" : "先写下一个细节"} onClick={addMaterial}>加入</ActionButton>
            </div>
            <div className="creation-action-bar">
              <span className="form-helper">已确认 {lockedMaterialCount}/3 个专属素材。</span>
              {directions.length > 0 ? <button className="button primary" type="button" onClick={() => setEditingMaterials(false)}>返回故事预览</button> : <ActionButton className="button primary" disabled={busy !== null} disabledHint="正在保存素材" onClick={refreshDirections}>{busy === "directions" ? "正在整理故事..." : "看看故事怎么讲"}</ActionButton>}
            </div>
          </Card>
          {photoMaterialSection}
          {awaitingPhotoReferences.length > 0 && <Notice title="制作前需要处理照片" copy={`还有 ${awaitingPhotoReferences.length} 张照片未确认用途或同画风参考；可以先继续整理故事，开始制作前需要处理。`} tone="warn" />}
        </section>
      )}

      {activeStep === 1 && !outline && (
        <section className="page-stack">
          {awaitingPhotoReferences.length > 0 && <Notice title="制作前需要处理照片" copy={`还有 ${awaitingPhotoReferences.length} 张照片未确认用途或同画风参考；可以先继续整理故事，开始制作前需要处理。`} tone="warn" />}
          <div className="section-head"><div><p className="eyebrow">故事预览</p><h2>这个故事想怎样讲？</h2><p>选一个你喜欢的讲法，系统会继续完成完整故事。</p></div></div>
          <div className="direction-grid">{directions.map((direction) => <button className={`direction-card ${selectedDirection?.id === direction.id ? "selected" : ""}`} type="button" key={direction.id} disabled={busy !== null} onClick={() => chooseDirection(direction)}><strong>{direction.title}</strong><span>{direction.summary}</span><em>会出现：{materialLabels(direction.materialIds).join("、") || "待补充"}</em></button>)}</div>
          {selectedDirection && missingDirectionMaterials.length === 0 && selectedMaterialLabels.length > 0 && <p className="form-helper">专属内容会这样出现：{selectedMaterialLabels.slice(0, 2).join("、")}会进入故事的关键情节。</p>}
          {selectedDirection && missingDirectionMaterials.length > 0 && <Notice title="这个走向还没有安排全部专属素材" copy={`尚未安排：${missingDirectionMaterials.map((item) => item.label).join("、")}。请选择其他走向，或调整素材后重新生成方向。`} tone="warn" />}
          <div className="creation-action-bar"><div className="inline-actions"><button className="button secondary" type="button" onClick={() => setEditingMaterials(true)}>调整对象与素材</button><ActionButton className="button secondary" disabled={busy !== null} disabledHint="正在整理" onClick={refreshDirections}>换一个故事走向</ActionButton></div><ActionButton className="button primary" disabled={!selectedDirection || busy !== null || missingDirectionMaterials.length > 0} disabledHint={!selectedDirection ? "先选一个故事走向" : missingDirectionMaterials.length > 0 ? "先确认每个专属素材都有故事落点" : "正在整理故事大纲"} onClick={generateOutline}>{busy === "outline" ? "正在生成故事大纲..." : "按这个故事继续"}</ActionButton></div>
        </section>
      )}

      {activeStep === 1 && outline && (
        <section className="page-stack">
          {awaitingPhotoReferences.length > 0 && <Notice title="制作前需要处理照片" copy={`还有 ${awaitingPhotoReferences.length} 张照片未确认用途或同画风参考；处理完后才能开始制作。`} tone="warn" />}
          <div className="section-head"><div><p className="eyebrow">故事走向</p><h2>故事会这样展开</h2><p>{outline.summary}</p></div></div>
          <div className="outline-list">{outline.pages.map((page) => <Card key={page.pageNumber} className="outline-row"><Badge tone="info">{page.pageNumber}</Badge><div><strong>{page.summary}</strong><p className="form-helper">本页素材：{materialLabels(page.materialIds).join("、") || "待补充"}</p><div className="inline-form"><input value={pageInstruction[page.pageNumber] || ""} onChange={(event) => setPageInstruction((current) => ({ ...current, [page.pageNumber]: event.target.value }))} placeholder="补充这页（可选）" /><ActionButton className="button secondary" disabled={!pageInstruction[page.pageNumber]?.trim() || busy !== null} disabledHint="先写下这页的要求" onClick={() => updateOutlinePage(page.pageNumber)}>{busy === `page-${page.pageNumber}` ? "更新中..." : "更新这页"}</ActionButton></div></div></Card>)}</div>
          {missingOutlineMaterials.length > 0 && <Notice title="大纲还没有安排全部专属素材" copy={`尚未安排：${missingOutlineMaterials.map((item) => item.label).join("、")}。请回到素材或故事走向调整后再制作。`} tone="warn" />}
          <div className="creation-action-bar"><div className="inline-actions"><button className="button secondary" type="button" onClick={() => setEditingMaterials(true)}>调整对象与素材</button><ActionButton className="button secondary" disabled={busy !== null} disabledHint="正在整理新的故事走向" onClick={refreshDirections}>{busy === "directions" ? "正在整理故事..." : "换一个故事走向"}</ActionButton></div><ActionButton className="button primary" disabled={busy !== null || missingOutlineMaterials.length > 0 || awaitingPhotoReferences.length > 0} disabledHint={awaitingPhotoReferences.length > 0 ? "先处理照片用途和同画风参考" : missingOutlineMaterials.length > 0 ? "先确认每个专属素材都进入大纲" : "正在更新故事走向"} onClick={generateBook}>{busy === "generating" ? "正在开始制作..." : session.status === "failed" ? "重新开始制作" : "开始制作专属绘本"}</ActionButton></div>
        </section>
      )}

      {activeStep === 2 && (
        <Card className="creation-progress-card">
          <Badge tone="info">制作中</Badge>
          <h2>正在把故事画出来</h2>
          <p>{session.generationSummary.textStatus === "succeeded" ? "文字已经完成，正在整理画面。" : "正在确认故事文字和画面。"}</p>
          <div className="creation-stage-list">{generationStages(session).map((stage, index) => <div className={stage.state} key={stage.label}><span>{index + 1}</span>{stage.label}</div>)}</div>
          <div className="creation-action-bar">
            <p className="form-helper">完成后会保留在作品列表中，可以随时回来查看。</p>
            <div className="inline-actions">
              <Link className="button secondary" to={`/app/${workspace.id}/storybooks`}>先去做别的</Link>
              <button className="button ghost compact" type="button" disabled={busy !== null} onClick={() => setCancelDirectCreationConfirmOpen(true)}>取消制作</button>
            </div>
          </div>
        </Card>
      )}
      {activeStep === 3 && (
        <Card className="creation-progress-card">
          <Badge tone="good">制作完成</Badge>
          <h2>专属绘本已准备好</h2>
          <p>你可以进入绘本检查内容与画面，也可以稍后从园所绘本列表继续处理。</p>
          <div className="creation-action-bar">
            <Link className="button secondary" to={`/app/${workspace.id}/storybooks`}>返回园所绘本</Link>
            {session.storybookId
              ? <Link className="button primary" to={`/app/${workspace.id}/storybooks/${session.storybookId}/review?result=personalized`}>查看并检查绘本</Link>
              : <button className="button primary" type="button" onClick={() => void refreshCurrentSession(session.id)}>刷新作品状态</button>}
          </div>
        </Card>
      )}
      {cancelDirectCreationConfirmOpen && (
        <Modal title="确认取消本次制作？" onClose={() => busy === null && setCancelDirectCreationConfirmOpen(false)}>
          <p>取消后，正在生成的专属绘本会停止，本次未完成的制作不能继续恢复。</p>
          <div className="modal-actions">
            <button className="button secondary" type="button" disabled={busy !== null} onClick={() => setCancelDirectCreationConfirmOpen(false)}>继续制作</button>
            <ActionButton className="button danger" disabled={busy !== null} disabledHint="正在取消本次制作" onClick={cancelDirectCreationRun}>{busy === "loading" ? "正在取消..." : "确认取消"}</ActionButton>
          </div>
        </Modal>
      )}
      {pendingVisualStyleChangeModal}
      {materialPendingRemoval && (
        <Modal title={`移除“${materialPendingRemoval.label}”？`} onClose={() => { if (busy === null) { setMaterialRemovalError(""); setMaterialPendingRemoval(null); } }}>
          <p>移除后，这项素材不会再进入后续故事计划或新的生成输入。</p>
          {(directions.length > 0 || outline) && <p className="form-helper">当前已选故事走向和大纲会清空，需要根据剩余素材重新整理。</p>}
          {materialRemovalError && <p className="form-helper warn" role="alert">{materialRemovalError}</p>}
          <div className="modal-actions">
            <button className="button secondary" type="button" disabled={busy !== null} onClick={() => { setMaterialRemovalError(""); setMaterialPendingRemoval(null); }}>保留素材</button>
            <ActionButton className="button danger" disabled={busy !== null} disabledHint="正在移除素材" onClick={() => void removeMaterial(materialPendingRemoval)}>确认移除</ActionButton>
          </div>
        </Modal>
      )}
      {creativeSettingsDrawer}
    </div>
  );
}
