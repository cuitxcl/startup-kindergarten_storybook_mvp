import { FormEvent, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { login, register, shouldUseApi } from "../../api/client";
import { workspaces } from "../../data/mock";
import { pickPrimaryWorkspace } from "../../utils/workspace";

export function LoginPage() {
  const navigate = useNavigate();
  const [identifier, setIdentifier] = useState("lin@example.com");
  const [password, setPassword] = useState("password");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!shouldUseApi) {
      navigate(`/app/${pickPrimaryWorkspace(workspaces).id}/dashboard`);
      return;
    }
    setLoading(true);
    setError("");
    try {
      const session = await login(identifier, password);
      navigate(`/app/${pickPrimaryWorkspace(session.workspaces).id}/dashboard`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "登录失败");
    } finally {
      setLoading(false);
    }
  }

  return (
    <main className="auth-page">
      <form className="auth-panel" onSubmit={submit}>
        <p className="eyebrow">Kindleaf</p>
        <h1>登录绘本工作台</h1>
        <p>进入个人空间或园所空间，继续生成普通绘本、定制绘本和管理市场内容。</p>
        <label>邮箱或手机号<input value={identifier} onChange={(event) => setIdentifier(event.target.value)} /></label>
        <label>密码<input type="password" value={password} onChange={(event) => setPassword(event.target.value)} /></label>
        {error && <p className="form-error">{error}</p>}
        <button className="button primary" type="submit" disabled={loading}>{loading ? "登录中..." : "登录并进入个人空间"}</button>
        <div className="auth-links">
          <Link to="/register">注册新账号</Link>
          {!shouldUseApi && <Link to="/invite/demo-token">我有老师邀请</Link>}
        </div>
      </form>
    </main>
  );
}

export function RegisterPage() {
  const navigate = useNavigate();
  const [displayName, setDisplayName] = useState("新老师");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("password123");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!shouldUseApi) {
      navigate(`/app/${pickPrimaryWorkspace(workspaces).id}/dashboard`);
      return;
    }
    setLoading(true);
    setError("");
    try {
      const session = await register(displayName, email, password);
      navigate(`/app/${pickPrimaryWorkspace(session.workspaces).id}/dashboard`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "注册失败");
    } finally {
      setLoading(false);
    }
  }

  return (
    <main className="auth-page">
      <form className="auth-panel" onSubmit={submit}>
        <p className="eyebrow">创建账号</p>
        <h1>注册账号</h1>
        <p>注册后系统会自动创建个人空间，后续也可以通过邀请加入园所空间。</p>
        <label>显示名称<input required value={displayName} onChange={(event) => setDisplayName(event.target.value)} placeholder="例如：林老师" /></label>
        <label>邮箱<input required type="email" value={email} onChange={(event) => setEmail(event.target.value)} placeholder="name@example.com" /></label>
        <label>密码<input required type="password" value={password} onChange={(event) => setPassword(event.target.value)} placeholder="至少 8 位" /></label>
        {error && <p className="form-error">{error}</p>}
        <button className="button primary" type="submit" disabled={loading}>{loading ? "注册中..." : "注册并进入个人空间"}</button>
        <Link to="/login">已有账号，去登录</Link>
      </form>
    </main>
  );
}
