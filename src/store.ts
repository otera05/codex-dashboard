import { create } from "zustand";
import type { Account, DashboardEvent, Session } from "./types";

interface DashboardState {
  sessions: Session[];
  account: Account;
  connected: boolean;
  selectedId?: string;
  settingsOpen: boolean;
  hydrate: (sessions: Session[], account: Account, connected: boolean) => void;
  select: (id: string) => void;
  setSettingsOpen: (open: boolean) => void;
  applyEvent: (event: DashboardEvent) => void;
}

export const useDashboard = create<DashboardState>((set) => ({
  sessions: [], account: { connected: false }, connected: false, settingsOpen: false,
  hydrate: (sessions, account, connected) => set((state) => ({ sessions, account, connected, selectedId: state.selectedId ?? sessions[0]?.id })),
  select: (selectedId) => set({ selectedId, settingsOpen: false }),
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  applyEvent: (event) => set((state) => {
    if (event.type === "snapshot") return { sessions: event.snapshot.sessions, account: event.snapshot.account, connected: event.snapshot.connected, selectedId: state.selectedId ?? event.snapshot.sessions[0]?.id };
    if (event.type === "account.updated") return { account: event.account };
    if (event.type === "connection.changed") return { connected: event.connected };
    const exists = state.sessions.some((session) => session.id === event.session.id);
    return { sessions: exists ? state.sessions.map((session) => session.id === event.session.id ? event.session : session) : [event.session, ...state.sessions] };
  }),
}));
