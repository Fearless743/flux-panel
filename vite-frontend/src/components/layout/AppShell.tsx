import React, { useEffect, useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import {
  LayoutDashboard, ArrowLeftRight, Network, Server, Activity,
  Gauge, Users, Settings, Search, ChevronsLeft, ChevronsRight,
  LogOut, KeyRound, Command, Home, Zap,
} from "lucide-react";
import { clsx } from "clsx";
import { siteConfig } from "@/config/site";
import { safeLogout } from "@/utils/logout";
import { updatePassword } from "@/api";
import { toast } from "react-hot-toast";
import { Modal, Input, Field, Button } from "@/components/ui";

export interface NavItem { path: string; label: string; icon: React.ReactNode; adminOnly?: boolean; keywords?: string }

export const NAV: NavItem[] = [
  { path: "/dashboard", label: "总览", icon: <LayoutDashboard size={17} />, keywords: "dashboard 仪表板 首页 流量" },
  { path: "/forward", label: "端口转发", icon: <ArrowLeftRight size={17} />, keywords: "forward 转发 端口" },
  { path: "/tunnel", label: "隧道", icon: <Network size={17} />, keywords: "tunnel 隧道 拓扑", adminOnly: true },
  { path: "/node", label: "节点", icon: <Server size={17} />, keywords: "node 节点 服务器", adminOnly: true },
  { path: "/node-status", label: "运行状态", icon: <Activity size={17} />, keywords: "status 状态 监控 cpu", adminOnly: true },
  { path: "/limit", label: "限速", icon: <Gauge size={17} />, keywords: "limit 限速 速度", adminOnly: true },
  { path: "/user", label: "用户", icon: <Users size={17} />, keywords: "user 用户 账号", adminOnly: true },
  { path: "/config", label: "系统配置", icon: <Settings size={17} />, keywords: "config 配置 备份 网站", adminOnly: true },
];

function useIsAdmin() {
  const [admin, setAdmin] = useState(false);
  useEffect(() => {
    let flag = localStorage.getItem("admin") === "true";
    if (localStorage.getItem("admin") === null) {
      flag = parseInt(localStorage.getItem("role_id") || "1", 10) === 0;
      localStorage.setItem("admin", String(flag));
    }
    setAdmin(flag);
  }, []);
  return admin;
}

export function isAdminUser() {
  if (typeof window === "undefined") return false;
  if (localStorage.getItem("admin") !== null) return localStorage.getItem("admin") === "true";
  return parseInt(localStorage.getItem("role_id") || "1", 10) === 0;
}

/* ---------- Command palette ---------- */
function CommandPalette({ open, onClose }: { open: boolean; onClose: () => void }) {
  const navigate = useNavigate();
  const isAdmin = useIsAdmin();
  const [q, setQ] = useState("");
  const [active, setActive] = useState(0);

  const items = useMemo(() => {
    const kw = q.trim().toLowerCase();
    const navs = NAV.filter((n) => !n.adminOnly || isAdmin).filter(
      (n) => !kw || n.label.toLowerCase().includes(kw) || n.path.includes(kw) || (n.keywords ?? "").toLowerCase().includes(kw)
    );
    const actions = [
      { label: "修改密码", icon: <KeyRound size={15} />, run: () => window.dispatchEvent(new CustomEvent("flux:change-password")) },
      { label: "退出登录", icon: <LogOut size={15} />, run: () => { safeLogout(); navigate("/"); } },
    ].filter((a) => !kw || a.label.includes(q.trim()));
    return [
      ...navs.map((n) => ({ label: `前往 · ${n.label}`, hint: n.path, icon: n.icon, run: () => navigate(n.path) })),
      ...actions.map((a) => ({ ...a, hint: "操作" })),
    ];
  }, [q, isAdmin, navigate]);

  useEffect(() => {
    if (open) { setQ(""); setActive(0); }
  }, [open ]);
  useEffect(() => setActive(0), [q]);

  useEffect(() => {
    if (!open) return;
    const fn = (e: KeyboardEvent) => {
      if (e.key === "ArrowDown") { e.preventDefault(); setActive((a) => Math.min(a + 1, items.length - 1)); }
      if (e.key === "ArrowUp") { e.preventDefault(); setActive((a) => Math.max(a - 1, 0)); }
      if (e.key === "Enter") { items[active]?.run(); onClose(); }
    };
    window.addEventListener("keydown", fn);
    return () => window.removeEventListener("keydown", fn);
  }, [open, items, active, onClose]);

  if (!open) return null;
  return (
    <div className="modal-overlay !items-start pt-[14vh]" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal-panel glass w-full max-w-xl !rounded-2xl overflow-hidden">
        <div className="flex items-center gap-2.5 border-b border-white/[0.07] px-4 py-3">
          <Search size={16} className="text-slate-500" />
          <input
            autoFocus
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="搜索页面、操作… (↑↓ 选择, 回车跳转)"
            className="w-full bg-transparent text-sm text-white placeholder:text-slate-500 outline-none"
          />
          <kbd className="cmd-kbd">ESC</kbd>
        </div>
        <div className="max-h-[46vh] overflow-y-auto p-2">
          {items.length === 0 && <p className="px-3 py-6 text-center text-sm text-slate-500">无匹配结果</p>}
          {items.map((it, i) => (
            <button
              key={i}
              onMouseEnter={() => setActive(i)}
              onClick={() => { it.run(); onClose(); }}
              className={clsx(
                "flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-sm transition-colors",
                i === active ? "bg-blue-400/10 text-white" : "text-slate-300"
              )}
            >
              <span className={clsx(i === active ? "text-blue-300" : "text-slate-500")}>{it.icon}</span>
              <span className="flex-1">{it.label}</span>
              <span className="mono text-[0.7rem] text-slate-500">{it.hint}</span>
            </button>
          ))}
        </div>
        <div className="flex items-center gap-4 border-t border-white/[0.07] px-4 py-2.5 text-[0.72rem] text-slate-500">
          <span className="flex items-center gap-1.5"><Command size={12} /> 命令面板</span>
          <span>Flux Panel · {NAV.length} 个模块</span>
        </div>
      </div>
    </div>
  );
}

/* ---------- Shell ---------- */
export default function AppShell({ children }: { children: React.ReactNode }) {
  const navigate = useNavigate();
  const location = useLocation();
  const isAdmin = useIsAdmin();
  const [collapsed, setCollapsed] = useState(() => localStorage.getItem("flux:nav-collapsed") === "1");
  const [mobileNav, setMobileNav] = useState(false);
  const [cmdOpen, setCmdOpen] = useState(false);
  const [pwdOpen, setPwdOpen] = useState(false);
  const [pwdLoading, setPwdLoading] = useState(false);
  const [username] = useState(() => localStorage.getItem("name") || "Admin");
  const [pwdForm, setPwdForm] = useState({ newUsername: "", currentPassword: "", newPassword: "", confirmPassword: "" });
  const [userMenu, setUserMenu] = useState(false);
  const isMobile = typeof window !== "undefined" && window.innerWidth <= 768;

  const visibleNav = NAV.filter((n) => !n.adminOnly || isAdmin);
  const current = NAV.find((n) => location.pathname === n.path) ?? NAV[0];

  useEffect(() => {
    localStorage.setItem("flux:nav-collapsed", collapsed ? "1" : "0");
  }, [collapsed]);

  useEffect(() => {
    const open = () => setPwdOpen(true);
    const key = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") { e.preventDefault(); setCmdOpen((v) => !v); }
    };
    window.addEventListener("flux:change-password", open);
    window.addEventListener("keydown", key);
    return () => { window.removeEventListener("flux:change-password", open); window.removeEventListener("keydown", key); };
  }, []);

  useEffect(() => setMobileNav(false), [location.pathname]);
  useEffect(() => { setUserMenu(false); }, [location.pathname]);

  const submitPwd = async () => {
    if (!pwdForm.newUsername.trim() || pwdForm.newUsername.length < 3) return toast.error("请输入至少 3 位新用户名");
    if (!pwdForm.currentPassword) return toast.error("请输入当前密码");
    if (pwdForm.newPassword.length < 6) return toast.error("新密码至少 6 位");
    if (pwdForm.newPassword !== pwdForm.confirmPassword) return toast.error("两次密码不一致");
    setPwdLoading(true);
    try {
      const r = await updatePassword(pwdForm);
      if (r.code === 0) { toast.success("修改成功，请重新登录"); setPwdOpen(false); safeLogout(); navigate("/"); }
      else toast.error(r.msg || "修改失败");
    } catch { toast.error("请求失败"); }
    finally { setPwdLoading(false); }
  };

  const go = (p: string) => navigate(p);

  return (
    <div className="min-h-screen text-slate-200">
      <div className="app-bg" />

      {/* desktop sidebar */}
      <aside className={clsx(
        "fixed inset-y-0 left-0 z-40 hidden flex-col border-r border-white/[0.07] bg-[#0e1218] backdrop-blur-xl transition-all duration-300 md:flex",
        collapsed ? "w-[4.6rem]" : "w-60"
      )}>
        <div className="flex h-16 items-center gap-2.5 px-4">
          <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-blue-600">
            <Zap size={18} className="text-white" />
          </div>
          {!collapsed && (
            <div className="min-w-0 flex-1">
              <p className="truncate text-[0.9rem] font-bold tracking-tight text-white">{siteConfig.name}</p>
              <p className="mono text-[0.68rem] text-slate-500">v{siteConfig.version} · FLUX</p>
            </div>
          )}
        </div>

        {!collapsed && (
          <button
            onClick={() => setCmdOpen(true)}
            className="mx-3 mb-2 flex items-center gap-2 rounded-xl border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-[0.78rem] text-slate-500 transition-colors hover:border-blue-400/30 hover:text-slate-300"
          >
            <Search size={14} />
            <span className="flex-1 text-left">搜索…</span>
            <kbd className="cmd-kbd">⌘K</kbd>
          </button>
        )}

        <nav className="flex-1 space-y-1 overflow-y-auto px-3 py-2">
          {!collapsed && <p className="px-2 pb-1.5 text-[0.66rem] font-semibold uppercase tracking-[0.14em] text-slate-600">Console</p>}
          {visibleNav.map((n) => (
            <button key={n.path} onClick={() => go(n.path)} title={collapsed ? n.label : undefined}
              className={clsx("navlink", location.pathname === n.path && "active", collapsed && "justify-center px-0")}>
              {n.icon}
              {!collapsed && <span>{n.label}</span>}
              {!collapsed && location.pathname === n.path && (
                <span className="ml-auto h-1.5 w-1.5 rounded-full bg-blue-300" />
              )}
            </button>
          ))}
        </nav>

        <div className="border-t border-white/[0.07] p-3">
          {!collapsed ? (
            <div className="rounded-xl border border-white/[0.07] bg-white/[0.03] p-3">
              <div className="flex items-center gap-2.5">
                <div className="flex h-8 w-8 items-center justify-center rounded-full bg-blue-500/15 text-sm font-bold text-blue-200">
                  {(username || "A").slice(0, 1).toUpperCase()}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-[0.82rem] font-medium text-white">{username}</p>
                  <p className="text-[0.7rem] text-slate-500">{isAdmin ? "Administrator" : "Member"}</p>
                </div>
              </div>
              <div className="mt-2.5 grid grid-cols-2 gap-1.5">
                <button onClick={() => setPwdOpen(true)} className="rounded-lg bg-white/[0.05] py-1.5 text-[0.72rem] text-slate-300 hover:bg-white/[0.09]">改密</button>
                <button onClick={() => { safeLogout(); navigate("/"); }} className="rounded-lg bg-red-500/10 py-1.5 text-[0.72rem] text-red-300 hover:bg-red-500/20">退出</button>
              </div>
            </div>
          ) : (
            <div className="flex flex-col items-center gap-2">
              <div className="flex h-9 w-9 items-center justify-center rounded-full bg-blue-500/15 text-sm font-bold text-blue-200">
                {(username || "A").slice(0, 1).toUpperCase()}
              </div>
            </div>
          )}
          <button onClick={() => setCollapsed((v) => !v)}
            className="mt-2 flex w-full items-center justify-center gap-1 rounded-lg py-1.5 text-slate-500 hover:bg-white/[0.05] hover:text-slate-300">
            {collapsed ? <ChevronsRight size={15} /> : <><ChevronsLeft size={15} /><span className="text-[0.72rem]">收起</span></>}
          </button>
        </div>
      </aside>

      {/* mobile top bar */}
      <header className="safe-top fixed inset-x-0 top-0 z-40 border-b border-white/[0.07] bg-[#0b0e13] backdrop-blur-xl md:hidden">
        <div className="flex h-14 items-center gap-2.5 px-4">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-blue-600">
            <Zap size={15} className="text-white" />
          </div>
          <p className="flex-1 truncate text-[0.92rem] font-bold text-white">{current.label}</p>
          <button onClick={() => setCmdOpen(true)} className="rounded-lg border border-white/10 bg-white/[0.04] p-2 text-slate-400" aria-label="搜索">
            <Search size={16} />
          </button>
          <button onClick={() => { setUserMenu((v) => !v); }} className="flex h-8 w-8 items-center justify-center rounded-full bg-blue-500/15 text-xs font-bold text-blue-200">
            {(username || "A").slice(0, 1).toUpperCase()}
          </button>
        </div>
        {userMenu && (
          <div className="border-t border-white/[0.07] px-4 py-2.5">
            <p className="text-xs text-slate-400">{username} · {isAdmin ? "管理员" : "普通用户"}</p>
            <div className="mt-2 flex gap-2">
              <Button size="sm" variant="outline" onClick={() => { setUserMenu(false); setPwdOpen(true); }}>修改密码</Button>
              <Button size="sm" variant="danger" onClick={() => { safeLogout(); navigate("/"); }}>退出登录</Button>
            </div>
          </div>
        )}
      </header>

      {/* content */}
      <div className={clsx("transition-all duration-300", collapsed ? "md:pl-[4.6rem]" : "md:pl-60")}>
        {/* desktop topbar */}
        <div className="sticky top-0 z-30 hidden border-b border-white/[0.06] bg-[#0b0e13] backdrop-blur-xl md:block">
          <div className="flex h-16 items-center gap-3 px-6">
            <nav className="flex items-center gap-1.5 text-[0.8rem] text-slate-500">
              <Home size={13} />
              <span>/</span>
              <span className="text-slate-200 font-medium">{current.label}</span>
            </nav>
            <div className="flex-1" />
            <button onClick={() => setCmdOpen(true)}
              className="hidden w-64 items-center gap-2 rounded-xl border border-white/[0.08] bg-white/[0.03] px-3 py-2 text-[0.78rem] text-slate-500 transition-colors hover:border-blue-400/30 lg:flex">
              <Search size={14} />
              <span className="flex-1 text-left">跳转页面 / 操作…</span>
              <kbd className="cmd-kbd">⌘K</kbd>
            </button>
          </div>
        </div>

        <main className="mx-auto w-full max-w-[1200px] px-4 pb-28 pt-20 md:px-6 md:pb-12 md:pt-6">
          <div key={location.pathname} className="page-enter">
            {children}
          </div>
        </main>
      </div>

      {/* mobile bottom tab */}
      {!isMobile ? null : (
        <nav className="safe-bottom fixed inset-x-0 bottom-0 z-40 border-t border-white/[0.08] bg-[#0e1218] backdrop-blur-xl md:hidden">
          <div className="grid" style={{ gridTemplateColumns: `repeat(${Math.min(visibleNav.length + 1, 5)}, 1fr)` }}>
            {visibleNav.slice(0, 4).map((n) => {
              const active = location.pathname === n.path;
              return (
                <button key={n.path} onClick={() => go(n.path)} className="flex flex-col items-center gap-1 py-2.5">
                  <span className={clsx(
                    "flex h-8 w-12 items-center justify-center rounded-full transition-all",
                    active ? "bg-blue-400/15 text-blue-300" : "text-slate-500"
                  )}>{n.icon}</span>
                  <span className={clsx("text-[0.64rem] font-medium", active ? "text-blue-200" : "text-slate-500")}>{n.label}</span>
                </button>
              );
            })}
            <button onClick={() => setMobileNav(true)} className="flex flex-col items-center gap-1 py-2.5">
              <span className="flex h-8 w-12 items-center justify-center rounded-full text-slate-500">
                <Command size={17} />
              </span>
              <span className="text-[0.64rem] text-slate-500">更多</span>
            </button>
          </div>
        </nav>
      )}

      {/* mobile more sheet */}
      {mobileNav && (
        <div className="modal-overlay !items-end !p-0 md:hidden" onMouseDown={(e) => e.target === e.currentTarget && setMobileNav(false)}>
          <div className="modal-panel glass w-full !rounded-b-none !rounded-t-3xl p-4 pb-8">
            <div className="mx-auto mb-3 h-1 w-10 rounded-full bg-white/15" />
            <div className="grid grid-cols-4 gap-2">
              {visibleNav.map((n) => (
                <button key={n.path} onClick={() => go(n.path)}
                  className={clsx("flex flex-col items-center gap-1.5 rounded-2xl border p-3.5",
                    location.pathname === n.path ? "border-blue-400/30 bg-blue-400/10 text-blue-200" : "border-white/[0.07] bg-white/[0.03] text-slate-300")}>
                  {n.icon}
                  <span className="text-[0.7rem]">{n.label}</span>
                </button>
              ))}
            </div>
            <div className="mt-3 grid grid-cols-2 gap-2">
              <Button variant="outline" onClick={() => { setMobileNav(false); setPwdOpen(true); }}><KeyRound size={14} /> 修改密码</Button>
              <Button variant="danger" onClick={() => { safeLogout(); navigate("/"); }}><LogOut size={14} /> 退出登录</Button>
            </div>
          </div>
        </div>
      )}

      <CommandPalette open={cmdOpen} onClose={() => setCmdOpen(false)} />

      <Modal open={pwdOpen} onClose={() => setPwdOpen(false)} title="修改密码" desc="修改后将退出并重新登录"
        footer={<><Button variant="ghost" onClick={() => setPwdOpen(false)}>取消</Button><Button variant="primary" loading={pwdLoading} onClick={submitPwd}>确认修改</Button></>}>
        <div className="space-y-3.5">
          <Field label="新用户名"><Input value={pwdForm.newUsername} onChange={(e) => setPwdForm({ ...pwdForm, newUsername: e.target.value })} placeholder="至少 3 位" /></Field>
          <Field label="当前密码"><Input type="password" value={pwdForm.currentPassword} onChange={(e) => setPwdForm({ ...pwdForm, currentPassword: e.target.value })} placeholder="请输入当前密码" /></Field>
          <div className="grid grid-cols-1 gap-3.5 sm:grid-cols-2">
            <Field label="新密码"><Input type="password" value={pwdForm.newPassword} onChange={(e) => setPwdForm({ ...pwdForm, newPassword: e.target.value })} placeholder="至少 6 位" /></Field>
            <Field label="确认密码"><Input type="password" value={pwdForm.confirmPassword} onChange={(e) => setPwdForm({ ...pwdForm, confirmPassword: e.target.value })} placeholder="再次输入" /></Field>
          </div>
        </div>
      </Modal>
    </div>
  );
}
