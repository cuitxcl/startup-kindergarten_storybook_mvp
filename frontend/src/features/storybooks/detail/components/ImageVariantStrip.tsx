import { useEffect, useMemo, useState } from "react";
import { downloadGenerationImageFile } from "../../../../api/client";
import { ImageLightbox } from "../../../../components/ui";
import type { StorybookImageVariant } from "../../../../types/domain";
import { cacheImagePreview, getCachedImagePreview } from "../../../../utils/imagePreviewCache";

type PreviewState = Record<string, { url?: string; error?: string }>;

export function ImageVariantStrip({
  workspaceId,
  variants,
  selectingVariantId,
  emptyText,
  aspectRatio,
  label = "候选图",
  onSelect,
}: {
  workspaceId: string;
  variants: StorybookImageVariant[];
  selectingVariantId: string | null;
  emptyText: string;
  aspectRatio?: string;
  label?: string;
  onSelect: (variant: StorybookImageVariant) => void;
}) {
  const displayVariants = variants.filter((variant) => variant.status !== "failed");
  const [previews, setPreviews] = useState<PreviewState>({});
  const [previewingVariantId, setPreviewingVariantId] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const candidates = displayVariants.filter((variant) => variant.status === "ready" && variant.imageUrl && variant.generationJobId);
    const cached = candidates.reduce<PreviewState>((result, variant) => {
      const url = getCachedImagePreview(variant.generationJobId || variant.id);
      if (url) result[variant.id] = { url };
      return result;
    }, {});
    const missing = candidates.filter((variant) => !cached[variant.id]);
    setPreviews(cached);

    Promise.all(missing.map(async (variant) => {
      try {
        const file = await downloadGenerationImageFile(workspaceId, variant.generationJobId!);
        const url = window.URL.createObjectURL(file);
        cacheImagePreview(variant.generationJobId || variant.id, url);
        return [variant.id, { url }] as const;
      } catch (error) {
        return [variant.id, { error: error instanceof Error ? error.message : "读取失败" }] as const;
      }
    })).then((entries) => {
      if (active) setPreviews((current) => ({ ...current, ...Object.fromEntries(entries) }));
    });
    return () => { active = false; };
  }, [workspaceId, variants]);

  const gallery = useMemo(
    () => displayVariants.filter((variant) => variant.status === "ready" && previews[variant.id]?.url),
    [displayVariants, previews],
  );
  const previewingIndex = gallery.findIndex((variant) => variant.id === previewingVariantId);
  const previewingVariant = previewingIndex >= 0 ? gallery[previewingIndex] : undefined;

  if (!displayVariants.length) return <div className="image-variant-strip empty">{emptyText}</div>;

  return (
    <>
      <div className="image-variant-strip" aria-label={`${label}列表`}>
        {displayVariants.map((variant, index) => {
          const preview = previews[variant.id];
          const statusLabel = variant.isSelected ? "当前使用" : variant.status === "generating" ? "生成中" : "可使用";
          return (
            <div className={`image-variant-thumb ${variant.isSelected ? "selected" : ""}`} key={variant.id} aria-label={`${label} ${index + 1}，${statusLabel}`}>
              <button
                type="button"
                className="image-variant-preview"
                style={aspectRatio ? { aspectRatio } : undefined}
                disabled={!preview?.url}
                onClick={() => preview?.url && setPreviewingVariantId(variant.id)}
                title={preview?.url ? `放大查看${label} ${index + 1}` : statusLabel}
              >
                {preview?.url ? <img src={preview.url} alt={`${label} ${index + 1}`} /> : <span>{preview?.error || statusLabel}</span>}
              </button>
            </div>
          );
        })}
      </div>
      {previewingVariant && (
        <ImageLightbox
          src={previews[previewingVariant.id].url!}
          alt={`${label} ${previewingIndex + 1}`}
          positionLabel={`${previewingIndex + 1} / ${gallery.length}`}
          previousAction={previewingIndex > 0 ? () => setPreviewingVariantId(gallery[previewingIndex - 1].id) : undefined}
          nextAction={previewingIndex < gallery.length - 1 ? () => setPreviewingVariantId(gallery[previewingIndex + 1].id) : undefined}
          primaryAction={previewingVariant.isSelected ? { label: "当前使用", disabled: true, onClick: () => undefined } : {
            label: selectingVariantId === previewingVariant.id ? "切换中..." : "使用这张",
            disabled: selectingVariantId === previewingVariant.id,
            onClick: () => { setPreviewingVariantId(null); onSelect(previewingVariant); },
          }}
          onClose={() => setPreviewingVariantId(null)}
        />
      )}
    </>
  );
}
