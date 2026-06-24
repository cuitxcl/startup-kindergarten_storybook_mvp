---
name: frontend-architect
description: Plan frontend architecture for web apps and sites, including route structure, component boundaries, state ownership, data fetching, API integration, form architecture, styling strategy, performance, accessibility foundations, test strategy, and implementation sequencing. Use when Codex is asked to design or refactor frontend structure before coding, choose component/data/state patterns, or make a complex UI maintainable.
---

# Frontend Architect

## Overview

Use this skill to make frontend implementation structurally sound before or during build work. Prefer the repository's existing framework, conventions, and component patterns unless there is a concrete reason to change them.

## Architecture Workflow

1. Inspect the current app structure, framework, routing, styling, state management, and build tooling.
2. Identify the feature's screens, data dependencies, mutations, and user-visible states.
3. Decide component boundaries around ownership and change frequency, not visual boxes alone.
4. Define state ownership: URL state, server state, local UI state, form state, derived state, and persisted draft state.
5. Define data contracts needed by the UI and how loading/error/retry behavior is represented.
6. Choose styling strategy that matches the repo: design tokens, CSS modules, utility classes, component library, or existing global CSS.
7. Plan accessibility, responsive behavior, and keyboard interaction foundations early.
8. Sequence implementation into low-risk milestones with verification at each step.

## Component Boundary Rules

- Keep page components responsible for layout, data orchestration, and route-level state.
- Keep feature components responsible for one workflow or domain object.
- Keep primitive components responsible for reusable controls only when reuse is real.
- Avoid abstractions that only wrap a single use case.
- Avoid mixing data fetching, mutation side effects, and presentation inside deeply nested leaf components unless the existing codebase already does so consistently.
- Keep generated content, form drafts, and persisted records clearly separated.

## State Ownership Guide

- URL state: route, selected tab, filters, search, pagination, shareable state.
- Server state: records from APIs, generation tasks, permissions, usage data.
- Form state: user-editable inputs before submit.
- Local UI state: open modals, selected cards, temporary previews, disclosure panels.
- Derived state: labels, summaries, progress text, computed badges.
- Persistent client state: drafts, recent local history, unsent changes.

Do not duplicate source-of-truth state unless there is a specific synchronization plan.

## Data and API Planning

For each screen or workflow, specify:

- Read models needed for initial render.
- Mutations and their optimistic or pessimistic behavior.
- Loading, empty, error, retry, and success states.
- Validation rules shared with the backend.
- Authorization and visibility rules.
- Cache invalidation or refresh behavior.
- Long-running task handling when jobs are asynchronous.

## Styling and Design System

- Reuse local tokens, layout primitives, and established component classes.
- Keep responsive behavior with explicit grid/flex constraints.
- Avoid one-off spacing/color values when a token exists.
- Keep interaction states consistent across buttons, links, fields, tabs, cards, and modals.
- Treat design-system drift as architecture debt when it creates duplicated component behavior.

## Performance and Reliability

- Keep first render lightweight for route shells and dashboards.
- Defer expensive previews, charts, or media when they are not needed immediately.
- Avoid unnecessary full-page rerenders for local state changes.
- Make long-running operations cancelable, retryable, or visibly resumable when the domain requires it.
- Ensure route changes, reloads, and failed requests do not lose important user work.

## Testing Strategy

Match tests to risk:

- Unit tests for pure transformations, validators, reducers, and formatters.
- Component tests for forms, conditional rendering, and stateful controls.
- Integration or browser tests for primary workflows, routing, modals, and responsive critical paths.
- Snapshot or visual checks only when they protect meaningful layout contracts.

## Output Format

When asked for an architecture plan, use:

- **Architecture Summary**
- **Routes and Screens**
- **Component Boundaries**
- **State Model**
- **Data/API Dependencies**
- **Styling Strategy**
- **Accessibility and Responsive Foundations**
- **Implementation Phases**
- **Verification Plan**
- **Open Technical Risks**

When reviewing architecture, lead with risks and concrete refactor directions before summarizing.
