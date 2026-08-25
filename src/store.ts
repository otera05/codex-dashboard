import { create } from "zustand";
import type { Account, DashboardEvent, Session } from "./types";

interface DashboardState {
  sessions: Session[];
  account: Account;
  connected: boolean;
  selectedId?: string;
  settingsOpen: boolean;
  loadingSessionId?: string;
  sessionError?: string;
  hydrate: (sessions: Session[], account: Account, connected: boolean) => void;
  select: (id: string) => void;
  setSettingsOpen: (open: boolean) => void;
  setSessionLoading: (id?: string, error?: string) => void;
  mergeSession: (session: Session) => void;
  applyEvent: (event: DashboardEvent) => void;
}

export const useDashboard = create<DashboardState>((set) => ({
  sessions: [], account: { connected: false }, connected: false, settingsOpen: false,
  hydrate: (sessions, account, connected) => set((state) => ({
    sessions,
    account,
    connected,
    selectedId: state.selectedId && sessions.some((session) => session.id === state.selectedId) ? state.selectedId : sessions[0]?.id,
  })),
  select: (selectedId) => set({ selectedId, settingsOpen: false }),
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  setSessionLoading: (loadingSessionId, sessionError) => set({ loadingSessionId, sessionError }),
  mergeSession: (session) => set((state) => ({
    sessions: state.sessions.map((item) => item.id === session.id ? session : item),
    loadingSessionId: state.loadingSessionId === session.id ? undefined : state.loadingSessionId,
    sessionError: undefined,
  })),
  applyEvent: (event) => set((state) => {
    if (event.type === "snapshot") {
      const sessions = event.snapshot.sessions.map((session) => {
        const current = state.sessions.find((item) => item.id === session.id);
        return current?.historyLoaded ? { ...session, messages: current.messages, tokenUsage: current.tokenUsage, activeTurnId: current.activeTurnId, historyLoaded: true } : session;
      });
      return {
        sessions,
        account: event.snapshot.account,
        connected: event.snapshot.connected,
        selectedId: state.selectedId && sessions.some((session) => session.id === state.selectedId) ? state.selectedId : sessions[0]?.id,
      };
    }
    if (event.type === "account.updated") return { account: event.account };
    if (event.type === "connection.changed") return { connected: event.connected };
    const exists = state.sessions.some((session) => session.id === event.session.id);
    return { sessions: exists ? state.sessions.map((session) => session.id === event.session.id ? event.session : session) : [event.session, ...state.sessions] };
  }),
}));
