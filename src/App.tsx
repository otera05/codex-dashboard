import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Activity, Archive, ArrowUp, Bell, BellOff, Bot, CircleStop, Clock3, Folder, Gauge, LoaderCircle, LogIn, MessageSquarePlus, MoreHorizontal, PanelLeftClose, PanelLeftOpen, Pencil, RotateCcw, Search, Settings, Sparkles, Unplug, UserRound, X } from "lucide-react";
import { getSession, getSnapshot, interruptTurn, listArchivedSessions, logoutAccount, refreshAccount, sendTurn, startLogin, subscribe } from "./lib/bridge";
import { MessageContent } from "./components/MessageContent";
import { CommandActivity } from "./components/CommandActivity";
import { FileChangeActivity } from "./components/FileChangeActivity";
import { ApprovalCard } from "./components/ApprovalCard";
import { NewSessionDialog } from "./components/NewSessionDialog";
import { SessionActionDialog } from "./components/SessionActionDialog";
import { useSessionSync } from "./hooks/useSessionSync";
import { useSessionListSync } from "./hooks/useSessionListSync";
import { useDesktopNotifications } from "./hooks/useDesktopNotifications";
import { useDashboard } from "./store";
import { filterSessions, type SessionFilters } from "./lib/sessionFilters";
import type { ApprovalRequest, DashboardSnapshot, Session, SessionStatus } from "./types";

const statusLabel: Record<SessionStatus, string> = { working: "Working", waiting: "Waiting", idle: "Idle", error: "Error" };
const sessionFiltersKey = "codex-dashboard.session-filters";
const sidebarCollapsedKey = "codex-dashboard.sidebar-collapsed";
const defaultSessionFilters: SessionFilters = { query: "", status: "all", approvalsOnly: false, sort: "updated-desc" };

function loadSessionFilters(): SessionFilters {
  try {
    const saved = JSON.parse(localStorage.getItem(sessionFiltersKey) ?? "null") as Partial<SessionFilters> | null;
    return saved ? { ...defaultSessionFilters, ...saved, query: "" } : defaultSessionFilters;
  } catch {
    return defaultSessionFilters;
  }
}

function relativeTime(timestamp: number) {
  const seconds = Math.max(1, Math.floor((Date.now() - timestamp) / 1000));
  if (seconds < 60) return "now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  return `${Math.floor(seconds / 3600)}h`;
}

