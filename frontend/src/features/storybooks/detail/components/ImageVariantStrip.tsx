import { useEffect, useState } from "react";
import { downloadGenerationImageFile } from "../../../../api/client";
import { Badge } from "../../../../components/ui";
import type { StorybookImageVariant } from "../../../../types/domain";
import { cacheImagePreview, getCachedImagePreview } from "../../../../utils/imagePreviewCache";

export function ImageVariantStrip({
  workspaceId,
  variants,
  selectingVariantId,
  emptyText,
  aspectRatio,
  onSelect,
  onZoom,
}: {
  workspaceId: string;
  variants: StorybookImageVariant[];
  selectingVariantId: string | null;
  emptyText: string;
  aspectRatio?: string;
  onSelect: (variant: StorybookImageVariant) => void;
  onZoom: (src: string) => void;
}) {
  const displayVariants = variants.filter((variant) => variant.status !== "failed");
  if (!displayVariants.length) {
    return <div className="image-variant-strip empty">{emptyText}</div>;
  }
  return (
    <div className="image-variant-strip">
      {displayVariants.map((variant) => (
        <ImageVariantThumb
          key={variant.id}
          workspaceId={workspaceId}
          variant={variant}
          selecting={selectingVariantId === variant.id}
          aspectRatio={aspectRatio}
          onSelect={onSelect}
          onZoom={onZoom}
        />
      ))}
    </div>
  );
}

function ImageVariantThumb({
  workspaceId,
  variant,
  selecting,
  aspectRatio,
  onSelect,
  onZoom,
}: {
  workspaceId: string;
  variant: StorybookImageVariant;
  selecting: boolean;
  aspectRatio?: string;
  onSelect: (variant: StorybookImageVariant) => void;
  onZoom: (src: string) => void;
}) {
  const [previewUrl, setPreviewUrl] = useState("");
  const [previewError, setPreviewError] = useState("");

  useEffect(() => {
    if (!variant.imageUrl || !variant.generationJobId || variant.status !== "ready") {
      setPreviewUrl("");
      setPreviewError("");
      return;
    }
    const cached = getCachedImagePreview(variant.generationJobId);
    if (cached) {
      setPreviewUrl(cached);
      setPreviewError("");
      return;
    }
    let active = true;
    setPreviewUrl("");
    setPreviewError("");
    downloadGenerationImageFile(workspaceId, variant.generationJobId)
      .then((file) => {
        if (!active) return;
        const url = window.URL.createObjectURL(file);
        cacheImagePreview(variant.generationJobId || variant.id, url);
        setPreviewUrl(url);
      })
      .catch((err) => {
        if (active) {
          setPreviewError(err instanceof Error ? err.message : "读取失败");
        }
      });
    return () => {
      active = false;
    };
  }, [variant.generationJobId, variant.id, variant.imageUrl, variant.status, workspaceId]);

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
        style={aspectRatio ? { aspectRatio } : undefined}
        disabled={!previewUrl}
        onClick={() => previewUrl && onZoom(previewUrl)}
        title={previewUrl ? "查看大图" : statusLabel}
      >
        {previewUrl ? <img src={previewUrl} alt="候选图" /> : <span>{previewError || statusLabel}</span>}
      </button>
      <Badge tone={variant.isSelected ? "good" : variant.status === "failed" ? "danger" : variant.status === "generating" ? "neutral" : "info"}>{statusLabel}</Badge>
      {variant.status === "ready" && !variant.isSelected && (
        <button className="button secondary compact" type="button" disabled={selecting} onClick={() => onSelect(variant)}>
          {selecting ? "切换中..." : "使用这张"}
        </button>
      )}
    </div>
  );
}
