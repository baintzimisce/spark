# Design Tokens & Guidelines — Spark

Based on the **Impeccable** Neo Kinpaku design system.
> *"Two brand anchors, kinpaku gold and verdigris patina, sit on dark warm-black lacquer. Restraint in chrome, brilliance in texture."*

## Foundations

### Typography
- **Display / Headings:** Clean sans-serif (Segoe UI Variable / Inter), bold/semibold, subtle tracking.
- **Body / Interface:** System Sans (`13px–14px`), high legibility on dark lacquer.
- **Code / Monospace:** Cascadia Code / Consolas (`12px–13px`) for streamed CLI output and command previews.

### Color Palette (Impeccable Neo Kinpaku — sRGB approximations for Native Rust UI)
- **Canvas / Background (`lacquer-black`):** `#12110E` (`oklch(7% 0.006 95)`)
- **Deepest Inset (`lacquer-deep`):** `#0A0907` (`oklch(4% 0.004 95)`) — output consoles, textareas
- **Surface / Card (`raised-lacquer`):** `#1C1A16` (`oklch(11% 0.006 95)`) — agent panes, header bar
- **Inactive Control (`graphite`):** `#26241F` (`oklch(15% 0.008 95)`)
- **Hover / Step Up (`graphite-2`):** `#302E28` (`oklch(19% 0.008 95)`)
- **Primary / Brand (`kinpaku-gold`):** `#EAB308` / `#F0C446` (`oklch(84% 0.19 80.46)`) — Transmit CTA, active focus
- **On Primary (`dark-ink`):** `#24221B` (`oklch(14% 0.018 95)`) — text on gold buttons
- **Secondary / Live State (`verdigris-patina`):** `#4DB6AC` (`oklch(70% 0.12 188)`) — running/connected status
- **Primary Text (`champagne` / `text-warm`):** `#E8E8E8` / `#DFDFDF`
- **Muted Text (`text-muted`):** `#B8B8B8`
- **Faint Text (`text-faint`):** `#9E9E9E`
- **Border / Hairline:** `1px` subtle warm graphite (`#2E2C26`)

## Surface & Hierarchy
- **Lacquer layering:** `lacquer-black` page ground → `raised-lacquer` agent panels → `lacquer-deep` recessed output streams.
- **No cards-in-cards:** Keep a single level of containment per agent column; use hairline dividers (`1px`) instead of nested boxes.
- **Restraint in chrome:** Reserve `kinpaku-gold` strictly for the primary action (`Transmit`) and active tab/focus indicators. Do not flood the screen with saturated colors.

## Component Conventions
- **Project-Grouped Active Sessions Layout (v0.2):** Left `SidePanel` in `raised-lacquer` listing active sessions grouped under collapsible Projects, with top-bar multi-select AI filter pills (`◆ Antigravity`, `● Claude Code`, `▲ OpenCode`) and a single focused session workspace in `lacquer-black`.
- **Agent Distinction:** Subtle top hairline accent or minimal glyph per agent (`◆` Antigravity, `●` Claude Code, `▲` OpenCode) rather than loud neon borders.
- **Status Pills:** Minimalist badges (`READY`, `RUNNING` in `verdigris-patina`, `DONE`, `FAILED`).
- **Interactive Controls:** Keyboard-first (`Ctrl+Enter` to transmit), explicit `Stop` / `Stop All` button visible when processes are active.
