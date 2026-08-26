import { useEffect } from "react";
import { Activity, Bot, FileCode2, Folder, Gauge, MessageSquare, Terminal, X } from "lucide-react";
import type { Session } from "../types";

export function SessionActivityPanel({ session, onClose }: { session: Session; onClose: () => void }) {
  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  const messages = session.messages.filter((item) => item.type === "message").length;
  const commands = session.messages.filter((item) => item.type === "command").length;
  const files = new Set(session.messages.flatMap((item) => item.type === "fileChange" ? item.changes.map((change) => change.path) : [])).size;
  const totalTokens = session.tokenUsage.input + session.tokenUsage.output;

  return <div className="activity-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <aside className="activity-panel" role="dialog" aria-modal="true" aria-labelledby="activity-title">
      <header className="activity-panel-header"><div><Activity size={16} /><h2 id="activity-title">Session activity</h2></div><button className="icon-button" aria-label="Close activity panel" onClick={onClose}><X size={18} /></button></header>
      <div className="activity-panel-body">
        <section className="activity-session"><span><Bot size={17} /></span><div><strong>{session.title}</strong><small>{session.model}</small></div><i className={`status-dot ${session.status}`} /></section>
        <section className="activity-location"><Folder size={14} /><span>{session.cwd}</span></section>
        <section className="activity-stats" aria-label="Session statistics">
          <div><MessageSquare size={15} /><strong>{messages}</strong><span>Messages</span></div>
          <div><Terminal size={15} /><strong>{commands}</strong><span>Commands</span></div>
          <div><FileCode2 size={15} /><strong>{files}</strong><span>Files changed</span></div>
        </section>
        <section className="activity-token-card"><div><Gauge size={15} /><span>Token usage</span></div><strong>{totalTokens.toLocaleString()}</strong><dl><div><dt>Input</dt><dd>{session.tokenUsage.input.toLocaleString()}</dd></div><div><dt>Output</dt><dd>{session.tokenUsage.output.toLocaleString()}</dd></div><div><dt>Cached</dt><dd>{session.tokenUsage.cached.toLocaleString()}</dd></div></dl></section>
      </div>
    </aside>
  </div>;
}