function Sidebar({ collapsed, onToggle, onNewSession, onSessionAction, onArchiveViewChange }: { collapsed: boolean; onToggle: () => void; onNewSession: () => void; onSessionAction: (action: "rename" | "archive" | "restore", session: Session) => void; onArchiveViewChange: (archived: boolean) => void }) {
  const { sessions, archivedSessions, showingArchived, approvals, selectedId, select, account, connected, setSettingsOpen } = useDashboard();
  const [filters, setFilters] = useState(loadSessionFilters);
  const [menuId, setMenuId] = useState<string>();
  const searchRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const closeMenu = () => setMenuId(undefined);
    window.addEventListener("click", closeMenu);
    return () => window.removeEventListener("click", closeMenu);
  }, []);
  useEffect(() => {
    localStorage.setItem(sessionFiltersKey, JSON.stringify({ ...filters, query: "" }));
  }, [filters]);
  useEffect(() => {
    const focusSearch = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, []);
  const displayedSessions = showingArchived ? archivedSessions : sessions;
  const filtered = useMemo(() => filterSessions(displayedSessions, approvals ?? [], filters), [displayedSessions, approvals, filters]);
  const hasFilters = Boolean(filters.query || filters.status !== "all" || filters.approvalsOnly || filters.sort !== "updated-desc");
  const usage = Math.min(100, Math.max(0, account.usedPercent ?? 0));
  return <aside className={`sidebar ${collapsed ? "collapsed" : ""}`}>
    <div className="brand-row"><div className="brand"><span className="brand-mark"><Sparkles size={15} /></span><span>Codex</span></div><button className="icon-button sidebar-toggle" aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"} aria-expanded={!collapsed} onClick={onToggle}>{collapsed ? <PanelLeftOpen size={17} /> : <PanelLeftClose size={17} />}</button></div>
    <button className="new-session" onClick={onNewSession} aria-label="New session"><MessageSquarePlus size={16} /><span>New session</span><kbd>⌘ N</kbd></button>
    <label className="search"><Search size={14} /><input ref={searchRef} value={filters.query} onChange={(event) => setFilters((current) => ({ ...current, query: event.target.value }))} placeholder="Search sessions" aria-label="Search sessions" /><kbd>⌘K</kbd></label>
    <div className="session-filters">
      <select aria-label="Filter by status" value={filters.status} onChange={(event) => setFilters((current) => ({ ...current, status: event.target.value as SessionFilters["status"] }))}><option value="all">All status</option><option value="working">Working</option><option value="waiting">Waiting</option><option value="idle">Idle</option><option value="error">Error</option></select>
      <button className={filters.approvalsOnly ? "active" : ""} aria-pressed={filters.approvalsOnly} onClick={() => setFilters((current) => ({ ...current, approvalsOnly: !current.approvalsOnly }))}><Bell size={11} /> Approval</button>
      <select aria-label="Sort sessions" value={filters.sort} onChange={(event) => setFilters((current) => ({ ...current, sort: event.target.value as SessionFilters["sort"] }))}><option value="updated-desc">Newest</option><option value="updated-asc">Oldest</option><option value="title">Name</option></select>
      {hasFilters && <button className="clear-filters" aria-label="Clear session filters" onClick={() => setFilters(defaultSessionFilters)}><RotateCcw size={12} /></button>}
    </div>
    <div className="session-view-tabs"><button className={!showingArchived ? "active" : ""} onClick={() => onArchiveViewChange(false)}>Active</button><button className={showingArchived ? "active" : ""} onClick={() => onArchiveViewChange(true)}>Archived</button></div>
    <div className="section-title"><span>{showingArchived ? "Archived" : "Sessions"}</span><span>{filtered.length}/{displayedSessions.length}</span></div>
    <nav className="session-list">
      {filtered.map((session) => <div key={session.id} className={`session-item ${selectedId === session.id ? "selected" : ""}`}>
        <button className="session-select" title={collapsed ? `${session.title} — ${statusLabel[session.status]}` : undefined} aria-label={collapsed ? session.title : undefined} onClick={() => { select(session.id); setMenuId(undefined); }}><span className={`status-dot ${session.status}`} /><span className="session-copy"><strong>{session.title}</strong><span>{statusLabel[session.status]} · {relativeTime(session.updatedAt)}</span></span></button>
        <button className="session-more" aria-label={`Session actions for ${session.title}`} onClick={(event) => { event.stopPropagation(); setMenuId((current) => current === session.id ? undefined : session.id); }}><MoreHorizontal size={16} /></button>
        {menuId === session.id && <div className="session-menu" onClick={(event) => event.stopPropagation()}>{showingArchived ? <button onClick={() => { setMenuId(undefined); onSessionAction("restore", session); }}><RotateCcw size={13} /> Restore</button> : <><button onClick={() => { setMenuId(undefined); onSessionAction("rename", session); }}><Pencil size={13} /> Rename</button><button className="archive" onClick={() => { setMenuId(undefined); onSessionAction("archive", session); }}><Archive size={13} /> Archive</button></>}</div>}
      </div>)}
      {!filtered.length && <div className="empty-list">No sessions found</div>}
    </nav>
    <div className="account-card">
      <div className="usage-head"><span><Gauge size={15} /> Codex usage</span><strong>{account.usedPercent == null ? "—" : `${Math.round(usage)}%`}</strong></div>
      <div className="usage-track"><span style={{ width: `${usage}%` }} /></div>
      <div className="usage-meta"><span>{account.plan ?? "Not connected"}</span><span>{account.resetsAt ? `Resets ${relativeReset(account.resetsAt)}` : "Usage unavailable"}</span></div>
      <button className="account-button" onClick={() => setSettingsOpen(true)}><span className="avatar"><UserRound size={15} /></span><span><strong>{account.email ?? "Connect ChatGPT"}</strong><small>{connected ? "App Server connected" : "Preview mode"}</small></span><Settings size={16} /></button>
    </div>
  </aside>;
}

