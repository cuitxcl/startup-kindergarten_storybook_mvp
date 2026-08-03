import { ReactNode, Suspense, lazy, useEffect, useState } from "react";
import { Navigate, Route, Routes, useOutletContext } from "react-router-dom";
import { currentSession, isApiClientError } from "./api/client";
import { AppShell } from "./layout/AppShell";
import { HomePage } from "./features/home/HomePage";
import { LoginPage, RegisterPage } from "./features/auth/AuthPages";
import { EmptyState } from "./components/ui";
import { pickPrimaryWorkspace } from "./utils/workspace";
import type { Workspace } from "./types/domain";

// 路由级代码分割：管理、运营、市场、详情等重页面按需加载，首包只保留入口与认证。
const InvitePage = lazy(() => import("./features/auth/InvitePage").then((m) => ({ default: m.InvitePage })));
const DashboardPage = lazy(() => import("./features/dashboard/DashboardPage").then((m) => ({ default: m.DashboardPage })));
const StorybookListPage = lazy(() => import("./features/storybooks/StorybookListPage").then((m) => ({ default: m.StorybookListPage })));
const StorybookDetailPage = lazy(() => import("./features/storybooks/StorybookDetailPage").then((m) => ({ default: m.StorybookDetailPage })));
const NewStorybookPage = lazy(() => import("./features/storybooks/NewStorybookPage").then((m) => ({ default: m.NewStorybookPage })));
const CustomizeStorybookPage = lazy(() => import("./features/storybooks/CustomizeStorybookPage").then((m) => ({ default: m.CustomizeStorybookPage })));
const ChildrenPage = lazy(() => import("./features/children/ChildrenPage").then((m) => ({ default: m.ChildrenPage })));
const ChildDetailPage = lazy(() => import("./features/children/ChildDetailPage").then((m) => ({ default: m.ChildDetailPage })));
const MarketplacePage = lazy(() => import("./features/marketplace/MarketplacePage").then((m) => ({ default: m.MarketplacePage })));
const MarketplaceDetailPage = lazy(() => import("./features/marketplace/MarketplaceDetailPage").then((m) => ({ default: m.MarketplaceDetailPage })));
const AdminPage = lazy(() => import("./features/admin/AdminPage").then((m) => ({ default: m.AdminPage })));
const MembersPage = lazy(() => import("./features/admin/MembersPage").then((m) => ({ default: m.MembersPage })));
const ClassesPage = lazy(() => import("./features/admin/ClassesPage").then((m) => ({ default: m.ClassesPage })));
const SubmissionsPage = lazy(() => import("./features/admin/SubmissionsPage").then((m) => ({ default: m.SubmissionsPage })));
const AuditLogsPage = lazy(() => import("./features/admin/AuditLogsPage").then((m) => ({ default: m.AuditLogsPage })));
const OperatorMarketplacePage = lazy(() => import("./features/admin/OperatorPages").then((m) => ({ default: m.OperatorMarketplacePage })));
const OperatorSubmissionsPage = lazy(() => import("./features/admin/OperatorPages").then((m) => ({ default: m.OperatorSubmissionsPage })));
const IntakeLinkPage = lazy(() => import("./features/links/LinkPages").then((m) => ({ default: m.IntakeLinkPage })));
const ShareLinkPage = lazy(() => import("./features/links/LinkPages").then((m) => ({ default: m.ShareLinkPage })));

function AppRedirect() {
  const [target, setTarget] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;

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
    <Suspense fallback={<main className="page-stack shell-loading"><strong>正在加载页面...</strong></main>}>
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
    </Suspense>
  );
}
