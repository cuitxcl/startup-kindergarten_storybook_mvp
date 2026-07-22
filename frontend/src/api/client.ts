import type {
  AuditLog,
  ChildProfile,
  Classroom,
  MarketplaceSubmission,
  MarketplaceTemplate,
  ParentIntake,
  ParentIntakeLink,
  PublicParentIntakeLink,
  Storybook,
  StorybookPage,
  StorybookRole,
  User,
  Workspace,
  WorkspaceInvitation,
  WorkspaceMember,
} from "../types/domain";

type Envelope<T> = {
  data: T;
  meta?: {
    total: number;
    limit: number;
    offset: number;
    has_more: boolean;
  };
};

export type PaginationMeta = NonNullable<Envelope<unknown>["meta"]>;

export type PaginatedResult<T> = {
  data: T[];
  meta: PaginationMeta;
};

type ErrorEnvelope = {
  error?: {
    code?: string;
    message?: string;
    field?: string | null;
  };
};

export class ApiClientError extends Error {
  status: number;
  code: string;
  field?: string;
  payload: unknown;

  constructor(status: number, code: string, message: string, field: string | undefined, payload: unknown) {
    super(message);
    this.name = "ApiClientError";
    this.status = status;
    this.code = code;
    this.field = field;
    this.payload = payload;
  }
}

export function isApiClientError(error: unknown): error is ApiClientError {
  return error instanceof ApiClientError;
}

type ApiUser = {
  id: string;
  display_name: string;
  email: string;
};

type ApiWorkspace = {
  id: string;
  name: string;
  type: Workspace["type"];
  role: Workspace["role"];
  description: string;
};

type ApiWorkspaceMember = {
  id: string;
  workspace_id: string;
  name: string;
  email: string;
  role: WorkspaceMember["role"];
  status: WorkspaceMember["status"];
  classes: string[];
  invitation_token?: string | null;
  invitation_url?: string | null;
};

type ApiWorkspaceInvitation = {
  token: string;
  workspace_id: string;
  workspace_name: string;
  invited_by: string;
  invited_contact: string;
  role: "school_teacher";
  classrooms: string[];
  status: "pending" | "invited" | "accepted" | "active" | "expired" | "revoked";
};

type ApiClassroom = {
  id: string;
  workspace_id: string;
  name: string;
  age_group: string;
  teachers: number;
  children: number;
  status: Classroom["status"];
};

type ApiStorybookPage = {
  id: string;
  page_number: number;
  title: string;
  body: string;
  illustration_prompt: string;
  status: StorybookPage["status"];
};

type ApiStorybookRole = {
  id: string;
  name: string;
  role_type: StorybookRole["roleType"];
  appearance: string;
  story_function: string;
  needs_consistency: boolean;
};

type ApiStorybook = {
  id: string;
  workspace_id: string;
  title: string;
  type: Storybook["type"];
  status: Storybook["status"];
  visibility: Storybook["visibility"];
  source: string;
  source_title?: string | null;
  target_child_id?: string | null;
  creator_name: string;
  updated_at: string;
  age_group: string;
  use_scene: string;
  teaching_goal: string;
  cover_tone: string;
  pages: ApiStorybookPage[];
  roles: ApiStorybookRole[];
};

type ApiExportJob = {
  id: string;
  storybook_id: string;
  status: string;
  file_url?: string | null;
  last_error?: string | null;
  created_at: string;
  finished_at?: string | null;
};

type ApiChildProfile = {
  id: string;
  workspace_id: string;
  nickname: string;
  age_group: string;
  classroom?: string | null;
  interests: string[];
  traits: string[];
  focus: string;
  completeness: number;
  status?: "active" | "archived";
  updated_at: string;
};

type ApiParentIntake = {
  id: string;
  workspace_id: string;
  child_nickname: string;
  age_group: string;
  classroom?: string | null;
  interests: string[];
  status: ParentIntake["status"];
  confirmed_child_id?: string | null;
  created_at: string;
  updated_at: string;
};

type ApiParentIntakeLink = {
  id: string;
  workspace_id: string;
  token: string;
  label: string;
  classroom?: string | null;
  status: ParentIntakeLink["status"];
  url: string;
  expires_at?: string | null;
  access_count: number;
  last_accessed_at?: string | null;
  created_at: string;
  updated_at: string;
};

type ApiPublicParentIntakeLink = {
  token: string;
  workspace_id: string;
  workspace_name: string;
  label: string;
  classroom?: string | null;
  status: PublicParentIntakeLink["status"];
  expires_at?: string | null;
};

type ApiSubmission = {
  id: string;
  workspace_id: string;
  title: string;
  source_storybook_title: string;
  submitted_by: string;
  status: MarketplaceSubmission["status"];
  privacy_confirmed: boolean;
  updated_at: string;
};

type ApiMarketplaceTemplate = {
  id: string;
  title: string;
  summary: string;
  source_type: MarketplaceTemplate["sourceType"];
  source_label: string;
  source_storybook_id?: string | null;
  age_group: string;
  use_scene: string;
  page_count: number;
  supports_customization: boolean;
  tags: string[];
};

type ApiAuditLog = {
  id: string;
  workspace_id?: string | null;
  actor_user_id?: string | null;
  actor_name?: string | null;
  action: string;
  resource_type: string;
  resource_id?: string | null;
  metadata_json: Record<string, unknown>;
  created_at: string;
};

type ApiGenerationProvider = {
  provider: string;
  mode: string;
  schema_version: string;
  requires_api_key: boolean;
  supports_text: string[];
  supports_image: string[];
  real_text_ready: boolean;
  real_image_ready: boolean;
  production_ready: boolean;
  missing_configuration: string[];
  components?: ApiGenerationProviderComponent[];
  diagnostic: string;
};

type ApiGenerationProviderComponent = {
  kind: string;
  provider: string;
  configured: boolean;
  ready: boolean;
  model: string;
  endpoint: string;
  supports: string[];
  required_configuration: string[];
};

type ApiStorageStatus = {
  backend: string;
  exports_dir: string;
  generated_images_dir: string;
  export_max_bytes: number;
  generated_image_max_bytes: number;
  filename_validation: boolean;
  size_limit_enabled: boolean;
  download_strategy: string;
  public_direct_access: boolean;
};

type ApiReadinessCheck = {
  key: string;
  label: string;
  ok: boolean;
  message: string;
};

type ApiOperatorReadiness = {
  ready: boolean;
  mode: string;
  checks: ApiReadinessCheck[];
  provider: ApiGenerationProvider;
  storage: ApiStorageStatus;
};

