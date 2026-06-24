---
name: frontend-reviewer
description: Review frontend implementations for visual quality, UX behavior, accessibility, responsiveness, state handling, and consistency with an existing design system. Use when Codex is asked to review a website, web app, static prototype, React/Vue/Svelte UI, CSS, HTML, component implementation, or recent frontend changes before shipping.
---

# Frontend Reviewer

## Overview

Use this skill to perform a practical frontend review. Prioritize user-visible defects, broken flows, accessibility gaps, responsive layout problems, and inconsistencies with the local design system over style preferences.

## Review Workflow

1. Identify the target surface: routes, components, changed files, and the expected user workflow.
2. Read local design guidance first when present, especially `design-system.md`, component docs, or existing CSS tokens.
3. Inspect implementation files before judging visuals. Prefer `rg --files`, `rg`, and focused file reads.
4. Run the app when feasible and verify in a browser or equivalent rendered environment.
5. Check desktop and mobile layouts. Use representative widths such as 1440px, 1024px, 768px, and 390px when tooling allows.
6. Exercise core interactions: navigation, forms, modals, loading states, empty states, success states, and error states.
7. Report findings first, ordered by severity, with file and line references when available.

## Review Criteria

### Visual and Design System

- Verify colors, typography, spacing, radius, shadows, and component patterns match the local design system.
- Flag inconsistent button styles, card nesting, oversized headings in dense panels, weak hierarchy, and one-off styling that should reuse existing tokens.
- Check whether the first viewport communicates the product, object, workflow, or task clearly.
- Flag decorative visuals that obscure inspection of the real product state.

### Layout and Responsiveness

- Check for overflow, clipped text, overlapping UI, unstable grid sizing, and buttons or chips that cannot fit their labels.
- Verify fixed-format elements have stable dimensions or responsive constraints.
- Confirm mobile layouts collapse intentionally and preserve primary actions.
- Check sticky panels, side navigation, modals, and toolbars at small widths.

### Interaction and State

- Verify primary actions are obvious and unique per view.
- Check hover, focus, active, disabled, loading, empty, success, and error states.
- Confirm forms show clear validation feedback and do not rely only on browser defaults.
- Verify state changes update all related UI surfaces consistently.
- Confirm destructive, privacy-sensitive, or sharing actions require clear confirmation.

### Accessibility

- Check visible labels for form controls.
- Check keyboard focus visibility and logical tab order where tooling allows.
- Verify clickable targets are at least 44px high for core controls.
- Flag color-only status indicators; state must also be communicated in text.
- Check modal semantics and focus behavior when relevant.
- Verify contrast concerns where text is low contrast or placed over complex backgrounds.

### Frontend Code Quality

- Prefer existing component, utility, and CSS patterns over new ad hoc abstractions.
- Flag duplicated markup or styles only when it creates real maintenance risk.
- Check that JS selectors still match the DOM and that required IDs/classes/templates exist.
- Verify route or state initialization cannot crash when optional elements are absent.
- Check that assets, imports, and module paths resolve in the local setup.

## Reporting Format

Use a code-review stance:

- Start with findings, ordered by severity.
- For each finding include: severity, file/line, user impact, and a concrete fix direction.
- Add open questions only when they affect correctness or shipping risk.
- Add a short verification note listing what was run or what could not be run.
- If there are no issues, say so clearly and mention residual risk or untested areas.

Keep summaries brief. Do not bury serious issues after praise or implementation notes.

## Severity Guide

- **High**: Broken primary workflow, runtime crash, inaccessible core action, severe mobile layout failure, privacy leak, or impossible form completion.
- **Medium**: Important state missing, confusing navigation, significant responsive defect, inconsistent design-system use that affects comprehension, or incomplete validation.
- **Low**: Minor visual inconsistency, polish issue, copy ambiguity, or maintainability concern with limited user impact.

## Verification Expectations

When feasible:

- Run syntax/type/build checks used by the project.
- Start the local dev server or static server.
- Inspect rendered pages in a browser.
- Capture screenshots for visual review when browser tooling is available.
- Test at least one happy path and one non-happy state for the primary workflow.

If browser verification is unavailable, state that limitation and rely on static DOM/CSS/JS checks.
