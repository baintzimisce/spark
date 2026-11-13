# Design Tokens & Guidelines — Spark

Based on the **Impeccable** Obsidian & Mist Blue design system.
> *"Cool obsidian surfaces, calm mist blue actions, and verdigris live telemetry. Restraint in chrome, clarity in conversation."*

## Foundations

### Typography
- **Display / Headings:** Clean sans-serif (Segoe UI Variable / Segoe UI / Inter), bold/semibold, subtle tracking.
- **Body / Interface:** System Sans (`13.5px`), high legibility on deep obsidian.
- **Code / Monospace:** Cascadia Mono / Cascadia Code / Consolas (`12.2px–12.5px`) for code blocks and raw CLI streams.

### Color Palette (Impeccable Obsidian & Mist Blue — sRGB for Native Rust UI)
- **Canvas / Background (`obsidian-bg`):** `#101216`
- **Deepest Inset (`obsidian-deep`):** `#0B0D10` — code blocks, filter inputs, raw output
- **Surface / Raised (`obsidian-raised`):** `#171A20` — top bar, sidebar, composer card, modals
- **Subtle Surface (`user-surface`):** `#14171D` — user prompt bubbles, row hover
- **Inactive Control (`slate-control`):** `#20242C`
- **Hover / Step Up (`slate-hover`):** `#292E38`
- **Primary / Accent (`mist-blue`):** `#7AA2F7` (`#93B4FA` bright inline code highlight) — Transmit CTA, `+ New Session`, active focus
- **On Primary (`on-accent`):** `#0E131D` — high-contrast ink on mist-blue buttons
- **Secondary / Live State (`verdigris-patina`):** `#4DB6AC` — running/connected/done status
- **Primary Text (`text-bright` / `text-body`):** `#ECEEF2` / `#D5D9E0`
- **Muted Text (`text-muted`):** `#9DA5B4`
- **Faint Text (`text-faint`):** `#717887`
- **Border / Hairline:** `1px` cool slate (`#272C36` standard, `#1F232B` subtle)

## Surface & Hierarchy
- **Obsidian layering:** `obsidian-bg` page ground → `obsidian-raised` sidebar & composer → `obsidian-deep` recessed code & search insets.
- **No cards-in-cards:** Keep a single level of containment; use hairline dividers (`1px`) and surface stepping instead of nested boxes.
- **Restraint in chrome:** Reserve `mist-blue` strictly for primary actions (`Transmit`, `+ New Session`) and active focus/selection indicators.

## Component Conventions
- **Project-Grouped Active Sessions Layout (v0.2):** Left `SidePanel` in `obsidian-raised` listing active sessions grouped under collapsible Projects, with top-bar multi-select AI filter chips (`◆ Antigravity`, `● Claude Code`, `▲ OpenCode`) and a centered conversation workspace in `obsidian-bg`.
- **Agent Distinction:** Minimal geometric glyph per agent (`◆` Antigravity, `●` Claude Code, `▲` OpenCode) with subtle status coloration rather than loud neon borders.
- **Status Pills:** Minimalist tinted pills (`READY`, `RUNNING` in `verdigris-patina`, `DONE`, `FAILED`).
- **Interactive Controls:** Keyboard-first (`Ctrl+Enter` to transmit, `Ctrl+N` new session, `Esc` dismiss modal), explicit `Stop` / `Stop All` button visible when processes are active.
