import { create } from "zustand";
import type { Account, ApprovalRequest, DashboardEvent, Session } from "./types";

interface DashboardState {
  sessions: Session[];
  archivedSessions: Session[];
  showingArchived: boolean;
  approvals: ApprovalRequest[];
  unreadCounts: Record<string, number>;
  account: Account;
  connected: boolean;
  selectedId?: string;
  settingsOpen: boolean;
  loadingSessionId?: string;
  sessionError?: string;
  hydrate: (sessions: Session[], approvals: ApprovalRequest[], account: Account, connected: boolean) => void;
  select: (id: string) => void;
  setSettingsOpen: (open: boolean) => void;
  setSessionLoading: (id?: string, error?: string) => void;
  mergeSession: (session: Session) => void;
  addSession: (session: Session) => void;
  removeSession: (id: string) => void;
  setArchivedSessions: (sessions: Session[]) => void;
  setShowingArchived: (showing: boolean) => void;
  archiveSession: (id: string) => void;
  restoreSession: (session: Session) => void;
  applyEvent: (event: DashboardEvent) => void;
}

export const useDashboard = create<DashboardState>((set) => ({
  sessions: [], archivedSessions: [], showingArchived: false, approvals: [], unreadCounts: {}, account: { connected: false }, connected: false, settingsOpen: false,
  hydrate: (sessions, approvals, account, connected) => set((state) => ({
    sessions,
    archivedSessions: state.archivedSessions,
    approvals,
    account,
    connected,
    selectedId: state.selectedId && sessions.some((session) => session.id === state.selectedId) ? state.selectedId : sessions[0]?.id,
  })),
  select: (selectedId) => set((state) => ({ selectedId, settingsOpen: false, unreadCounts: { ...state.unreadCounts, [selectedId]: 0 } })),
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  setSessionLoading: (loadingSessionId, sessionError) => set({ loadingSessionId, sessionError }),
  mergeSession: (session) => set((state) => {
    const previous = state.sessions.find((item) => item.id === session.id) ?? state.archivedSessions.find((item) => item.id === session.id);
    const hasNewActivity = previous && (session.updatedAt > previous.updatedAt || session.messages.length > previous.messages.length);
    return {
      sessions: state.sessions.map((item) => item.id === session.id ? session : item),
      archivedSessions: state.archivedSessions.map((item) => item.id === session.id ? session : item),
      unreadCounts: hasNewActivity && state.selectedId !== session.id ? { ...state.unreadCounts, [session.id]: Math.min(99, (state.unreadCounts[session.id] ?? 0) + 1) } : state.unreadCounts,
      loadingSessionId: state.loadingSessionId === session.id ? undefined : state.loadingSessionId,
      sessionError: undefined,
    };
  }),
  addSession: (session) => set((state) => ({
    sessions: [session, ...state.sessions.filter((item) => item.id !== session.id)],
    selectedId: session.id,
    unreadCounts: { ...state.unreadCounts, [session.id]: 0 },
    showingArchived: false,
    settingsOpen: false,
  })),
  setArchivedSessions: (archivedSessions) => set({ archivedSessions }),
  setShowingArchived: (showingArchived) => set((state) => {
    const selectedId = (showingArchived ? state.archivedSessions : state.sessions)[0]?.id;
    return { showingArchived, selectedId, settingsOpen: false, unreadCounts: selectedId ? { ...state.unreadCounts, [selectedId]: 0 } : state.unreadCounts };
  }),
  archiveSession: (id) => set((state) => {
    const archived = state.sessions.find((session) => session.id === id);
    const sessions = state.sessions.filter((session) => session.id !== id);
    return { sessions, archivedSessions: archived ? [archived, ...state.archivedSessions.filter((item) => item.id !== id)] : state.archivedSessions, selectedId: state.selectedId === id ? sessions[0]?.id : state.selectedId, unreadCounts: { ...state.unreadCounts, [id]: 0 } };
  }),
  restoreSession: (session) => set((state) => ({
    sessions: [session, ...state.sessions.filter((item) => item.id !== session.id)],
    archivedSessions: state.archivedSessions.filter((item) => item.id !== session.id),
    showingArchived: false,
    selectedId: session.id,
    unreadCounts: { ...state.unreadCounts, [session.id]: 0 },
  })),
  removeSession: (id) => set((state) => {
    const sessions = state.sessions.filter((session) => session.id !== id);
    return {
      sessions,
      approvals: state.approvals.filter((approval) => approval.threadId !== id),
      unreadCounts: { ...state.unreadCounts, [id]: 0 },
      selectedId: state.selectedId === id ? sessions[0]?.id : state.selectedId,
    };
  }),
  applyEvent: (event) => set((state) => {
    if (event.type === "snapshot") {
      const sessions = event.snapshot.sessions.map((session) => {
        const current = state.sessions.find((item) => item.id === session.id);
        return current?.historyLoaded ? { ...session, messages: current.messages, tokenUsage: current.tokenUsage, activeTurnId: current.activeTurnId, historyLoaded: true } : session;
      });
      const archivedSessions = event.snapshot.archivedSessions ?? state.archivedSessions;
      const unreadCounts = { ...state.unreadCounts };
      for (const session of sessions) {
        const previous = state.sessions.find((item) => item.id === session.id);
        if (previous && session.id !== state.selectedId && session.updatedAt > previous.updatedAt) unreadCounts[session.id] = Math.min(99, (unreadCounts[session.id] ?? 0) + 1);
      }
      return {
        sessions,
        archivedSessions,
        approvals: event.snapshot.approvals ?? [],
        account: event.snapshot.account,
        connected: event.snapshot.connected,
        unreadCounts,
        selectedId: state.showingArchived
          ? state.selectedId && archivedSessions.some((session) => session.id === state.selectedId) ? state.selectedId : archivedSessions[0]?.id
          : state.selectedId && sessions.some((session) => session.id === state.selectedId) ? state.selectedId : sessions[0]?.id,
      };
    }
    if (event.type === "approval.requested") return { approvals: [...state.approvals.filter((item) => item.requestId !== event.approval.requestId), event.approval], unreadCounts: event.approval.threadId !== state.selectedId ? { ...state.unreadCounts, [event.approval.threadId]: Math.min(99, (state.unreadCounts[event.approval.threadId] ?? 0) + 1) } : state.unreadCounts };
    if (event.type === "approval.resolved") return { approvals: state.approvals.filter((item) => item.requestId !== event.requestId) };
    if (event.type === "account.updated") return { account: event.account };
    if (event.type === "connection.changed") return { connected: event.connected };
    const previous = state.sessions.find((session) => session.id === event.session.id);
    const exists = Boolean(previous);
    const hasNewActivity = previous && (event.session.updatedAt > previous.updatedAt || event.session.messages.length > previous.messages.length);
    return { sessions: exists ? state.sessions.map((session) => session.id === event.session.id ? event.session : session) : [event.session, ...state.sessions], unreadCounts: hasNewActivity && event.session.id !== state.selectedId ? { ...state.unreadCounts, [event.session.id]: Math.min(99, (state.unreadCounts[event.session.id] ?? 0) + 1) } : state.unreadCounts };
  }),
}));