function relativeReset(timestamp: number) {
  const minutes = Math.max(0, Math.floor((timestamp - Date.now()) / 60000));
  return minutes >= 60 ? `in ${Math.floor(minutes / 60)}h ${minutes % 60}m` : `in ${minutes}m`;
}

const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

function Workspace({ session, approvals, archived, loading, error }: { session?: Session; approvals: ApprovalRequest[]; archived: boolean; loading: boolean; error?: string }) {
  const [draft, setDraft] = useState("");
  const conversationRef = useRef<HTMLElement>(null);
  const followLatest = useRef(true);
  const messageVersion = session?.messages.map((item) => {
    if (item.type === "message") return `${item.id}:${item.text.length}`;
    if (item.type === "command") return `${item.id}:${item.status}:${item.output?.length ?? 0}`;
    return `${item.id}:${item.status}:${item.changes.map((change) => `${change.path}:${change.diff.length}`).join(",")}`;
  }).join("|");

  useEffect(() => {
    const conversation = conversationRef.current;
    if (conversation && followLatest.current) conversation.scrollTop = conversation.scrollHeight;
  }, [session?.id, messageVersion]);

  if (!session) return <main className="workspace empty"><Bot size={34} /><h2>Select a session</h2><p>Choose a Codex session from the sidebar.</p></main>;
  const submit = async () => {
    const value = draft.trim();
    if (!value) return;
    setDraft("");
    await sendTurn(session.id, value);
  };
  return <main className="workspace">
    <header className="workspace-header">
      <div><div className="title-line"><h1>{session.title}</h1><span className={`status-pill ${session.status}`}><i />{statusLabel[session.status]}</span></div><p><Folder size={13} /> {session.cwd}</p></div>
      <div className="header-actions"><button className="subtle-button"><Activity size={15} /> Activity</button><button className="icon-button"><MoreHorizontal size={18} /></button></div>
    </header>
    <section className="conversation" ref={conversationRef} onScroll={(event) => {
      const element = event.currentTarget;
      followLatest.current = element.scrollHeight - element.scrollTop - element.clientHeight < 80;
    }}>
      <div className="session-intro"><div className="intro-icon"><Bot size={20} /></div><div><h2>{session.title}</h2><p>Started with {session.model}</p></div></div>
      {archived && <div className="archived-notice"><Archive size={14} /> Archived sessions are read-only. Restore this session to continue working.</div>}
      {approvals.map((approval) => <ApprovalCard approval={approval} key={String(approval.requestId)} />)}
      {loading ? <div className="history-state"><LoaderCircle size={19} /> Loading session history…</div> : error ? <div className="history-state error">{error}</div> : session.messages.length ? session.messages.map((item) => item.type === "command" ? <CommandActivity key={item.id} item={item} /> : item.type === "fileChange" ? <FileChangeActivity key={item.id} item={item} /> : <article className={`message ${item.role}`} key={item.id}>
        <div className="message-avatar">{item.role === "assistant" ? <Sparkles size={15} /> : <UserRound size={15} />}</div>
        <div className="message-body"><div className="message-meta"><strong>{item.role === "assistant" ? "Codex" : "You"}</strong><span>{relativeTime(item.createdAt)}</span></div><MessageContent text={item.text} />{item.streaming && <span className="cursor" />}</div>
      </article>) : <div className="no-messages">This session has no messages yet.</div>}
    </section>
    {!archived && <footer className="composer-wrap">
      {session.status === "working" && session.activeTurnId && <div className="running-bar"><span><span className="pulse" /> Codex is working</span><button onClick={() => interruptTurn(session.id, session.activeTurnId!)}><CircleStop size={14} /> Stop</button></div>}
      <div className="composer"><textarea rows={2} value={draft} onChange={(event) => setDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); void submit(); } }} placeholder="Message Codex…" /><div className="composer-bottom"><span>↵ send · ⇧↵ new line</span><button disabled={!draft.trim()} onClick={() => void submit()}><ArrowUp size={17} /></button></div></div>
      <div className="token-summary"><span>{session.model}</span><span>{(session.tokenUsage.input + session.tokenUsage.output).toLocaleString()} tokens</span><span>{session.tokenUsage.cached.toLocaleString()} cached</span></div>
    </footer>}
  </main>;
}

