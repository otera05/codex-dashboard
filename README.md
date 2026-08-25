# Codex Dashboard

A desktop dashboard for monitoring and controlling multiple local Codex sessions in real time.

## Stack

- Tauri 2 and Rust for local process management
- React, TypeScript, and Vite for the interface
- Codex App Server over JSON-RPC/stdio

## Development

Requirements: Node.js 20+, Rust stable, and a recent `codex` CLI available on `PATH`.

```sh
npm install
npm run tauri dev
```

For UI-only development (with preview data):

```sh
npm run dev
```

The native process launches `codex app-server --stdio`, reads session/account data, and forwards normalized events to the React application. ChatGPT credentials stay inside Codex; the dashboard does not persist them.
