export type WorkspaceRole =
  | "personal_owner"
  | "school_teacher"
  | "school_admin"
  | "platform_operator";

export type WorkspaceType = "personal" | "school" | "platform";

export type StorybookType = "plain" | "custom";

export type StorybookStatus =
  | "draft"
  | "plan_pending"
  | "roles_pending"
  | "editing"
  | "image_pending"
  | "exportable"
  | "submitted"
  | "listed";

export type Visibility = "private" | "workspace" | "market_submission" | "market_listed";

export interface User {
  id: string;
  displayName: string;
  email: string;
}

export interface Workspace {
  id: string;
  name: string;
  type: WorkspaceType;
  role: WorkspaceRole;
  description: string;
}

export interface WorkspaceMember {
  id: string;
  workspaceId: string;
  name: string;
  email: string;
  role: WorkspaceRole;
  status: "active" | "invited" | "expired" | "revoked";
  classes: string[];
  invitationToken?: string;
  invitationUrl?: string;
}

export interface Classroom {
  id: string;
  workspaceId: string;
  name: string;
  ageGroup: string;
  teachers: number;
  children: number;
  status: "active" | "archived";
}

export interface StorybookPage {
  id: string;
  pageNumber: number;
  title: string;
  body: string;
  illustrationPrompt: string;
  status: "ready" | "needs_regeneration" | "generating";
}

export interface StorybookRole {
  id: string;
  name: string;
  roleType: "protagonist" | "supporting" | "peer" | "teacher" | "prop";
  appearance: string;
  storyFunction: string;
  needsConsistency: boolean;
  referenceImageUrl?: string;
  referenceImagePrompt?: string;
  referenceStatus: "not_started" | "generating" | "ready" | "needs_regeneration" | "failed";
}

export interface Storybook {
  id: string;
  workspaceId: string;
  title: string;
  type: StorybookType;
  status: StorybookStatus;
  visibility: Visibility;
  source: "blank" | "marketplace" | "derived" | "duplicate";
  sourceTitle?: string;
  targetChildId?: string;
  creatorName: string;
  updatedAt: string;
  ageGroup: string;
  useScene: string;
  teachingGoal: string;
  coverTone: string;
  pages: StorybookPage[];
  roles: StorybookRole[];
}

export interface ChildProfile {
  id: string;
  workspaceId: string;
  nickname: string;
  ageGroup: string;
  classroom?: string;
  interests: string[];
  traits: string[];
  focus: string;
  completeness: number;
  status?: "active" | "archived";
  updatedAt: string;
}

export interface ParentIntake {
  id: string;
  workspaceId: string;
  childNickname: string;
  ageGroup: string;
  classroom?: string;
  interests: string[];
  status: "submitted" | "confirmed" | "rejected";
  confirmedChildId?: string;
  createdAt: string;
  updatedAt: string;
}

export interface ParentIntakeLink {
  id: string;
  workspaceId: string;
  token: string;
  label: string;
  classroom?: string;
  status: "active" | "revoked" | "expired";
  url: string;
  expiresAt?: string;
  accessCount: number;
  lastAccessedAt?: string;
  createdAt: string;
  updatedAt: string;
}

export interface PublicParentIntakeLink {
  token: string;
  workspaceId: string;
  workspaceName: string;
  label: string;
  classroom?: string;
  status: "active" | "revoked" | "expired";
  expiresAt?: string;
}

export interface MarketplaceTemplate {
  id: string;
  title: string;
  summary: string;
  sourceType: "platform" | "school_submission";
  sourceLabel: string;
  sourceStorybookId?: string;
  ageGroup: string;
  useScene: string;
  pageCount: number;
  supportsCustomization: boolean;
  tags: string[];
}

export interface WorkspaceInvitation {
  id: string;
  workspaceName: string;
  workspaceId?: string;
  invitedBy: string;
  invitedContact?: string;
  role: "school_teacher";
  classrooms: string[];
  status: "pending" | "invited" | "accepted" | "active" | "expired" | "revoked";
}

export interface MarketplaceSubmission {
  id: string;
  workspaceId: string;
  title: string;
  sourceStorybookTitle: string;
  submittedBy: string;
  status: "draft" | "submitted" | "approved" | "rejected" | "listed" | "unlisted";
  privacyConfirmed: boolean;
  updatedAt: string;
}

export interface AuditLog {
  id: string;
  workspaceId?: string;
  actorUserId?: string;
  actorName?: string;
  action: string;
  resourceType: string;
  resourceId?: string;
  metadata: Record<string, unknown>;
  createdAt: string;
}