function DisconnectAccountDialog({ onClose, onDisconnected }: { onClose: () => void; onDisconnected: () => void }) {
  const { applyEvent } = useDashboard();
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string>();

  const disconnect = async () => {
    if (submitting) return;
    setSubmitting(true);
    setError(undefined);
    try {
      const account = await logoutAccount();
      applyEvent({ type: "account.updated", account });
      onDisconnected();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setSubmitting(false);
    }
  };

  return <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && !submitting) onClose(); }}>
    <section className="session-action-dialog" role="dialog" aria-modal="true" aria-labelledby="disconnect-account-title">
      <div className="dialog-heading"><span><Unplug size={17} /></span><div><h2 id="disconnect-account-title">Disconnect ChatGPT</h2><p>Remove local Codex authentication</p></div><button type="button" aria-label="Close disconnect account dialog" onClick={onClose} disabled={submitting}><X size={17} /></button></div>
      <div className="session-action-content"><p>Disconnect the current ChatGPT account?</p><span>Codex will remove its local authentication. API keys already created by Codex may need to be revoked from the OpenAI dashboard.</span>{error && <div className="dialog-error">{error}</div>}</div>
      <div className="dialog-actions"><button type="button" onClick={onClose} disabled={submitting}>Cancel</button><button className="dialog-danger" type="button" onClick={() => void disconnect()} disabled={submitting}>{submitting && <LoaderCircle size={14} />}{submitting ? "Disconnecting…" : "Disconnect"}</button></div>
    </section>
  </div>;
}

