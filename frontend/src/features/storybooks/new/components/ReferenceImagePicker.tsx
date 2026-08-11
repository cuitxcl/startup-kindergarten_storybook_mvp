import { useEffect, useState } from "react";
import {
  downloadGenerationImageFile,
  getStorybook,
  listStorybookImageVariants,
  selectStorybookImageVariant,
} from "../../../../api/client";
import { Badge } from "../../../../components/ui";
import type { StorybookImageVariant } from "../../../../types/domain";
import { rolesFromStorybook } from "../helpers";
import type { EditableRole } from "../types";

type ReferenceImagePickerProps = {
  workspaceId: string;
  storybookId?: string;
  role: EditableRole;
  roleIndex: number;
  referenceIsGenerating: boolean;
  variantRefreshKey?: number;
  onGenerateReference?: (role: EditableRole, roleIndex: number) => Promise<void>;
  onRolesRefresh?: (roles: EditableRole[]) => void;
};

export function ReferenceImagePicker({
  workspaceId,
  storybookId,
  role,
  roleIndex,
  referenceIsGenerating,
  variantRefreshKey,
  onGenerateReference,
  onRolesRefresh,
}: ReferenceImagePickerProps) {
  const [referenceVariants, setReferenceVariants] = useState<StorybookImageVariant[]>([]);
  const [selectingVariantId, setSelectingVariantId] = useState<string | null>(null);
  const [zoomedVariantId, setZoomedVariantId] = useState<string | null>(null);
  const displayReferenceVariants = referenceVariants.filter((variant) => variant.status !== "failed");

  useEffect(() => {
    if (!storybookId || !role.id) {
      setReferenceVariants([]);
      return;
    }
    let active = true;
    listStorybookImageVariants(workspaceId, storybookId, {
      targetType: "role_reference",
      targetId: role.id,
    })
      .then((variants) => {
        if (active) setReferenceVariants(variants);
      })
      .catch(() => {
        if (active) setReferenceVariants([]);
      });
    return () => {
      active = false;
    };
  }, [role.id, storybookId, variantRefreshKey, workspaceId]);

  const selectReferenceVariant = async (variant: StorybookImageVariant) => {
    if (!storybookId) return;
    setSelectingVariantId(variant.id);
    try {
      await selectStorybookImageVariant(workspaceId, storybookId, variant.id);
      const refreshed = await getStorybook(workspaceId, storybookId);
      onRolesRefresh?.(rolesFromStorybook(refreshed.roles));
      setReferenceVariants(await listStorybookImageVariants(workspaceId, storybookId, {
        targetType: "role_reference",
        targetId: variant.targetId,
      }));
    } finally {
      setSelectingVariantId(null);
    }
  };

  return (
    <>
      <div className="reference-actions-panel">
        <div>
          <strong>{referenceVariants.length ? `已保留 ${referenceVariants.length} 张候选参考图` : "先生成一张角色参考图"}</strong>
          <span>
            {referenceIsGenerating
              ? "正在生成新候选，完成前当前使用图不会被覆盖。"
              : referenceVariants.length
                ? "点击图片查看大图；选中满意版本后再进入分页。"
                : "参考图会用于后续分页插图保持同一形象。"}
          </span>
        </div>
        <button
          className="button primary compact-action"
          type="button"
          disabled={referenceIsGenerating || !onGenerateReference}
          onClick={() => onGenerateReference?.(role, roleIndex)}
        >
          {referenceIsGenerating ? "生成中..." : role.referenceImageUrl ? "重新生成" : "生成参考图"}
        </button>
      </div>
      <ImageVariantStrip
        workspaceId={workspaceId}
        variants={displayReferenceVariants}
        emptyText={role.id ? "还没有历史参考图" : "生成参考图后会在这里保留历史候选"}
        onOpen={(variant) => setZoomedVariantId(variant.id)}
      />
      {zoomedVariantId && (
        <ReferenceVariantLightbox
          workspaceId={workspaceId}
          variants={displayReferenceVariants}
          variantId={zoomedVariantId}
          alt={`${role.name || "角色"}的候选参考图`}
          selectingVariantId={selectingVariantId}
          onClose={() => setZoomedVariantId(null)}
          onChangeVariant={setZoomedVariantId}
          onUse={(variant) => void selectReferenceVariant(variant)}
        />
      )}
    </>
  );
}

function ImageVariantStrip({
  workspaceId,
  variants,
  emptyText,
  onOpen,
}: {
  workspaceId: string;
  variants: StorybookImageVariant[];
  emptyText: string;
  onOpen: (variant: StorybookImageVariant) => void;
}) {
  if (!variants.length) {
    return <div className="image-variant-strip empty">{emptyText}</div>;
  }
  return (
    <div className="image-variant-strip">
      {variants.map((variant) => (
        <ImageVariantThumb
          key={variant.id}
          workspaceId={workspaceId}
          variant={variant}
          onOpen={onOpen}
        />
      ))}
    </div>
  );
}

