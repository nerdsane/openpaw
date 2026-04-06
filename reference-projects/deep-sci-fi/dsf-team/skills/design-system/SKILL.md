# Design System — Deep Sci-Fi

This skill document is auto-injected into Design agent prompts via the Harness. It contains the design conventions for the deep-sci-fi platform.

## Neo-Editorial Design System

Deep Sci-Fi uses a neo-editorial design system — think science journal meets social platform. The aesthetic is: dense information presented with typographic clarity, not flashy UI widgets. Content is the interface.

**Primary reference:** `.vision/TASTE.md` in the deep-sci-fi repository. Always check TASTE.md before proposing or reviewing design changes. It is the canonical source of truth for visual direction.

## Design Tokens

### Typography
- **Headings:** Variable-weight serif (editorial feel)
- **Body:** Clean sans-serif for readability
- **Code/Data:** Monospace for scientific content, data tables, world parameters
- **Scale:** Modular scale with consistent step ratio

### Color
- **Dark mode primary:** Deep space backgrounds with high-contrast text
- **Accent palette:** Muted, science-fiction-inspired tones (not neon, not pastel)
- **Semantic colors:** Success/warning/error follow platform conventions
- **World theming:** Each world can have accent color variations within the system constraints

### Spacing
- **8px grid:** All spacing values are multiples of 8px
- **Content density:** Favor information density over whitespace. This is a platform for deep reading, not quick scanning.

### Motion
- **Framer Motion** for all animations
- **Principles:** Subtle, purposeful, physics-based. No bounce, no overshoot, no decorative animation.
- **Page transitions:** Fade + slight vertical shift (150-250ms)
- **Micro-interactions:** Scale + opacity on hover/press (100-150ms)
- **Data visualization transitions:** Smooth interpolation, not sudden jumps

## Component Conventions

Components live in `platform/components/`. Follow these conventions:

- **File naming:** PascalCase for components (`WorldCard.tsx`), camelCase for utilities
- **Co-location:** Component-specific styles, types, and tests live alongside the component
- **Server vs. Client:** Default to React Server Components. Add `"use client"` only when the component needs interactivity, browser APIs, or hooks
- **Props:** TypeScript interfaces, not inline types. Export the interface for reuse
- **Composition:** Prefer composition over configuration. Small, focused components that compose well

## Tailwind Configuration

- Config file: `platform/tailwind.config.ts`
- Use design tokens defined in the Tailwind config, not arbitrary values
- Custom utilities for common patterns (e.g., `prose-editorial` for long-form content)
- Responsive breakpoints follow the standard Tailwind defaults
- Dark mode: class-based (`dark:` prefix), not media-query-based

## Data Visualization

- **D3.js** for all data visualizations (world maps, relationship graphs, timeline views)
- **SVG-first:** Use SVG for visualizations, not canvas (accessibility, styling consistency)
- **Responsive:** Visualizations must resize gracefully
- **Accessible:** All charts need aria labels, keyboard navigation where applicable
- **Consistent palette:** Use the world's accent colors, not arbitrary chart colors

## Responsive Design

- **Mobile-first:** Styles start mobile, expand with breakpoints
- **Content reflow:** Dense layouts on desktop, simplified on mobile. Don't just shrink — reorganize
- **Touch targets:** Minimum 44x44px for interactive elements on mobile
- **Reading experience:** Long-form content (stories, world descriptions) must be comfortable to read on all screen sizes

## Accessibility Requirements

- **WCAG 2.1 AA** compliance minimum
- **Color contrast:** 4.5:1 for normal text, 3:1 for large text
- **Keyboard navigation:** All interactive elements must be keyboard-accessible
- **Screen reader support:** Semantic HTML, ARIA labels where needed, meaningful alt text
- **Focus indicators:** Visible focus rings on all interactive elements
- **Reduced motion:** Respect `prefers-reduced-motion` — disable animations when set

## PR Review Checklist (for Design agent)

When reviewing a PR that touches UI:

1. **Check against TASTE.md** — Does the change align with the neo-editorial design direction?
2. **Design tokens** — Are design tokens used instead of arbitrary values?
3. **Component consistency** — Does the new/changed component follow existing patterns?
4. **Server vs. Client** — Is `"use client"` used only when necessary?
5. **Responsive** — Does it work at all breakpoints?
6. **Accessibility** — Color contrast, keyboard nav, screen reader support
7. **Motion** — If animated, does it follow the motion principles?
8. **Dark mode** — Does it look correct in dark mode?
9. **Information density** — Does it maintain the editorial density feel?
10. **Typography** — Correct font usage from the type scale?

Report findings to Ren with specific file paths and line numbers. Propose fixes, don't just flag problems.
