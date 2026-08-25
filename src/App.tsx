import { useEffect, useMemo, useState } from "react";
import { Activity, ArrowUp, Bot, CircleStop, Clock3, Folder, Gauge, LoaderCircle, LogIn, MessageSquarePlus, MoreHorizontal, PanelLeftClose, Search, Settings, Sparkles, Unplug, UserRound } from "lucide-react";
import { getSession, getSnapshot, interruptTurn, sendTurn, startLogin, subscribe } from "./lib/bridge";
import { useDashboard } from "./store";
import type { Session, SessionStatus } from "./types";

const statusLabel: Record<SessionStatus, string> = { working: "Working", waiting: "Waiting", idle: "Idle", error: "Error" };

function relativeTime(timestamp: number) {
  const seconds = Math.max(1, Math.floor((Date.now() - timestamp) / 1000));
  if (seconds < 60) return "now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  return `${Math.floor(seconds / 3600)}h`;
}

function Sidebar() {
  const { sessions, selectedId, select, account, connected, setSettingsOpen } = useDashboard();
  const [query, setQuery] = useState("");
  const filtered = sessions.filter((session) => session.title.toLowerCase().includes(query.toLowerCase()));
  const usage = Math.min(100, Math.max(0, account.usedPercent ?? 0));
  return <aside className="sidebar">
    <div className="window-drag"><span /><span /><span /></div>
    <div className="brand-row"><div className="brand"><span className="brand-mark"><Sparkles size={15} /></span><span>Codex</span></div><button className="icon-button" aria-label="Collapse sidebar"><PanelLeftClose size={17} /></button></div>
    <button className="new-session"><MessageSquarePlus size={16} /> New session <kbd>⌘ N</kbd></button>
    <label className="search"><Search size={14} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search sessions" /></label>
    <div className="section-title"><span>Sessions</span><span>{filtered.length}</span></div>
    <nav className="session-list">
      {filtered.map((session) => <button key={session.id} className={`session-item ${selectedId === session.id ? "selected" : ""}`} onClick={() => select(session.id)}>
        <span className={`status-dot ${session.status}`} />
        <span className="session-copy"><strong>{session.title}</strong><span>{statusLabel[session.status]} · {relativeTime(session.updatedAt)}</span></span>
        <MoreHorizontal className="session-more" size={16} />
      </button>)}
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

function Workspace({ session, loading, error }: { session?: Session; loading: boolean; error?: string }) {
  const [draft, setDraft] = useState("");
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
    <section className="conversation">
      <div className="session-intro"><div className="intro-icon"><Bot size={20} /></div><div><h2>{session.title}</h2><p>Started with {session.model}</p></div></div>
      {loading ? <div className="history-state"><LoaderCircle size={19} /> Loading session history…</div> : error ? <div className="history-state error">{error}</div> : session.messages.length ? session.messages.map((message) => <article className={`message ${message.role}`} key={message.id}>
        <div className="message-avatar">{message.role === "assistant" ? <Sparkles size={15} /> : <UserRound size={15} />}</div>
        <div><div className="message-meta"><strong>{message.role === "assistant" ? "Codex" : "You"}</strong><span>{relativeTime(message.createdAt)}</span></div><p>{message.text}</p>{message.streaming && <span className="cursor" />}</div>
      </article>) : <div className="no-messages">This session has no messages yet.</div>}
    </section>
    <footer className="composer-wrap">
      {session.status === "working" && session.activeTurnId && <div className="running-bar"><span><span className="pulse" /> Codex is working</span><button onClick={() => interruptTurn(session.id, session.activeTurnId!)}><CircleStop size={14} /> Stop</button></div>}
      <div className="composer"><textarea rows={2} value={draft} onChange={(event) => setDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); void submit(); } }} placeholder="Message Codex…" /><div className="composer-bottom"><span>↵ send · ⇧↵ new line</span><button disabled={!draft.trim()} onClick={() => void submit()}><ArrowUp size={17} /></button></div></div>
      <div className="token-summary"><span>{session.model}</span><span>{(session.tokenUsage.input + session.tokenUsage.output).toLocaleString()} tokens</span><span>{session.tokenUsage.cached.toLocaleString()} cached</span></div>
    </footer>
  </main>;
}

function SettingsView() {
  const { account, connected, setSettingsOpen } = useDashboard();
  const [loggingIn, setLoggingIn] = useState(false);
  const login = async () => { setLoggingIn(true); try { const url = await startLogin(); if (url) window.open(url, "_blank"); } finally { setLoggingIn(false); } };
  return <main className="workspace settings-view"><header className="workspace-header"><div><h1>Account & usage</h1><p>Manage your Codex connection</p></div><button className="subtle-button" onClick={() => setSettingsOpen(false)}>Done</button></header>
    <div className="settings-content"><section className="settings-section"><h2>ChatGPT account</h2><div className="settings-card"><span className="large-avatar"><UserRound size={22} /></span><div className="settings-account"><strong>{account.email ?? "No account connected"}</strong><span>{account.plan ? `ChatGPT ${account.plan}` : "Connect an account to use Codex"}</span></div><button className="primary-button" onClick={() => void login()} disabled={loggingIn}><LogIn size={15} /> {loggingIn ? "Opening…" : account.connected ? "Reconnect" : "Connect"}</button></div></section>
    <section className="settings-section"><h2>Connection</h2><div className="settings-card compact"><span className={`connection-icon ${connected ? "online" : ""}`}>{connected ? <Activity size={19} /> : <Unplug size={19} />}</span><div className="settings-account"><strong>Codex App Server</strong><span>{connected ? "Connected and receiving live events" : "Not connected — dashboard is showing preview data"}</span></div><span className={`connection-label ${connected ? "online" : ""}`}>{connected ? "Online" : "Offline"}</span></div></section>
    <section className="settings-section"><h2>Usage window</h2><div className="usage-large"><div><span>Current usage</span><strong>{account.usedPercent == null ? "—" : `${Math.round(account.usedPercent)}%`}</strong></div><div className="usage-track"><span style={{ width: `${account.usedPercent ?? 0}%` }} /></div><p><Clock3 size={14} /> {account.resetsAt ? `Resets ${relativeReset(account.resetsAt)}` : "Reset time is unavailable"}</p></div></section></div>
  </main>;
}

export function App() {
  const { sessions, selectedId, settingsOpen, connected, loadingSessionId, sessionError, hydrate, applyEvent, mergeSession, setSessionLoading } = useDashboard();
  useEffect(() => { let unsubscribe: () => void = () => undefined; void getSnapshot().then((snapshot) => hydrate(snapshot.sessions, snapshot.account, snapshot.connected)); void subscribe(applyEvent).then((fn) => { unsubscribe = fn; }); return () => unsubscribe(); }, [hydrate, applyEvent]);
  const session = useMemo(() => sessions.find((item) => item.id === selectedId), [sessions, selectedId]);
  useEffect(() => {
    if (!selectedId || !connected || session?.historyLoaded || loadingSessionId === selectedId) return;
    let active = true;
    setSessionLoading(selectedId);
    void getSession(selectedId).then((loaded) => { if (active) mergeSession(loaded); }).catch((error: unknown) => { if (active) setSessionLoading(undefined, error instanceof Error ? error.message : String(error)); });
    return () => { active = false; };
  }, [selectedId, connected, session?.historyLoaded, loadingSessionId, mergeSession, setSessionLoading]);
  return <div className="app-shell"><Sidebar />{settingsOpen ? <SettingsView /> : <Workspace session={session} loading={loadingSessionId === selectedId} error={sessionError} />}</div>;
}
