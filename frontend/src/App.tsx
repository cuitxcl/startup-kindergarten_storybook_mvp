import { ReactNode, useEffect, useState } from "react";
import { Navigate, Route, Routes, useOutletContext } from "react-router-dom";
import { currentSession, isApiClientError, shouldUseApi } from "./api/client";
import { AppShell } from "./layout/AppShell";
import { HomePage } from "./features/home/HomePage";
import { LoginPage, RegisterPage } from "./features/auth/AuthPages";
import { InvitePage } from "./features/auth/InvitePage";
import { DashboardPage } from "./features/dashboard/DashboardPage";
import { StorybookListPage } from "./features/storybooks/StorybookListPage";
import { StorybookDetailPage } from "./features/storybooks/StorybookDetailPage";
import { NewStorybookPage } from "./features/storybooks/NewStorybookPage";
import { CustomizeStorybookPage } from "./features/storybooks/CustomizeStorybookPage";
import { ChildrenPage } from "./features/children/ChildrenPage";
import { ChildDetailPage } from "./features/children/ChildDetailPage";
import { MarketplacePage } from "./features/marketplace/MarketplacePage";
import { MarketplaceDetailPage } from "./features/marketplace/MarketplaceDetailPage";
import { AdminPage } from "./features/admin/AdminPage";
import { MembersPage } from "./features/admin/MembersPage";
import { ClassesPage } from "./features/admin/ClassesPage";
import { SubmissionsPage } from "./features/admin/SubmissionsPage";
import { AuditLogsPage } from "./features/admin/AuditLogsPage";
import { OperatorMarketplacePage, OperatorSubmissionsPage } from "./features/admin/OperatorPages";
import { IntakeLinkPage, ShareLinkPage } from "./features/links/LinkPages";
import { EmptyState } from "./components/ui";
import { workspaces } from "./data/mock";
import { pickPrimaryWorkspace } from "./utils/workspace";
import type { Workspace } from "./types/domain";

function AppRedirect() {
  const [target, setTarget] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;
    if (!shouldUseApi) {
      setTarget(`/app/${pickPrimaryWorkspace(workspaces).id}/dashboard`);
      return;
    }

    currentSession()
      .then((session) => {
        if (mounted) {
          setTarget(`/app/${pickPrimaryWorkspace(session.workspaces).id}/dashboard`);
        }
      })
      .catch((err) => {
        if (mounted) {
          if (isApiClientError(err) && [401, 403].includes(err.status)) {
            localStorage.removeItem("kindleaf_token");
          }
          setTarget("/login");
        }
      });

    return () => {
      mounted = false;
    };
  }, []);

  if (!target) {
    return <main className="page-stack shell-loading"><strong>正在进入工作台...</strong></main>;
  }

  return <Navigate to={target} replace />;
}

function AdminOnlyRoute({ children }: { children: ReactNode }) {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  if (workspace.role !== "school_admin") {
    return (
      <EmptyState
        title="需要园所管理员权限"
        copy="当前空间角色不能访问园所管理。请切换到管理员空间，或联系园所管理员处理成员、班级和投稿设置。"
      />
    );
  }
  return children;
}

export default function App() {
  return (
    <Routes>
      <Route path="/" element={<HomePage />} />
      <Route path="/login" element={<LoginPage />} />
      <Route path="/register" element={<RegisterPage />} />
      <Route path="/invite/:token" element={<InvitePage />} />
      <Route path="/link/intake/:token" element={<IntakeLinkPage />} />
      <Route path="/link/share/:token" element={<ShareLinkPage />} />
      <Route path="/operator/marketplace" element={<OperatorMarketplacePage />} />
      <Route path="/operator/submissions" element={<OperatorSubmissionsPage />} />
      <Route path="/app" element={<AppRedirect />} />
      <Route path="/app/:workspaceId" element={<AppShell />}>
        <Route index element={<Navigate to="dashboard" replace />} />
        <Route path="dashboard" element={<DashboardPage />} />
        <Route path="storybooks" element={<StorybookListPage />} />
        <Route path="storybooks/new" element={<NewStorybookPage />} />
        <Route path="storybooks/:storybookId" element={<StorybookDetailPage />} />
        <Route path="storybooks/:storybookId/customize" element={<CustomizeStorybookPage />} />
        <Route path="children" element={<ChildrenPage />} />
        <Route path="children/:childId" element={<ChildDetailPage />} />
        <Route path="marketplace" element={<MarketplacePage />} />
        <Route path="marketplace/:templateId" element={<MarketplaceDetailPage />} />
        <Route path="admin" element={<AdminOnlyRoute><AdminPage /></AdminOnlyRoute>} />
        <Route path="admin/members" element={<AdminOnlyRoute><MembersPage /></AdminOnlyRoute>} />
        <Route path="admin/classes" element={<AdminOnlyRoute><ClassesPage /></AdminOnlyRoute>} />
        <Route path="admin/submissions" element={<AdminOnlyRoute><SubmissionsPage /></AdminOnlyRoute>} />
        <Route path="admin/audit-logs" element={<AdminOnlyRoute><AuditLogsPage /></AdminOnlyRoute>} />
      </Route>
    </Routes>
  );
}
