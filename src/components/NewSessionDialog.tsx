import { Bot, Folder, FolderOpen, LoaderCircle, X } from "lucide-react";
import { FormEvent, useEffect, useRef, useState } from "react";
import { createThread, listModels, pickDirectory, validateDirectory } from "../lib/bridge";
import type { CodexModel, Session } from "../types";

export function NewSessionDialog({ defaultCwd, defaultModel, onClose, onCreated }: { defaultCwd: string; defaultModel?: string; onClose: () => void; onCreated: (session: Session) => void }) {
  const recentDirectoriesKey = "codex-dashboard.recent-directories";
  const [cwd, setCwd] = useState(defaultCwd);
  const [model, setModel] = useState(defaultModel ?? "");
  const [prompt, setPrompt] = useState("");
  const [models, setModels] = useState<CodexModel[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string>();
  const promptRef = useRef<HTMLTextAreaElement>(null);
  const [recentDirectories, setRecentDirectories] = useState<string[]>(() => {
    try { return JSON.parse(localStorage.getItem(recentDirectoriesKey) ?? "[]").filter((value: unknown) => typeof value === "string").slice(0, 5); } catch { return []; }
  });

  useEffect(() => {
    promptRef.current?.focus();
    void listModels().then((items) => {
      setModels(items);
      setModel((current) => items.some((item) => item.id === current) ? current : items.find((item) => item.isDefault)?.id || items[0]?.id || "");
    }).catch((cause: unknown) => setError(cause instanceof Error ? cause.message : String(cause)));
  }, []);

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => { if (event.key === "Escape" && !submitting) onClose(); };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose, submitting]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!cwd.trim() || submitting) return;
    setSubmitting(true);
    setError(undefined);
    try {
      const directory = cwd.trim();
      if (!await validateDirectory(directory)) throw new Error("Working directory does not exist or is not a folder.");
      const created = await createThread(directory, model || undefined, prompt.trim());
      const nextRecent = [directory, ...recentDirectories.filter((item) => item !== directory)].slice(0, 5);
      localStorage.setItem(recentDirectoriesKey, JSON.stringify(nextRecent));
      setRecentDirectories(nextRecent);
      onCreated(created);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setSubmitting(false);
    }
  };

  const browse = async () => {
    try {
      const selected = await pickDirectory(cwd.trim() || undefined);
      if (selected) { setCwd(selected); setError(undefined); }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  return <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && !submitting) onClose(); }}>
    <form className="new-session-dialog" role="dialog" aria-modal="true" aria-labelledby="new-session-title" onSubmit={(event) => void submit(event)}>
      <div className="dialog-heading"><span><Bot size={18} /></span><div><h2 id="new-session-title">New Codex session</h2><p>Start a local coding session</p></div><button type="button" aria-label="Close new session dialog" onClick={onClose} disabled={submitting}><X size={17} /></button></div>
      <div className="dialog-fields">
        <label><span>Working directory</span><div className="dialog-input directory-input"><Folder size={14} /><input value={cwd} onChange={(event) => setCwd(event.target.value)} placeholder="/path/to/project" required /><button type="button" aria-label="Browse working directory" onClick={() => void browse()}><FolderOpen size={14} /> Browse</button></div>{recentDirectories.length > 0 && <div className="recent-directories"><small>Recent</small>{recentDirectories.map((directory) => <button type="button" title={directory} onClick={() => setCwd(directory)} key={directory}>{directory}</button>)}</div>}</label>
        <label><span>Model</span><select value={model} onChange={(event) => setModel(event.target.value)} disabled={!models.length}>{models.length ? models.map((item) => <option value={item.id} key={item.id}>{item.displayName}</option>) : <option value={model}>{model || "Loading models…"}</option>}</select></label>
        <label><span>Initial message <small>Optional</small></span><textarea ref={promptRef} rows={5} value={prompt} onChange={(event) => setPrompt(event.target.value)} placeholder="What should Codex work on?" /></label>
        {error && <div className="dialog-error">{error}</div>}
      </div>
      <div className="dialog-actions"><button type="button" onClick={onClose} disabled={submitting}>Cancel</button><button className="dialog-create" type="submit" disabled={!cwd.trim() || submitting}>{submitting && <LoaderCircle size={14} />}{submitting ? "Creating…" : "Create session"}</button></div>
    </form>
  </div>;
}