type ApiGenerationJob = {
  id: string;
  workspace_id: string;
  storybook_id?: string | null;
  job_type: string;
  status: string;
  input_json: unknown;
  output_json?: unknown;
  attempt_count: number;
  last_error?: string | null;
  next_run_at?: string | null;
  locked_by?: string | null;
  locked_at?: string | null;
  created_at: string;
  finished_at?: string | null;
};

type ApiGenerationCostLog = {
  id: string;
  workspace_id: string;
  workspace_name?: string | null;
  generation_job_id: string;
  storybook_id?: string | null;
  storybook_title?: string | null;
  provider: string;
  job_type: string;
  status: string;
  estimated_input_units: number;
  estimated_output_units: number;
  image_count: number;
  estimated_cost_micros: number;
  currency: string;
  metadata_json: Record<string, unknown>;
  created_at: string;
};

type ApiGenerationCostReport = {
  summary: {
    total_cost_micros: number;
    succeeded_cost_micros: number;
    failed_jobs: number;
    total_jobs: number;
    total_input_units: number;
    total_output_units: number;
    total_images: number;
    currency: string;
    budget_limit_micros?: number | null;
    budget_used_percent?: number | null;
    budget_warning_percent?: number | null;
    budget_warning?: boolean;
    budget_exceeded?: boolean;
  };
  items: ApiGenerationCostLog[];
};

type LoginResponse = {
  token: string;
  user: ApiUser;
  workspaces: ApiWorkspace[];
};

type DashboardResponse = {
  workspace: ApiWorkspace;
  storybooks: ApiStorybook[];
  children: ApiChildProfile[];
  submissions: ApiSubmission[];
};

export type DashboardData = {
  workspace: Workspace;
  storybooks: Storybook[];
  children: ChildProfile[];
  submissions: MarketplaceSubmission[];
};

export type ExportJob = {
  id: string;
  storybookId: string;
  status: string;
  fileUrl?: string;
  lastError?: string;
  createdAt: string;
  finishedAt?: string;
};

export type ShareLink = {
  id: string;
  storybookId: string;
  token: string;
  url: string;
  status: string;
  accessCount: number;
  lastAccessedAt?: string;
  expiresAt?: string;
};

export type ActionResponse = {
  status: string;
  message: string;
};

export type GenerationProviderStatus = {
  provider: string;
  mode: string;
  schemaVersion: string;
  requiresApiKey: boolean;
  supportsText: string[];
  supportsImage: string[];
  realTextReady: boolean;
  realImageReady: boolean;
  productionReady: boolean;
  missingConfiguration: string[];
  components: GenerationProviderComponent[];
  diagnostic: string;
};

export type StorageStatus = {
  backend: string;
  exportsDir: string;
  generatedImagesDir: string;
  exportMaxBytes: number;
  generatedImageMaxBytes: number;
  filenameValidation: boolean;
  sizeLimitEnabled: boolean;
  downloadStrategy: string;
  publicDirectAccess: boolean;
};

export type ReadinessCheck = {
  key: string;
  label: string;
  ok: boolean;
  message: string;
};

export type OperatorReadiness = {
  ready: boolean;
  mode: string;
  checks: ReadinessCheck[];
  provider: GenerationProviderStatus;
  storage: StorageStatus;
};

export type GenerationProviderComponent = {
  kind: string;
  provider: string;
  configured: boolean;
  ready: boolean;
  model: string;
  endpoint: string;
  supports: string[];
  requiredConfiguration: string[];
};

export type GenerationJob = {
  id: string;
  workspaceId: string;
  storybookId?: string;
  jobType: string;
  status: string;
  input: unknown;
  output?: unknown;
  attemptCount: number;
  lastError?: string;
  nextRunAt?: string;
  lockedBy?: string;
  lockedAt?: string;
  createdAt: string;
  finishedAt?: string;
};

export type GenerationCostLog = {
  id: string;
  workspaceId: string;
  workspaceName?: string;
  generationJobId: string;
  storybookId?: string;
  storybookTitle?: string;
  provider: string;
  jobType: string;
  status: string;
  estimatedInputUnits: number;
  estimatedOutputUnits: number;
  imageCount: number;
  estimatedCostMicros: number;
  currency: string;
  metadata: Record<string, unknown>;
  createdAt: string;
};

export type GenerationCostReport = {
  summary: {
    totalCostMicros: number;
    succeededCostMicros: number;
    failedJobs: number;
    totalJobs: number;
    totalInputUnits: number;
    totalOutputUnits: number;
    totalImages: number;
    currency: string;
    budgetLimitMicros?: number;
    budgetUsedPercent?: number;
    budgetWarningPercent?: number;
    budgetWarning: boolean;
    budgetExceeded: boolean;
  };
  items: GenerationCostLog[];
};

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || "http://127.0.0.1:8080";
const envUseApi = import.meta.env.VITE_USE_API;
const envModeLocked = envUseApi === "true" || envUseApi === "false";
export let shouldUseApi = envUseApi === "true" || envUseApi === undefined;

export function apiResourceUrl(pathOrUrl?: string | null) {
  if (!pathOrUrl) return undefined;
  if (/^https?:\/\//i.test(pathOrUrl)) return pathOrUrl;
  if (pathOrUrl.startsWith("/")) return `${API_BASE_URL}${pathOrUrl}`;
  return pathOrUrl;
}

function token() {
  const storedToken = localStorage.getItem("kindleaf_token");
  if (storedToken) return storedToken;
  return envModeLocked ? null : "dev-token";
}

async function requestEnvelope<T>(path: string, init: RequestInit = {}): Promise<Envelope<T>> {
  const authToken = token();
  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(authToken ? { Authorization: `Bearer ${authToken}` } : {}),
      ...init.headers,
    },
  });
  const payload = await response.json().catch(() => null);
  if (!response.ok) {
    const errorPayload = payload as ErrorEnvelope | null;
    const message = errorPayload?.error?.message || "请求失败，请稍后重试";
    const code = errorPayload?.error?.code || "request_failed";
    const field = errorPayload?.error?.field || undefined;
    throw new ApiClientError(response.status, code, message, field, payload);
  }
  return payload as Envelope<T>;
}

async function request<T>(path: string, init: RequestInit = {}) {
  return (await requestEnvelope<T>(path, init)).data;
}

async function requestBlob(path: string, init: RequestInit = {}) {
  const authToken = token();
  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers: {
      ...(authToken ? { Authorization: `Bearer ${authToken}` } : {}),
      ...init.headers,
    },
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null);
    const errorPayload = payload as ErrorEnvelope | null;
    const message = errorPayload?.error?.message || "文件下载失败，请稍后重试";
    const code = errorPayload?.error?.code || "download_failed";
    const field = errorPayload?.error?.field || undefined;
    throw new ApiClientError(response.status, code, message, field, payload);
  }
  return response.blob();
}