function SettingsView({ notificationsEnabled, notificationError, onNotificationsChange, onDisconnectAccount }: { notificationsEnabled: boolean; notificationError?: string; onNotificationsChange: (enabled: boolean) => Promise<boolean>; onDisconnectAccount: () => void }) {
  const { account, connected, setSettingsOpen, applyEvent } = useDashboard();
  const [loginState, setLoginState] = useState<"idle" | "starting" | "waiting">("idle");
  const [loginError, setLoginError] = useState<string>();
  const [loginMessage, setLoginMessage] = useState<string>();
  const loginAttempt = useRef(0);
  useEffect(() => () => { loginAttempt.current += 1; }, []);
  const login = async () => {
    const attempt = loginAttempt.current + 1;
    loginAttempt.current = attempt;
    const isCurrent = () => loginAttempt.current === attempt;
    setLoginState("starting");
    setLoginError(undefined);
    setLoginMessage(undefined);
    try {
      await startLogin();
      if (!isCurrent()) return;
      setLoginState("waiting");
      setLoginMessage("Complete the ChatGPT sign-in in your browser.");
      const deadline = Date.now() + 120_000;
      while (Date.now() < deadline) {
        const refreshed = await refreshAccount();
        applyEvent({ type: "account.updated", account: refreshed });
        if (!isCurrent()) return;
        if (refreshed.connected) {
          setLoginMessage("ChatGPT account connected.");
          setLoginState("idle");
          return;
        }
        await sleep(2_000);
        if (!isCurrent()) return;
      }
      setLoginMessage("Still waiting for ChatGPT sign-in to complete.");
      setLoginState("idle");
    } catch (cause) {
      if (!isCurrent()) return;
      setLoginError(cause instanceof Error ? cause.message : String(cause));
      setLoginState("idle");
    }
  };
  const loginButtonText = loginState === "starting" ? "Opening…" : loginState === "waiting" ? "Waiting…" : account.connected ? "Reconnect" : "Connect";
  return <main className="workspace settings-view"><header className="workspace-header"><div><h1>Account & usage</h1><p>Manage your Codex connection</p></div><button className="subtle-button" onClick={() => setSettingsOpen(false)}>Done</button></header>
    <div className="settings-content"><section className="settings-section"><h2>ChatGPT account</h2><div className="settings-card"><span className="large-avatar"><UserRound size={22} /></span><div className="settings-account"><strong>{account.email ?? "No account connected"}</strong><span>{loginMessage ?? (account.plan ? `ChatGPT ${account.plan}` : "Connect an account to use Codex")}</span>{loginError && <em className="settings-error">{loginError}</em>}</div><div className="settings-actions"><button className="primary-button" onClick={() => void login()} disabled={loginState !== "idle"}><LogIn size={15} /> {loginButtonText}</button>{account.connected && <button className="danger-button" onClick={onDisconnectAccount} disabled={loginState !== "idle"}><Unplug size={15} /> Disconnect</button>}</div></div></section>
    <section className="settings-section"><h2>Connection</h2><div className="settings-card compact"><span className={`connection-icon ${connected ? "online" : ""}`}>{connected ? <Activity size={19} /> : <Unplug size={19} />}</span><div className="settings-account"><strong>Codex App Server</strong><span>{connected ? "Connected and receiving live events" : "Not connected — dashboard is showing preview data"}</span></div><span className={`connection-label ${connected ? "online" : ""}`}>{connected ? "Online" : "Offline"}</span></div></section>
    <section className="settings-section"><h2>Notifications</h2><div className="settings-card compact"><span className={`connection-icon ${notificationsEnabled ? "online" : ""}`}>{notificationsEnabled ? <Bell size={19} /> : <BellOff size={19} />}</span><div className="settings-account"><strong>Desktop notifications</strong><span>Notify when Codex finishes, needs approval, or encounters an error</span>{notificationError && <em className="settings-error">{notificationError}</em>}</div><button className={`toggle ${notificationsEnabled ? "on" : ""}`} role="switch" aria-checked={notificationsEnabled} aria-label="Desktop notifications" onClick={() => void onNotificationsChange(!notificationsEnabled)}><span /></button></div></section>
    <section className="settings-section"><h2>Usage window</h2><div className="usage-large"><div><span>Current usage</span><strong>{account.usedPercent == null ? "—" : `${Math.round(account.usedPercent)}%`}</strong></div><div className="usage-track"><span style={{ width: `${account.usedPercent ?? 0}%` }} /></div><p><Clock3 size={14} /> {account.resetsAt ? `Resets ${relativeReset(account.resetsAt)}` : "Reset time is unavailable"}</p></div></section></div>
  </main>;
}

