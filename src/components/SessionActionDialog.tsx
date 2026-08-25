import { Archive, LoaderCircle, Pencil, X } from "lucide-react";
import { FormEvent, useEffect, useRef, useState } from "react";
import { archiveThread, renameThread } from "../lib/bridge";
import type { Session } from "../types";

export function SessionActionDialog({ action, session, onClose, onRenamed, onArchived }: { action: "rename" | "archive"; session: Session; onClose: () => void; onRenamed: (session: Session) => void; onArchived: (id: string) => void }) {
  const [name, setName] = useState(session.title);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string>();
  const inputRef = useRef<HTMLInputElement>(null);
  const isRename = action === "rename";

  useEffect(() => { if (isRename) inputRef.current?.select(); }, [isRename]);
  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => { if (event.key === "Escape" && !submitting) onClose(); };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose, submitting]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (submitting || (isRename && !name.trim())) return;
    setSubmitting(true);
    setError(undefined);
    try {
      if (isRename) onRenamed(await renameThread(session.id, name.trim()));
      else { await archiveThread(session.id); onArchived(session.id); }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setSubmitting(false);
    }
  };

  return <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && !submitting) onClose(); }}>
    <form className="session-action-dialog" role="dialog" aria-modal="true" aria-labelledby="session-action-title" onSubmit={(event) => void submit(event)}>
      <div className="dialog-heading"><span>{isRename ? <Pencil size={17} /> : <Archive size={17} />}</span><div><h2 id="session-action-title">{isRename ? "Rename session" : "Archive session"}</h2><p>{isRename ? "Choose a descriptive name" : "Remove this session from the active list"}</p></div><button type="button" aria-label="Close session action dialog" onClick={onClose} disabled={submitting}><X size={17} /></button></div>
      <div className="session-action-content">
        {isRename ? <label><span>Session name</span><input ref={inputRef} value={name} onChange={(event) => setName(event.target.value)} maxLength={120} required /></label> : <><p>Archive <strong>{session.title}</strong>?</p><span>The session history remains stored locally and is not deleted.</span></>}
        {error && <div className="dialog-error">{error}</div>}
      </div>
      <div className="dialog-actions"><button type="button" onClick={onClose} disabled={submitting}>Cancel</button><button className={isRename ? "dialog-create" : "dialog-danger"} type="submit" disabled={submitting || (isRename && !name.trim())}>{submitting && <LoaderCircle size={14} />}{submitting ? (isRename ? "Saving…" : "Archiving…") : (isRename ? "Save name" : "Archive")}</button></div>
    </form>
  </div>;
}