function queryString(query: Record<string, string | number | boolean | undefined>) {
  const params = new URLSearchParams();
  Object.entries(query).forEach(([key, value]) => {
    if (value !== undefined && value !== "") {
      params.set(key, String(value));
    }
  });
  const value = params.toString();
  return value ? `?${value}` : "";
}

function pageMeta<T>(envelope: Envelope<T[]>): PaginationMeta {
  return envelope.meta || {
    total: envelope.data.length,
    limit: envelope.data.length,
    offset: 0,
    has_more: false,
  };
}

export async function initApiMode() {
  if (envModeLocked) {
    shouldUseApi = envUseApi === "true";
    return shouldUseApi;
  }

  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 1200);
  try {
    const response = await fetch(`${API_BASE_URL}/api/health`, { signal: controller.signal });
    shouldUseApi = response.ok;
  } catch {
    shouldUseApi = false;
  } finally {
    window.clearTimeout(timeout);
  }
  return shouldUseApi;
}

export async function login(identifier: string, password: string) {
  const response = await request<LoginResponse>("/api/auth/login", {
    method: "POST",
    body: JSON.stringify({ identifier, password }),
  });
  localStorage.setItem("kindleaf_token", response.token);
  return {
    token: response.token,
    user: mapUser(response.user),
    workspaces: response.workspaces.map(mapWorkspace),
  };
}

export async function register(displayName: string, email: string, password: string) {
  const response = await request<LoginResponse>("/api/auth/register", {
    method: "POST",
    body: JSON.stringify({
      display_name: displayName,
      email,
      password,
    }),
  });
  localStorage.setItem("kindleaf_token", response.token);
  return {
    token: response.token,
    user: mapUser(response.user),
    workspaces: response.workspaces.map(mapWorkspace),
  };
}

export async function currentSession() {
  const response = await request<LoginResponse>("/api/auth/me");
  localStorage.setItem("kindleaf_token", response.token);
  return {
    token: response.token,
    user: mapUser(response.user),
    workspaces: response.workspaces.map(mapWorkspace),
  };
}

export async function dashboard(workspaceId: string): Promise<DashboardData> {
  const response = await request<DashboardResponse>(`/api/workspaces/${workspaceId}/dashboard`);
  return {
    workspace: mapWorkspace(response.workspace),
    storybooks: response.storybooks.map(mapStorybook),
    children: response.children.map(mapChild),
    submissions: response.submissions.map(mapSubmission),
  };
}