function ImageVariantThumb({
  workspaceId,
  variant,
  onOpen,
}: {
  workspaceId: string;
  variant: StorybookImageVariant;
  onOpen: (variant: StorybookImageVariant) => void;
}) {
  const [previewUrl, setPreviewUrl] = useState("");
  const [previewError, setPreviewError] = useState("");

  useEffect(() => {
    if (!variant.imageUrl || !variant.generationJobId || variant.status !== "ready") {
      setPreviewUrl("");
      setPreviewError("");
      return;
    }
    let active = true;
    let objectUrl = "";
    setPreviewUrl("");
    setPreviewError("");
    downloadGenerationImageFile(workspaceId, variant.generationJobId)
      .then((file) => {
        if (!active) return;
        objectUrl = window.URL.createObjectURL(file);
        setPreviewUrl(objectUrl);
      })
      .catch((err) => {
        if (active) setPreviewError(err instanceof Error ? err.message : "读取失败");
      });
    return () => {
      active = false;
      if (objectUrl) window.URL.revokeObjectURL(objectUrl);
    };
  }, [variant.generationJobId, variant.imageUrl, variant.status, workspaceId]);

  const statusLabel = variant.isSelected
    ? "当前使用"
    : variant.status === "generating"
      ? "生成中"
      : variant.status === "failed"
        ? "失败"
        : "可使用";

  return (
    <div className={`image-variant-thumb ${variant.isSelected ? "selected" : ""}`}>
      <button
        type="button"
        className="image-variant-preview"
        disabled={!previewUrl}
        onClick={() => previewUrl && onOpen(variant)}
        title={previewUrl ? "查看大图" : statusLabel}
      >
        {previewUrl ? <img src={previewUrl} alt="候选参考图" /> : <span>{previewError || statusLabel}</span>}
      </button>
      <Badge tone={variant.isSelected ? "good" : variant.status === "failed" ? "danger" : variant.status === "generating" ? "neutral" : "info"}>{statusLabel}</Badge>
    </div>
  );
}

function ReferenceVariantLightbox({
  workspaceId,
  variants,
  variantId,
  alt,
  selectingVariantId,
  onClose,
  onChangeVariant,
  onUse,
}: {
  workspaceId: string;
  variants: StorybookImageVariant[];
  variantId: string;
  alt: string;
  selectingVariantId: string | null;
  onClose: () => void;
  onChangeVariant: (variantId: string) => void;
  onUse: (variant: StorybookImageVariant) => void;
}) {
  const readyVariants = variants.filter((variant) => variant.status === "ready" && variant.imageUrl && variant.generationJobId);
  const foundIndex = readyVariants.findIndex((variant) => variant.id === variantId);
  const currentIndex = foundIndex >= 0 ? foundIndex : 0;
  const currentVariant = readyVariants[currentIndex];
  const [previewUrl, setPreviewUrl] = useState("");
  const [previewError, setPreviewError] = useState("");
  const [slideDirection, setSlideDirection] = useState(0);

  const go = (offset: number) => {
    if (!readyVariants.length) return;
    const nextIndex = (currentIndex + offset + readyVariants.length) % readyVariants.length;
    setSlideDirection(offset);
    onChangeVariant(readyVariants[nextIndex].id);
  };

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
      if (event.key === "ArrowLeft") go(-1);
      if (event.key === "ArrowRight") go(1);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentIndex, readyVariants.length, onClose]);

  useEffect(() => {
    if (!currentVariant?.generationJobId) {
      setPreviewUrl("");
      setPreviewError("");
      return;
    }
    let active = true;
    let objectUrl = "";
    setPreviewUrl("");
    setPreviewError("");
    downloadGenerationImageFile(workspaceId, currentVariant.generationJobId)
      .then((file) => {
        if (!active) return;
        objectUrl = window.URL.createObjectURL(file);
        setPreviewUrl(objectUrl);
      })
      .catch((err) => {
        if (active) setPreviewError(err instanceof Error ? err.message : "大图读取失败");
      });
    return () => {
      active = false;
      if (objectUrl) window.URL.revokeObjectURL(objectUrl);
    };
  }, [currentVariant?.generationJobId, workspaceId]);

  if (!currentVariant) return null;
  const isSelecting = selectingVariantId === currentVariant.id;
  const useLabel = currentVariant.isSelected ? "当前使用" : isSelecting ? "切换中..." : "设为当前使用";

  return (
    <div
      className="modal-backdrop image-lightbox-backdrop reference-gallery-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label={alt || "参考图候选预览"}
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <figure className="image-lightbox reference-gallery">
        <button className="icon-button image-lightbox-close" type="button" onClick={onClose} aria-label="关闭放大预览">
          ×
        </button>
        {readyVariants.length > 1 && (
          <>
            <button className="reference-gallery-nav prev" type="button" onClick={() => go(-1)} aria-label="上一张参考图">‹</button>
            <button className="reference-gallery-nav next" type="button" onClick={() => go(1)} aria-label="下一张参考图">›</button>
          </>
        )}
        <div className="reference-gallery-stage">
          {previewUrl ? (
            <button
              key={currentVariant.id}
              className={`reference-gallery-image ${slideDirection > 0 ? "slide-next" : slideDirection < 0 ? "slide-prev" : ""}`}
              type="button"
              disabled={currentVariant.isSelected || isSelecting}
              onClick={() => {
                if (!currentVariant.isSelected && !isSelecting) onUse(currentVariant);
              }}
              title={currentVariant.isSelected ? "当前使用" : "点击设为当前使用"}
            >
              <img src={previewUrl} alt={alt} />
            </button>
          ) : (
            <div className="reference-gallery-loading">{previewError || "正在读取大图"}</div>
          )}
        </div>
        <figcaption className="reference-gallery-caption">
          <span>{currentIndex + 1} / {readyVariants.length}</span>
          <Badge tone={currentVariant.isSelected ? "good" : "info"}>{currentVariant.isSelected ? "当前使用" : "可使用"}</Badge>
          <button
            className="button primary compact"
            type="button"
            disabled={currentVariant.isSelected || isSelecting}
            onClick={() => onUse(currentVariant)}
          >
            {useLabel}
          </button>
        </figcaption>
      </figure>
    </div>
  );
}
