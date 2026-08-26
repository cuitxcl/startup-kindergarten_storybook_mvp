import { BookOpen, Building2, LayoutDashboard, Library, Settings, UsersRound } from "lucide-react";
import { useEffect, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate, useParams } from "react-router-dom";
import { currentSession, isApiClientError } from "../api/client";
import type { User, Workspace } from "../types/domain";
import { appWorkspaces, pathWithWorkspace, pickPrimaryWorkspace, resolveWorkspaceAlias } from "../utils/workspace";

function roleLabel(role: string) {
  return {
    personal_owner: "个人",
    school_teacher: "老师",
    school_admin: "管理员",
    platform_operator: "平台运营",
  }[role] || role;
}

export function AppShell() {
  const { workspaceId } = useParams();
  const navigate = useNavigate();
  const location = useLocation();
  const [availableWorkspaces, setAvailableWorkspaces] = useState<Workspace[]>([]);
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const workspace = resolveWorkspaceAlias(workspaceId, availableWorkspaces) || availableWorkspaces[0];
  const isSchool = workspace?.type === "school";
  const isAdmin = workspace?.role === "school_admin";

  // 仅在切换空间时重新校验会话；同空间内路由跳转复用已有会话，避免整页 loading 闪烁。
  useEffect(() => {
    let mounted = true;
    setLoading(true);
    currentSession()
      .then((session) => {
        if (!mounted) return;
        const productWorkspaces = appWorkspaces(session.workspaces);
        const requestedPlatformWorkspace = session.workspaces.find((item) => item.id === workspaceId && item.type === "platform");
        if (requestedPlatformWorkspace) {
          navigate("/operator/submissions", { replace: true });
          return;
        }
        if (productWorkspaces.length === 0) {
          setAvailableWorkspaces([]);
          setUser(session.user);
          if (session.workspaces.some((item) => item.type === "platform")) {
            navigate("/operator/submissions", { replace: true });
            return;
          }
          setError("当前账号还没有可用空间，请先创建个人空间或接受园所邀请。");
          return;
        }
        setAvailableWorkspaces(productWorkspaces);
        setUser(session.user);
        setError("");
        const resolvedWorkspace = resolveWorkspaceAlias(workspaceId, productWorkspaces);
        if (!resolvedWorkspace) {
          navigate(`/app/${pickPrimaryWorkspace(productWorkspaces).id}/dashboard`, { replace: true });
        } else if (resolvedWorkspace.id !== workspaceId) {
          navigate(`${pathWithWorkspace(location.pathname, resolvedWorkspace.id)}${location.search}`, { replace: true });
        }
      })
      .catch((err) => {
        if (!mounted) return;
        if (isApiClientError(err) && [401, 403].includes(err.status)) {
          localStorage.removeItem("kindleaf_token");
          navigate("/login", { replace: true });
          return;
        }
        setError(err instanceof Error ? err.message : "无法读取账号空间");
      })
      .finally(() => {
        if (mounted) setLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, [navigate, workspaceId]);

  function logout() {
    localStorage.removeItem("kindleaf_token");
    navigate("/login", { replace: true });
  }

  const navItems = [
    { to: "dashboard", label: isSchool ? "园所工作台" : "我的工作台", icon: LayoutDashboard },
    { to: "storybooks", label: isSchool ? "园所绘本" : "我的绘本", icon: BookOpen },
    { to: "children", label: isSchool ? "班级儿童" : "我的孩子", icon: UsersRound },
    { to: "marketplace", label: "绘本市场", icon: Library },
    ...(isAdmin ? [{ to: "admin", label: "园所管理", icon: Settings }] : []),
  ];

  if (loading) {
    return <main className="page-stack shell-loading"><strong>正在读取账号空间...</strong></main>;
  }

  if (error) {
    return <main className="page-stack shell-loading"><strong>{error}</strong></main>;
  }

  if (!workspace || !user) {
    return <main className="page-stack shell-loading"><strong>当前账号没有可用空间。</strong></main>;
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">K</div>
          <div>
            <strong>Kindleaf</strong>
            <span>绘本生成系统</span>
          </div>
        </div>
        <nav className="nav-list">
          {navItems.map((item) => {
            const Icon = item.icon;
            return (
              <NavLink key={item.to} to={item.to} className={({ isActive }) => (isActive ? "active" : "")}>
                <Icon size={18} />
                {item.label}
              </NavLink>
            );
          })}
        </nav>
        <div className="sidebar-note">
          <strong>空间边界</strong>
          <p>{workspace.type === "personal" ? "当前内容仅归属个人空间。" : "当前内容归属园所空间。"}</p>
        </div>
      </aside>

      <main className="main">
        <header className="topbar">
          <div className="workspace-switcher">
            <Building2 size={18} />
            <label>
              <span className="visually-hidden">切换当前空间</span>
              <select
                value={workspace.id}
                onChange={(event) => navigate(`/app/${event.target.value}/dashboard`)}
              >
                {availableWorkspaces.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.name}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <div className="topbar-meta">
            <span>{user.displayName} · {roleLabel(workspace.role)}</span>
            <button className="button secondary" type="button" onClick={logout}>退出登录</button>
          </div>
        </header>
        <Outlet context={{ workspace }} />
      </main>
    </div>
  );
}
