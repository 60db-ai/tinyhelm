# TinyHelm frontend (React + TypeScript)

The SPA that replaces the Livewire/Blade UI. Talks to the Laravel `/api/v1` surface
(directly in dev via Vite proxy; in prod via the Rust gateway in `../rust-backend`).

Stack: Vite + React 18 + TypeScript, Tailwind v4, shadcn/ui, evilcharts-style charts (Recharts), lucide-react.

## Setup

```bash
cd frontend
npm install
npm run dev        # http://localhost:5173 (proxies /api -> http://127.0.0.1:8000)
npm run build      # outputs dist/, served by the Rust gateway
```

## shadcn/ui + evilcharts

Base tokens live in `src/index.css`; `src/components/ui/button.tsx` is a shadcn component.
Add more:

```bash
npx shadcn@latest add card table dialog input
# evilcharts: add from its registry, e.g.
npx shadcn@latest add https://evilcharts.com/r/gradient-area-chart.json
```

## Structure

- `src/App.tsx` — dashboard shell (API health check, stat cards, charts).
- `src/components/ui/` — shadcn primitives.
- `src/components/metrics-chart.tsx` — evilcharts-style gradient area chart.
- `src/lib/utils.ts` — `cn()` helper.

## Status

Scaffold / foundation only. Pages are ported incrementally from the Livewire components;
auth uses the existing Sanctum API tokens.
