import { create } from "zustand";
import type { Account, ApprovalRequest, DashboardEvent, Session } from "./types";

interface DashboardState {
  sessions: Session[];
  archivedSessions: Session[];
  showingArchived: boolean;
  approvals: ApprovalRequest[];
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
  sessions: [], archivedSessions: [], showingArchived: false, approvals: [], account: { connected: false }, connected: false, settingsOpen: false,
  hydrate: (sessions, approvals, account, connected) => set((state) => ({
    sessions,
    archivedSessions: state.archivedSessions,
    approvals,
    account,
    connected,
    selectedId: state.selectedId && sessions.some((session) => session.id === state.selectedId) ? state.selectedId : sessions[0]?.id,
  })),
  select: (selectedId) => set({ selectedId, settingsOpen: false }),
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  setSessionLoading: (loadingSessionId, sessionError) => set({ loadingSessionId, sessionError }),
  mergeSession: (session) => set((state) => ({
    sessions: state.sessions.map((item) => item.id === session.id ? session : item),
    archivedSessions: state.archivedSessions.map((item) => item.id === session.id ? session : item),
    loadingSessionId: state.loadingSessionId === session.id ? undefined : state.loadingSessionId,
    sessionError: undefined,
  })),
  addSession: (session) => set((state) => ({
    sessions: [session, ...state.sessions.filter((item) => item.id !== session.id)],
    selectedId: session.id,
    showingArchived: false,
    settingsOpen: false,
  })),
  setArchivedSessions: (archivedSessions) => set({ archivedSessions }),
  setShowingArchived: (showingArchived) => set((state) => ({
    showingArchived,
    selectedId: (showingArchived ? state.archivedSessions : state.sessions)[0]?.id,
    settingsOpen: false,
  })),
  archiveSession: (id) => set((state) => {
    const archived = state.sessions.find((session) => session.id === id);
    const sessions = state.sessions.filter((session) => session.id !== id);
    return { sessions, archivedSessions: archived ? [archived, ...state.archivedSessions.filter((item) => item.id !== id)] : state.archivedSessions, selectedId: state.selectedId === id ? sessions[0]?.id : state.selectedId };
  }),
  restoreSession: (session) => set((state) => ({
    sessions: [session, ...state.sessions.filter((item) => item.id !== session.id)],
    archivedSessions: state.archivedSessions.filter((item) => item.id !== session.id),
    showingArchived: false,
    selectedId: session.id,
  })),
  removeSession: (id) => set((state) => {
    const sessions = state.sessions.filter((session) => session.id !== id);
    return {
      sessions,
      approvals: state.approvals.filter((approval) => approval.threadId !== id),
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
      return {
        sessions,
        archivedSessions,
        approvals: event.snapshot.approvals ?? [],
        account: event.snapshot.account,
        connected: event.snapshot.connected,
        selectedId: state.showingArchived
          ? state.selectedId && archivedSessions.some((session) => session.id === state.selectedId) ? state.selectedId : archivedSessions[0]?.id
          : state.selectedId && sessions.some((session) => session.id === state.selectedId) ? state.selectedId : sessions[0]?.id,
      };
    }
    if (event.type === "approval.requested") return { approvals: [...state.approvals.filter((item) => item.requestId !== event.approval.requestId), event.approval] };
    if (event.type === "approval.resolved") return { approvals: state.approvals.filter((item) => item.requestId !== event.requestId) };
    if (event.type === "account.updated") return { account: event.account };
    if (event.type === "connection.changed") return { connected: event.connected };
    const exists = state.sessions.some((session) => session.id === event.session.id);
    return { sessions: exists ? state.sessions.map((session) => session.id === event.session.id ? event.session : session) : [event.session, ...state.sessions] };
  }),
}));