export function App() {
  const { sessions, archivedSessions, showingArchived, approvals, selectedId, settingsOpen, connected, loadingSessionId, sessionError, hydrate, applyEvent, mergeSession, addSession, archiveSession, restoreSession, setArchivedSessions, setShowingArchived, setSessionLoading } = useDashboard();
  const [newSessionOpen, setNewSessionOpen] = useState(false);
  const [sessionAction, setSessionAction] = useState<{ action: "rename" | "archive" | "restore"; session: Session }>();
  const [disconnectAccountOpen, setDisconnectAccountOpen] = useState(false);
  const [sidebarPreference, setSidebarPreference] = useState(() => localStorage.getItem(sidebarCollapsedKey) === "true");
  const [narrowWindow, setNarrowWindow] = useState(() => window.innerWidth < 960);
  const sidebarCollapsed = sidebarPreference || narrowWindow;
  const notifications = useDesktopNotifications(sessions, approvals ?? []);
  const applySnapshot = useCallback((snapshot: DashboardSnapshot) => applyEvent({ type: "snapshot", snapshot }), [applyEvent]);
  useEffect(() => { let unsubscribe: () => void = () => undefined; void getSnapshot().then((snapshot) => hydrate(snapshot.sessions, snapshot.approvals ?? [], snapshot.account, snapshot.connected)); void subscribe(applyEvent).then((fn) => { unsubscribe = fn; }); return () => unsubscribe(); }, [hydrate, applyEvent]);
  const session = useMemo(() => (showingArchived ? archivedSessions : sessions).find((item) => item.id === selectedId), [sessions, archivedSessions, showingArchived, selectedId]);
  useEffect(() => {
    if (!selectedId || !connected || session?.historyLoaded) return;
    let active = true;
    setSessionLoading(selectedId);
    void getSession(selectedId).then((loaded) => { if (active) mergeSession(loaded); }).catch((error: unknown) => { if (active) setSessionLoading(undefined, error instanceof Error ? error.message : String(error)); });
    return () => { active = false; };
  }, [selectedId, connected, session?.historyLoaded, mergeSession, setSessionLoading]);
  useSessionSync({ threadId: selectedId, enabled: !showingArchived && connected && Boolean(session?.historyLoaded), onSession: mergeSession });
  useSessionListSync({ enabled: connected, onSnapshot: applySnapshot });
  useEffect(() => {
    const openNewSession = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "n") {
        event.preventDefault();
        setNewSessionOpen(true);
      }
    };
    window.addEventListener("keydown", openNewSession);
    return () => window.removeEventListener("keydown", openNewSession);
  }, []);
  useEffect(() => {
    const updateWidth = () => setNarrowWindow(window.innerWidth < 960);
    window.addEventListener("resize", updateWidth);
    return () => window.removeEventListener("resize", updateWidth);
  }, []);
  useEffect(() => {
    const toggleSidebar = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === "b") {
        event.preventDefault();
        setSidebarPreference((current) => {
          const next = !current;
          localStorage.setItem(sidebarCollapsedKey, String(next));
          return next;
        });
      }
    };
    window.addEventListener("keydown", toggleSidebar);
    return () => window.removeEventListener("keydown", toggleSidebar);
  }, []);
  const sessionApprovals = (approvals ?? []).filter((approval) => approval.threadId === selectedId);
  const toggleSidebar = () => setSidebarPreference((current) => { const next = !current; localStorage.setItem(sidebarCollapsedKey, String(next)); return next; });
  return <div className={`app-shell ${sidebarCollapsed ? "sidebar-collapsed" : ""}`}><Sidebar collapsed={sidebarCollapsed} onToggle={toggleSidebar} onNewSession={() => setNewSessionOpen(true)} onSessionAction={(action, target) => setSessionAction({ action, session: target })} onArchiveViewChange={(archived) => { if (archived) void listArchivedSessions().then((items) => { setArchivedSessions(items); setShowingArchived(true); }); else setShowingArchived(false); }} />{settingsOpen ? <SettingsView notificationsEnabled={notifications.enabled} notificationError={notifications.permissionError} onNotificationsChange={notifications.setEnabled} onDisconnectAccount={() => setDisconnectAccountOpen(true)} /> : <Workspace session={session} approvals={sessionApprovals} archived={showingArchived} loading={loadingSessionId === selectedId} error={sessionError} />}{newSessionOpen && <NewSessionDialog defaultCwd={session?.cwd ?? ""} defaultModel={session?.model} onClose={() => setNewSessionOpen(false)} onCreated={(created) => { addSession(created); setNewSessionOpen(false); }} />}{sessionAction && <SessionActionDialog action={sessionAction.action} session={sessionAction.session} onClose={() => setSessionAction(undefined)} onRenamed={(renamed) => { mergeSession(renamed); setSessionAction(undefined); }} onArchived={(id) => { archiveSession(id); setSessionAction(undefined); }} onRestored={(restored) => { restoreSession(restored); setSessionAction(undefined); }} />}{disconnectAccountOpen && <DisconnectAccountDialog onClose={() => setDisconnectAccountOpen(false)} onDisconnected={() => setDisconnectAccountOpen(false)} />}</div>;
}