export async function listMembersPage(
  workspaceId: string,
  query: { limit?: number; offset?: number } = {},
): Promise<PaginatedResult<WorkspaceMember>> {
  const suffix = queryString({
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiWorkspaceMember[]>(`/api/workspaces/${workspaceId}/members${suffix}`);
  return {
    data: envelope.data.map(mapMember),
    meta: pageMeta(envelope),
  };
}

export async function listMembers(workspaceId: string) {
  return (await listMembersPage(workspaceId)).data;
}

export async function getInvitation(token: string) {
  const response = await request<ApiWorkspaceInvitation>(`/api/invitations/${token}`);
  return mapInvitation(response);
}

export async function acceptInvitation(token: string) {
  const response = await request<ApiWorkspaceInvitation>(`/api/invitations/${token}/accept`, {
    method: "POST",
  });
  return mapInvitation(response);
}

export async function createMember(
  workspaceId: string,
  payload: { name: string; email: string; classes: string[] },
) {
  const response = await request<ApiWorkspaceMember>(`/api/workspaces/${workspaceId}/members`, {
    method: "POST",
    body: JSON.stringify(payload),
  });
  return mapMember(response);
}

export async function revokeMemberInvitation(workspaceId: string, memberId: string) {
  const response = await request<ApiWorkspaceMember>(`/api/workspaces/${workspaceId}/members/${memberId}/revoke-invitation`, {
    method: "POST",
  });
  return mapMember(response);
}

export async function listClassroomsPage(
  workspaceId: string,
  query: { limit?: number; offset?: number } = {},
): Promise<PaginatedResult<Classroom>> {
  const suffix = queryString({
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiClassroom[]>(`/api/workspaces/${workspaceId}/classes${suffix}`);
  return {
    data: envelope.data.map(mapClassroom),
    meta: pageMeta(envelope),
  };
}

export async function listClassrooms(workspaceId: string) {
  return (await listClassroomsPage(workspaceId)).data;
}

export async function listAuditLogsPage(
  workspaceId: string,
  query: { limit?: number; offset?: number } = {},
): Promise<PaginatedResult<AuditLog>> {
  const suffix = queryString({
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiAuditLog[]>(`/api/workspaces/${workspaceId}/audit-logs${suffix}`);
  return {
    data: envelope.data.map(mapAuditLog),
    meta: pageMeta(envelope),
  };
}

export async function listAuditLogs(workspaceId: string) {
  return (await listAuditLogsPage(workspaceId)).data;
}

export async function createClassroom(workspaceId: string, payload: { name: string; ageGroup: string }) {
  const response = await request<ApiClassroom>(`/api/workspaces/${workspaceId}/classes`, {
    method: "POST",
    body: JSON.stringify({
      name: payload.name,
      age_group: payload.ageGroup,
    }),
  });
  return mapClassroom(response);
}

export async function archiveClassroom(workspaceId: string, classroomId: string) {
  const response = await request<ApiClassroom>(`/api/workspaces/${workspaceId}/classes/${classroomId}/archive`, {
    method: "POST",
  });
  return mapClassroom(response);
}

export async function listStorybooksPage(
  workspaceId: string,
  query: {
    type?: Storybook["type"];
    status?: Storybook["status"];
    targetChildId?: string;
    q?: string;
    limit?: number;
    offset?: number;
  } = {},
): Promise<PaginatedResult<Storybook>> {
  const suffix = queryString({
    type: query.type,
    status: query.status,
    target_child_id: query.targetChildId,
    q: query.q,
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiStorybook[]>(`/api/workspaces/${workspaceId}/storybooks${suffix}`);
  return {
    data: envelope.data.map(mapStorybook),
    meta: pageMeta(envelope),
  };
}

export async function listStorybooks(workspaceId: string) {
  return (await listStorybooksPage(workspaceId)).data;
}

export async function createStorybook(
  workspaceId: string,
  payload: {
    title: string;
    ageGroup: string;
    useScene: string;
    teachingGoal: string;
  },
) {
  const response = await request<ApiStorybook>(`/api/workspaces/${workspaceId}/storybooks`, {
    method: "POST",
    body: JSON.stringify({
      title: payload.title,
      age_group: payload.ageGroup,
      use_scene: payload.useScene,
      teaching_goal: payload.teachingGoal,
    }),
  });
  return mapStorybook(response);
}

export async function getStorybook(workspaceId: string, storybookId: string) {
  const response = await request<ApiStorybook>(`/api/workspaces/${workspaceId}/storybooks/${storybookId}`);
  return mapStorybook(response);
}

export async function updateStorybook(
  workspaceId: string,
  storybookId: string,
  payload: {
    title?: string;
    status?: Storybook["status"];
    visibility?: Storybook["visibility"];
    ageGroup?: string;
    useScene?: string;
    teachingGoal?: string;
    coverTone?: string;
  },
) {
  const response = await request<ApiStorybook>(`/api/workspaces/${workspaceId}/storybooks/${storybookId}`, {
    method: "PATCH",
    body: JSON.stringify({
      title: payload.title,
      status: payload.status,
      visibility: payload.visibility,
      age_group: payload.ageGroup,
      use_scene: payload.useScene,
      teaching_goal: payload.teachingGoal,
      cover_tone: payload.coverTone,
    }),
  });
  return mapStorybook(response);
}

export async function duplicateStorybook(workspaceId: string, storybookId: string) {
  const response = await request<ApiStorybook>(`/api/workspaces/${workspaceId}/storybooks/${storybookId}/duplicate`, {
    method: "POST",
  });
  return mapStorybook(response);
}

export async function updateStorybookPage(
  workspaceId: string,
  storybookId: string,
  pageId: string,
  payload: { title?: string; body?: string; illustrationPrompt?: string; status?: string },
) {
  const response = await request<ApiStorybookPage>(
    `/api/workspaces/${workspaceId}/storybooks/${storybookId}/pages/${pageId}`,
    {
      method: "PATCH",
      body: JSON.stringify({
        title: payload.title,
        body: payload.body,
        illustration_prompt: payload.illustrationPrompt,
        status: payload.status,
      }),
    },
  );
  return mapStorybookPage(response);
}

export async function updateStorybookRole(
  workspaceId: string,
  storybookId: string,
  roleId: string,
  payload: Partial<Pick<StorybookRole, "name" | "roleType" | "appearance" | "storyFunction" | "needsConsistency">>,
) {
  const response = await request<ApiStorybookRole>(
    `/api/workspaces/${workspaceId}/storybooks/${storybookId}/roles/${roleId}`,
    {
      method: "PATCH",
      body: JSON.stringify({
        name: payload.name,
        role_type: payload.roleType,
        appearance: payload.appearance,
        story_function: payload.storyFunction,
        needs_consistency: payload.needsConsistency,
      }),
    },
  );
  return mapStorybookRole(response);
}

export async function createPageImageTask(
  workspaceId: string,
  storybookId: string,
  pageId: string,
  payload: { prompt?: string },
): Promise<GenerationJob> {
  const response = await request<ApiGenerationJob>(
    `/api/workspaces/${workspaceId}/storybooks/${storybookId}/pages/${pageId}/image-tasks`,
    {
      method: "POST",
      body: JSON.stringify({ prompt: payload.prompt }),
    },
  );
  return mapGenerationJob(response);
}

export async function downloadGenerationImageFile(workspaceId: string, jobId: string): Promise<Blob> {
  return requestBlob(`/api/workspaces/${workspaceId}/generation-jobs/${jobId}/image`);
}

export async function createGenerationJob(
  workspaceId: string,
  payload: { jobType: string; storybookId?: string; input?: unknown },
): Promise<GenerationJob> {
  const response = await request<ApiGenerationJob>(`/api/workspaces/${workspaceId}/generation-jobs`, {
    method: "POST",
    body: JSON.stringify({
      job_type: payload.jobType,
      storybook_id: payload.storybookId,
      input_json: payload.input || {},
    }),
  });
  return mapGenerationJob(response);
}

export async function listGenerationJobsPage(
  workspaceId: string,
  query: { storybookId?: string; limit?: number; offset?: number } = {},
): Promise<PaginatedResult<GenerationJob>> {
  const suffix = queryString({
    storybook_id: query.storybookId,
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiGenerationJob[]>(`/api/workspaces/${workspaceId}/generation-jobs${suffix}`);
  return {
    data: envelope.data.map(mapGenerationJob),
    meta: pageMeta(envelope),
  };
}

export async function listGenerationJobs(workspaceId: string): Promise<GenerationJob[]> {
  return (await listGenerationJobsPage(workspaceId)).data;
}

export async function getGenerationJob(workspaceId: string, jobId: string): Promise<GenerationJob> {
  const response = await request<ApiGenerationJob>(`/api/workspaces/${workspaceId}/generation-jobs/${jobId}`);
  return mapGenerationJob(response);
}

export async function listStorybookGenerationJobs(
  workspaceId: string,
  storybookId: string,
  query: { limit?: number; offset?: number } = {},
): Promise<GenerationJob[]> {
  return (await listGenerationJobsPage(workspaceId, {
    storybookId,
    limit: query.limit ?? 8,
    offset: query.offset ?? 0,
  })).data;
}

export async function retryGenerationJob(workspaceId: string, jobId: string): Promise<GenerationJob> {
  const response = await request<ApiGenerationJob>(`/api/workspaces/${workspaceId}/generation-jobs/${jobId}/retry`, {
    method: "POST",
  });
  return mapGenerationJob(response);
}

export async function cancelGenerationJob(workspaceId: string, jobId: string): Promise<GenerationJob> {
  const response = await request<ApiGenerationJob>(`/api/workspaces/${workspaceId}/generation-jobs/${jobId}/cancel`, {
    method: "POST",
  });
  return mapGenerationJob(response);
}

export async function recoverGenerationJobs(
  workspaceId: string,
  payload: { ageMinutes?: number; limit?: number } = {},
): Promise<{ status: string; processed: number; message: string }> {
  return request<{ status: string; processed: number; message: string }>(`/api/workspaces/${workspaceId}/generation-jobs/recover`, {
    method: "POST",
    body: JSON.stringify({
      age_minutes: payload.ageMinutes,
      limit: payload.limit,
    }),
  });
}

function mapGenerationJob(response: ApiGenerationJob): GenerationJob {
  return {
    id: response.id,
    workspaceId: response.workspace_id,
    storybookId: response.storybook_id || undefined,
    jobType: response.job_type,
    status: response.status,
    input: response.input_json,
    output: response.output_json,
    attemptCount: response.attempt_count,
    lastError: response.last_error || undefined,
    nextRunAt: response.next_run_at || undefined,
    lockedBy: response.locked_by || undefined,
    lockedAt: response.locked_at || undefined,
    createdAt: response.created_at,
    finishedAt: response.finished_at || undefined,
  };
}

export async function createStorybookExport(workspaceId: string, storybookId: string): Promise<ExportJob> {
  const response = await request<ApiExportJob>(`/api/workspaces/${workspaceId}/storybooks/${storybookId}/exports`, { method: "POST" });
  return mapExportJob(response);
}

export async function getStorybookExport(workspaceId: string, storybookId: string, exportId: string): Promise<ExportJob> {
  const response = await request<ApiExportJob>(`/api/workspaces/${workspaceId}/storybooks/${storybookId}/exports/${exportId}`);
  return mapExportJob(response);
}

export async function downloadStorybookExportFile(workspaceId: string, storybookId: string, exportId: string): Promise<Blob> {
  return requestBlob(`/api/workspaces/${workspaceId}/storybooks/${storybookId}/exports/${exportId}/download`);
}

export async function listStorybookExports(workspaceId: string, storybookId: string): Promise<ExportJob[]> {
  return (await listStorybookExportsPage(workspaceId, storybookId)).data;
}

export async function listStorybookExportsPage(
  workspaceId: string,
  storybookId: string,
  query: { limit?: number; offset?: number } = {},
): Promise<PaginatedResult<ExportJob>> {
  const suffix = queryString({
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiExportJob[]>(`/api/workspaces/${workspaceId}/storybooks/${storybookId}/exports${suffix}`);
  return {
    data: envelope.data.map(mapExportJob),
    meta: pageMeta(envelope),
  };
}

export async function createShareExport(token: string): Promise<ExportJob> {
  const response = await request<ApiExportJob>(`/api/share-links/${token}/exports`, { method: "POST" });
  return mapExportJob(response);
}

export async function getShareExport(token: string, exportId: string): Promise<ExportJob> {
  const response = await request<ApiExportJob>(`/api/share-links/${token}/exports/${exportId}`);
  return mapExportJob(response);
}

export async function downloadShareExportFile(token: string, exportId: string): Promise<Blob> {
  return requestBlob(`/api/share-links/${token}/exports/${exportId}/download`);
}

export async function createShareLink(
  workspaceId: string,
  storybookId: string,
  payload: { expiresAt?: string } = {},
): Promise<ShareLink> {
  const response = await request<{
    id: string;
    storybook_id: string;
    token: string;
    url: string;
    status: string;
    access_count: number;
    last_accessed_at?: string | null;
    expires_at?: string | null;
  }>(`/api/workspaces/${workspaceId}/storybooks/${storybookId}/share-links`, {
    method: "POST",
    body: JSON.stringify({
      expires_at: payload.expiresAt,
    }),
  });
  return {
    id: response.id,
    storybookId: response.storybook_id,
    token: response.token,
    url: response.url,
    status: response.status,
    accessCount: response.access_count,
    lastAccessedAt: response.last_accessed_at || undefined,
    expiresAt: response.expires_at || undefined,
  };
}

export async function listShareLinks(workspaceId: string, storybookId: string): Promise<ShareLink[]> {
  return (await listShareLinksPage(workspaceId, storybookId)).data;
}

export async function listShareLinksPage(
  workspaceId: string,
  storybookId: string,
  query: { limit?: number; offset?: number } = {},
): Promise<PaginatedResult<ShareLink>> {
  const suffix = queryString({
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<
    {
      id: string;
      storybook_id: string;
      token: string;
      url: string;
      status: string;
      access_count: number;
      last_accessed_at?: string | null;
      expires_at?: string | null;
    }[]
  >(`/api/workspaces/${workspaceId}/storybooks/${storybookId}/share-links${suffix}`);
  return {
    data: envelope.data.map((item) => ({
      id: item.id,
      storybookId: item.storybook_id,
      token: item.token,
      url: item.url,
      status: item.status,
      accessCount: item.access_count,
      lastAccessedAt: item.last_accessed_at || undefined,
      expiresAt: item.expires_at || undefined,
    })),
    meta: pageMeta(envelope),
  };
}

export async function revokeShareLink(
  workspaceId: string,
  storybookId: string,
  shareLinkId: string,
): Promise<ShareLink> {
  const response = await request<{
    id: string;
    storybook_id: string;
    token: string;
    url: string;
    status: string;
    access_count: number;
    last_accessed_at?: string | null;
    expires_at?: string | null;
  }>(`/api/workspaces/${workspaceId}/storybooks/${storybookId}/share-links/${shareLinkId}/revoke`, { method: "POST" });
  return {
    id: response.id,
    storybookId: response.storybook_id,
    token: response.token,
    url: response.url,
    status: response.status,
    accessCount: response.access_count,
    lastAccessedAt: response.last_accessed_at || undefined,
    expiresAt: response.expires_at || undefined,
  };
}

export async function listChildrenPage(
  workspaceId: string,
  query: { limit?: number; offset?: number } = {},
): Promise<PaginatedResult<ChildProfile>> {
  const suffix = queryString({
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiChildProfile[]>(`/api/workspaces/${workspaceId}/children${suffix}`);
  return {
    data: envelope.data.map(mapChild),
    meta: pageMeta(envelope),
  };
}

export async function listChildren(workspaceId: string) {
  return (await listChildrenPage(workspaceId)).data;
}

export async function getChild(workspaceId: string, childId: string) {
  const response = await request<ApiChildProfile>(`/api/workspaces/${workspaceId}/children/${childId}`);
  return mapChild(response);
}

export async function createChild(
  workspaceId: string,
  payload: {
    nickname: string;
    ageGroup: string;
    classroom?: string;
    interests: string[];
    traits: string[];
    focus: string;
  },
) {
  const response = await request<ApiChildProfile>(`/api/workspaces/${workspaceId}/children`, {
    method: "POST",
    body: JSON.stringify({
      nickname: payload.nickname,
      age_group: payload.ageGroup,
      classroom: payload.classroom,
      interests: payload.interests,
      traits: payload.traits,
      focus: payload.focus,
    }),
  });
  return mapChild(response);
}

export async function updateChild(
  workspaceId: string,
  childId: string,
  payload: {
    nickname?: string;
    ageGroup?: string;
    classroom?: string;
    interests?: string[];
    traits?: string[];
    focus?: string;
  },
) {
  const response = await request<ApiChildProfile>(`/api/workspaces/${workspaceId}/children/${childId}`, {
    method: "PATCH",
    body: JSON.stringify({
      nickname: payload.nickname,
      age_group: payload.ageGroup,
      classroom: payload.classroom,
      interests: payload.interests,
      traits: payload.traits,
      focus: payload.focus,
    }),
  });
  return mapChild(response);
}

export async function archiveChild(workspaceId: string, childId: string) {
  const response = await request<ApiChildProfile>(`/api/workspaces/${workspaceId}/children/${childId}/archive`, {
    method: "POST",
  });
  return mapChild(response);
}

export async function restoreChild(workspaceId: string, childId: string) {
  const response = await request<ApiChildProfile>(`/api/workspaces/${workspaceId}/children/${childId}/restore`, {
    method: "POST",
  });
  return mapChild(response);
}

export async function listParentIntakesPage(
  workspaceId: string,
  query: { classroom?: string; limit?: number; offset?: number } = {},
): Promise<PaginatedResult<ParentIntake>> {
  const suffix = queryString({
    classroom: query.classroom,
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiParentIntake[]>(`/api/workspaces/${workspaceId}/parent-intakes${suffix}`);
  return {
    data: envelope.data.map(mapParentIntake),
    meta: pageMeta(envelope),
  };
}

export async function listParentIntakes(workspaceId: string) {
  return (await listParentIntakesPage(workspaceId)).data;
}

export async function listParentIntakeLinksPage(
  workspaceId: string,
  query: { status?: string; classroom?: string; limit?: number; offset?: number } = {},
): Promise<PaginatedResult<ParentIntakeLink>> {
  const suffix = queryString({
    status: query.status,
    classroom: query.classroom,
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiParentIntakeLink[]>(`/api/workspaces/${workspaceId}/parent-intake-links${suffix}`);
  return {
    data: envelope.data.map(mapParentIntakeLink),
    meta: pageMeta(envelope),
  };
}

export async function listParentIntakeLinks(workspaceId: string) {
  return (await listParentIntakeLinksPage(workspaceId)).data;
}

export async function createParentIntakeLink(workspaceId: string, payload: { label?: string; classroom?: string; expiresAt?: string } = {}) {
  const response = await request<ApiParentIntakeLink>(`/api/workspaces/${workspaceId}/parent-intake-links`, {
    method: "POST",
    body: JSON.stringify({
      label: payload.label,
      classroom: payload.classroom,
      expires_at: payload.expiresAt,
    }),
  });
  return mapParentIntakeLink(response);
}

export async function revokeParentIntakeLink(workspaceId: string, linkId: string) {
  const response = await request<ApiParentIntakeLink>(`/api/workspaces/${workspaceId}/parent-intake-links/${linkId}/revoke`, {
    method: "POST",
  });
  return mapParentIntakeLink(response);
}

export async function revokeActiveParentIntakeLinks(workspaceId: string, query: { classroom?: string } = {}) {
  const suffix = queryString({ classroom: query.classroom });
  return request<ActionResponse>(`/api/workspaces/${workspaceId}/parent-intake-links/revoke-active${suffix}`, {
    method: "POST",
  });
}

export async function getPublicParentIntakeLink(token: string) {
  const response = await request<ApiPublicParentIntakeLink>(`/api/parent-intake-links/${token}`);
  return mapPublicParentIntakeLink(response);
}

export async function confirmParentIntake(
  workspaceId: string,
  intakeId: string,
  payload: { focus?: string; traits?: string[] } = {},
) {
  const response = await request<ApiChildProfile>(
    `/api/workspaces/${workspaceId}/parent-intakes/${intakeId}/confirm`,
    {
      method: "POST",
      body: JSON.stringify({
        focus: payload.focus,
        traits: payload.traits || [],
      }),
    },
  );
  return mapChild(response);
}

export async function deriveCustomStorybook(
  workspaceId: string,
  storybookId: string,
  payload: { childId: string; intensity: "quick" | "standard"; customizationPlan?: unknown },
) {
  const response = await request<ApiStorybook>(`/api/workspaces/${workspaceId}/storybooks/${storybookId}/derive-custom`, {
    method: "POST",
    body: JSON.stringify({
      child_id: payload.childId,
      intensity: payload.intensity,
      customization_plan: payload.customizationPlan,
    }),
  });
  return mapStorybook(response);
}

export async function listMarketplaceTemplatesPage(
  query: { source?: string; q?: string; supportsCustomization?: boolean; limit?: number; offset?: number } = {},
): Promise<PaginatedResult<MarketplaceTemplate>> {
  const suffix = queryString({
    source: query.source,
    q: query.q,
    supports_customization: query.supportsCustomization,
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiMarketplaceTemplate[]>(`/api/marketplace/templates${suffix}`);
  return {
    data: envelope.data.map(mapMarketplaceTemplate),
    meta: pageMeta(envelope),
  };
}

export async function listMarketplaceTemplates(query?: { source?: string; q?: string }) {
  return (await listMarketplaceTemplatesPage(query)).data;
}

export async function getMarketplaceTemplate(templateId: string) {
  const response = await request<ApiMarketplaceTemplate>(`/api/marketplace/templates/${templateId}`);
  return mapMarketplaceTemplate(response);
}

export async function copyMarketplaceTemplate(workspaceId: string, templateId: string) {
  const response = await request<ApiStorybook>(`/api/workspaces/${workspaceId}/marketplace/templates/${templateId}/copy`, {
    method: "POST",
  });
  return mapStorybook(response);
}

export async function listSubmissionsPage(
  workspaceId: string,
  query: { status?: string; limit?: number; offset?: number } = {},
): Promise<PaginatedResult<MarketplaceSubmission>> {
  const suffix = queryString({
    status: query.status,
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiSubmission[]>(`/api/workspaces/${workspaceId}/submissions${suffix}`);
  return {
    data: envelope.data.map(mapSubmission),
    meta: pageMeta(envelope),
  };
}

export async function listSubmissions(workspaceId: string) {
  return (await listSubmissionsPage(workspaceId)).data;
}

export async function createSubmission(workspaceId: string, storybookId: string) {
  const response = await request<ApiSubmission>(`/api/workspaces/${workspaceId}/submissions`, {
    method: "POST",
    body: JSON.stringify({ storybook_id: storybookId }),
  });
  return mapSubmission(response);
}

export async function confirmSubmissionPrivacy(workspaceId: string, submissionId: string) {
  const response = await request<ApiSubmission>(
    `/api/workspaces/${workspaceId}/submissions/${submissionId}/privacy-confirm`,
    { method: "POST" },
  );
  return mapSubmission(response);
}

export async function listOperatorSubmissionsPage(
  query: { status?: string; limit?: number; offset?: number } = {},
): Promise<PaginatedResult<MarketplaceSubmission>> {
  const suffix = queryString({
    status: query.status,
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiSubmission[]>(`/api/operator/submissions${suffix}`);
  return {
    data: envelope.data.map(mapSubmission),
    meta: pageMeta(envelope),
  };
}

export async function listOperatorSubmissions() {
  return (await listOperatorSubmissionsPage()).data;
}

export async function getOperatorGenerationProvider() {
  const response = await request<ApiGenerationProvider>("/api/operator/generation-provider");
  return mapGenerationProvider(response);
}

export async function getOperatorStorage() {
  const response = await request<ApiStorageStatus>("/api/operator/storage");
  return mapStorageStatus(response);
}

export async function getOperatorReadiness() {
  const response = await request<ApiOperatorReadiness>("/api/operator/readiness");
  return mapOperatorReadiness(response);
}

export async function listOperatorGenerationCostsPage(
  query: {
    workspaceId?: string;
    provider?: string;
    jobType?: string;
    status?: string;
    limit?: number;
    offset?: number;
  } = {},
): Promise<{ data: GenerationCostReport; meta: PaginationMeta }> {
  const suffix = queryString({
    workspace_id: query.workspaceId,
    provider: query.provider,
    job_type: query.jobType,
    status: query.status,
    limit: query.limit,
    offset: query.offset,
  });
  const envelope = await requestEnvelope<ApiGenerationCostReport>(`/api/operator/generation-costs${suffix}`);
  return {
    data: mapGenerationCostReport(envelope.data),
    meta: envelope.meta || {
      total: envelope.data.items.length,
      limit: envelope.data.items.length,
      offset: 0,
      has_more: false,
    },
  };
}

export async function getWorkspaceGenerationProvider(workspaceId: string) {
  const response = await request<ApiGenerationProvider>(`/api/workspaces/${workspaceId}/generation-provider`);
  return mapGenerationProvider(response);
}

export async function updateOperatorMarketplaceTemplate(
  templateId: string,
  payload: {
    title?: string;
    summary?: string;
    ageGroup?: string;
    useScene?: string;
    supportsCustomization?: boolean;
    tags?: string[];
  },
) {
  const response = await request<ApiMarketplaceTemplate>(`/api/operator/marketplace/templates/${templateId}`, {
    method: "PATCH",
    body: JSON.stringify({
      title: payload.title,
      summary: payload.summary,
      age_group: payload.ageGroup,
      use_scene: payload.useScene,
      supports_customization: payload.supportsCustomization,
      tags: payload.tags,
    }),
  });
  return mapMarketplaceTemplate(response);
}

export async function approveOperatorSubmission(submissionId: string) {
  const response = await request<ApiMarketplaceTemplate>(`/api/operator/submissions/${submissionId}/approve`, {
    method: "POST",
  });
  return mapMarketplaceTemplate(response);
}

export async function rejectOperatorSubmission(submissionId: string) {
  const response = await request<ApiSubmission>(`/api/operator/submissions/${submissionId}/reject`, {
    method: "POST",
  });
  return mapSubmission(response);
}

export async function getSharedStorybook(token: string) {
  const response = await request<ApiStorybook>(`/api/share-links/${token}`);
  return mapStorybook(response);
}

export async function submitParentIntake(payload: {
  linkToken?: string;
  workspaceId?: string;
  childNickname: string;
  ageGroup: string;
  interests: string[];
}) {
  return request<ActionResponse>("/api/parent-intakes", {
    method: "POST",
    body: JSON.stringify({
      link_token: payload.linkToken,
      workspace_id: payload.workspaceId,
      child_nickname: payload.childNickname,
      age_group: payload.ageGroup,
      interests: payload.interests,
    }),
  });
}

function mapUser(user: ApiUser): User {
  return {
    id: user.id,
    displayName: user.display_name,
    email: user.email,
  };
}

function mapWorkspace(workspace: ApiWorkspace): Workspace {
  return {
    id: workspace.id,
    name: workspace.name,
    type: workspace.type,
    role: workspace.role,
    description: workspace.description,
  };
}

function mapMember(member: ApiWorkspaceMember): WorkspaceMember {
  return {
    id: member.id,
    workspaceId: member.workspace_id,
    name: member.name,
    email: member.email,
    role: member.role,
    status: member.status,
    classes: member.classes,
    invitationToken: member.invitation_token || undefined,
    invitationUrl: member.invitation_url || undefined,
  };
}

function mapInvitation(invitation: ApiWorkspaceInvitation): WorkspaceInvitation {
  return {
    id: invitation.token,
    workspaceId: invitation.workspace_id,
    workspaceName: invitation.workspace_name,
    invitedBy: invitation.invited_by,
    invitedContact: invitation.invited_contact,
    role: invitation.role,
    classrooms: invitation.classrooms,
    status: invitation.status,
  };
}

function mapClassroom(classroom: ApiClassroom): Classroom {
  return {
    id: classroom.id,
    workspaceId: classroom.workspace_id,
    name: classroom.name,
    ageGroup: classroom.age_group,
    teachers: classroom.teachers,
    children: classroom.children,
    status: classroom.status,
  };
}

function mapStorybook(book: ApiStorybook): Storybook {
  return {
    id: book.id,
    workspaceId: book.workspace_id,
    title: book.title,
    type: book.type,
    status: book.status,
    visibility: book.visibility,
    source: book.source.startsWith("derived")
      ? "derived"
      : book.source === "marketplace"
        ? "marketplace"
        : book.source === "duplicate"
          ? "duplicate"
          : "blank",
    sourceTitle: book.source_title || undefined,
    targetChildId: book.target_child_id || undefined,
    creatorName: book.creator_name,
    updatedAt: book.updated_at,
    ageGroup: book.age_group,
    useScene: book.use_scene,
    teachingGoal: book.teaching_goal,
    coverTone: book.cover_tone,
    pages: book.pages.map(mapStorybookPage),
    roles: book.roles.map(mapStorybookRole),
  };
}

function mapStorybookPage(page: ApiStorybookPage): StorybookPage {
  return {
    id: page.id,
    pageNumber: page.page_number,
    title: page.title,
    body: page.body,
    illustrationPrompt: page.illustration_prompt,
    status: page.status,
  };
}

function mapStorybookRole(role: ApiStorybookRole): StorybookRole {
  return {
    id: role.id,
    name: role.name,
    roleType: role.role_type,
    appearance: role.appearance,
    storyFunction: role.story_function,
    needsConsistency: role.needs_consistency,
  };
}

function mapExportJob(job: ApiExportJob): ExportJob {
  return {
    id: job.id,
    storybookId: job.storybook_id,
    status: job.status,
    fileUrl: apiResourceUrl(job.file_url),
    lastError: job.last_error || undefined,
    createdAt: job.created_at,
    finishedAt: job.finished_at || undefined,
  };
}

function mapChild(child: ApiChildProfile): ChildProfile {
  return {
    id: child.id,
    workspaceId: child.workspace_id,
    nickname: child.nickname,
    ageGroup: child.age_group,
    classroom: child.classroom || undefined,
    interests: child.interests,
    traits: child.traits,
    focus: child.focus,
    completeness: child.completeness,
    status: child.status,
    updatedAt: child.updated_at,
  };
}

function mapParentIntake(item: ApiParentIntake): ParentIntake {
  return {
    id: item.id,
    workspaceId: item.workspace_id,
    childNickname: item.child_nickname,
    ageGroup: item.age_group,
    classroom: item.classroom || undefined,
    interests: item.interests,
    status: item.status,
    confirmedChildId: item.confirmed_child_id || undefined,
    createdAt: item.created_at,
    updatedAt: item.updated_at,
  };
}

function mapParentIntakeLink(item: ApiParentIntakeLink): ParentIntakeLink {
  return {
    id: item.id,
    workspaceId: item.workspace_id,
    token: item.token,
    label: item.label,
    classroom: item.classroom || undefined,
    status: item.status,
    url: item.url,
    expiresAt: item.expires_at || undefined,
    accessCount: item.access_count,
    lastAccessedAt: item.last_accessed_at || undefined,
    createdAt: item.created_at,
    updatedAt: item.updated_at,
  };
}

function mapPublicParentIntakeLink(item: ApiPublicParentIntakeLink): PublicParentIntakeLink {
  return {
    token: item.token,
    workspaceId: item.workspace_id,
    workspaceName: item.workspace_name,
    label: item.label,
    classroom: item.classroom || undefined,
    status: item.status,
    expiresAt: item.expires_at || undefined,
  };
}

function mapAuditLog(item: ApiAuditLog): AuditLog {
  return {
    id: item.id,
    workspaceId: item.workspace_id || undefined,
    actorUserId: item.actor_user_id || undefined,
    actorName: item.actor_name || undefined,
    action: item.action,
    resourceType: item.resource_type,
    resourceId: item.resource_id || undefined,
    metadata: item.metadata_json || {},
    createdAt: item.created_at,
  };
}

function mapSubmission(item: ApiSubmission): MarketplaceSubmission {
  return {
    id: item.id,
    workspaceId: item.workspace_id,
    title: item.title,
    sourceStorybookTitle: item.source_storybook_title,
    submittedBy: item.submitted_by,
    status: item.status,
    privacyConfirmed: item.privacy_confirmed,
    updatedAt: item.updated_at,
  };
}

function mapMarketplaceTemplate(template: ApiMarketplaceTemplate): MarketplaceTemplate {
  return {
    id: template.id,
    title: template.title,
    summary: template.summary,
    sourceType: template.source_type,
    sourceLabel: template.source_label,
    sourceStorybookId: template.source_storybook_id || undefined,
    ageGroup: template.age_group,
    useScene: template.use_scene,
    pageCount: template.page_count,
    supportsCustomization: template.supports_customization,
    tags: template.tags,
  };
}

function mapGenerationProvider(provider: ApiGenerationProvider): GenerationProviderStatus {
  return {
    provider: provider.provider,
    mode: provider.mode,
    schemaVersion: provider.schema_version,
    requiresApiKey: provider.requires_api_key,
    supportsText: provider.supports_text,
    supportsImage: provider.supports_image,
    realTextReady: provider.real_text_ready,
    realImageReady: provider.real_image_ready,
    productionReady: provider.production_ready,
    missingConfiguration: provider.missing_configuration,
    components: (provider.components || []).map((component) => ({
      kind: component.kind,
      provider: component.provider,
      configured: component.configured,
      ready: component.ready,
      model: component.model,
      endpoint: component.endpoint,
      supports: component.supports,
      requiredConfiguration: component.required_configuration,
    })),
    diagnostic: provider.diagnostic,
  };
}

function mapStorageStatus(storage: ApiStorageStatus): StorageStatus {
  return {
    backend: storage.backend,
    exportsDir: storage.exports_dir,
    generatedImagesDir: storage.generated_images_dir,
    exportMaxBytes: storage.export_max_bytes,
    generatedImageMaxBytes: storage.generated_image_max_bytes,
    filenameValidation: storage.filename_validation,
    sizeLimitEnabled: storage.size_limit_enabled,
    downloadStrategy: storage.download_strategy,
    publicDirectAccess: storage.public_direct_access,
  };
}

function mapOperatorReadiness(readiness: ApiOperatorReadiness): OperatorReadiness {
  return {
    ready: readiness.ready,
    mode: readiness.mode,
    checks: readiness.checks.map((check) => ({
      key: check.key,
      label: check.label,
      ok: check.ok,
      message: check.message,
    })),
    provider: mapGenerationProvider(readiness.provider),
    storage: mapStorageStatus(readiness.storage),
  };
}

function mapGenerationCostReport(report: ApiGenerationCostReport): GenerationCostReport {
  return {
    summary: {
      totalCostMicros: report.summary.total_cost_micros,
      succeededCostMicros: report.summary.succeeded_cost_micros,
      failedJobs: report.summary.failed_jobs,
      totalJobs: report.summary.total_jobs,
      totalInputUnits: report.summary.total_input_units,
      totalOutputUnits: report.summary.total_output_units,
      totalImages: report.summary.total_images,
      currency: report.summary.currency,
      budgetLimitMicros: report.summary.budget_limit_micros ?? undefined,
      budgetUsedPercent: report.summary.budget_used_percent ?? undefined,
      budgetWarningPercent: report.summary.budget_warning_percent ?? undefined,
      budgetWarning: report.summary.budget_warning ?? false,
      budgetExceeded: report.summary.budget_exceeded ?? false,
    },
    items: report.items.map((item) => ({
      id: item.id,
      workspaceId: item.workspace_id,
      workspaceName: item.workspace_name || undefined,
      generationJobId: item.generation_job_id,
      storybookId: item.storybook_id || undefined,
      storybookTitle: item.storybook_title || undefined,
      provider: item.provider,
      jobType: item.job_type,
      status: item.status,
      estimatedInputUnits: item.estimated_input_units,
      estimatedOutputUnits: item.estimated_output_units,
      imageCount: item.image_count,
      estimatedCostMicros: item.estimated_cost_micros,
      currency: item.currency,
      metadata: item.metadata_json,
      createdAt: item.created_at,
    })),
  };
}
