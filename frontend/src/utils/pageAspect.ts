import type { PageAspectRatio } from "../types/domain";

export const PAGE_ASPECT_OPTIONS: Array<{
  value: PageAspectRatio;
  label: string;
  hint: string;
  cssRatio: string;
}> = [
  {
    value: "portrait_4_5",
    label: "竖版 4:5",
    hint: "推荐，适合绘本和 PDF",
    cssRatio: "4 / 5",
  },
  {
    value: "landscape_16_9",
    label: "横版 16:9",
    hint: "适合课堂大屏",
    cssRatio: "16 / 9",
  },
  {
    value: "square_1_1",
    label: "方形 1:1",
    hint: "适合卡片式故事",
    cssRatio: "1 / 1",
  },
];

export function pageAspectLabel(value: PageAspectRatio | string | undefined) {
  return PAGE_ASPECT_OPTIONS.find((option) => option.value === value)?.label || "竖版 4:5";
}

export function pageAspectCssRatio(value: PageAspectRatio | string | undefined) {
  return PAGE_ASPECT_OPTIONS.find((option) => option.value === value)?.cssRatio || "4 / 5";
}
