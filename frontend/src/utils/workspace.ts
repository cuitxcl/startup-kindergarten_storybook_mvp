import type { Workspace } from "../types/domain";

export function appWorkspaces(workspaces: Workspace[]) {
  return workspaces.filter((item) => item.type !== "platform");
}

export function pickPrimaryWorkspace(workspaces: Workspace[]) {
  const candidates = appWorkspaces(workspaces);
  return candidates.find((item) => item.type === "personal") || candidates[0] || workspaces[0];
}

export function resolveWorkspaceAlias(routeId: string | undefined, available: Workspace[]) {
  if (!routeId) return pickPrimaryWorkspace(available);

  const exact = available.find((item) => item.id === routeId);
  if (exact) return exact;

  if (routeId === "personal-1") {
    return available.find((item) => item.type === "personal") || pickPrimaryWorkspace(available);
  }

  const schoolWorkspaces = available.filter((item) => item.type === "school");
  if (routeId === "school-1") {
    return schoolWorkspaces[0] || pickPrimaryWorkspace(available);
  }
  if (routeId === "school-2") {
    return schoolWorkspaces[1] || schoolWorkspaces[0] || pickPrimaryWorkspace(available);
  }

  return undefined;
}

export function pathWithWorkspace(pathname: string, workspaceId: string) {
  const parts = pathname.split("/");
  if (parts[1] === "app") {
    parts[2] = workspaceId;
    return parts.join("/") || `/app/${workspaceId}/dashboard`;
  }
  return `/app/${workspaceId}/dashboard`;
}
