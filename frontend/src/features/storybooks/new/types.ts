import type { StorybookRole } from "../../../types/domain";

export type EditablePlan = {
  summary: string;
  outlineText: string;
  roleRequirementsText: string;
  reviewPointsText: string;
};

export type EditableRole = {
  id?: string;
  name: string;
  roleType: StorybookRole["roleType"];
  appearance: string;
  storyFunction: string;
  needsConsistency: boolean;
  referenceImagePrompt: string;
  referenceImageUrl?: string;
  referenceStatus?: StorybookRole["referenceStatus"];
};

export type EditablePage = {
  id?: string;
  pageNumber: number;
  title: string;
  body: string;
  illustrationPrompt: string;
};

export type StorybookRequestForm = {
  title: string;
  theme: string;
  ageGroup: string;
  pageCount: string;
  useScene: string;
  style: string;
  storyStyle: string;
  storyFramework: string;
};
