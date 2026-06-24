---
name: ux-designer
description: Design user experience for frontend products, including user flows, information architecture, screen structure, interaction behavior, content hierarchy, form behavior, empty/loading/error/success states, and usability risks. Use when Codex is asked to plan, redesign, critique, or specify the UX of a website, app, dashboard, workflow, onboarding, editor, form, or multi-step frontend feature before implementation.
---

# UX Designer

## Overview

Use this skill to turn product intent into a usable frontend experience. Focus on what users need to understand, decide, and do at each step.

## UX Workflow

1. Identify users, context, and the primary job-to-be-done.
2. Define the smallest complete workflow that reaches user value.
3. Map screens, entry points, transitions, and exits.
4. Establish page hierarchy: primary action, secondary actions, supporting information, and status.
5. Specify states: initial, loading, empty, validation error, system error, success, permission/auth restricted, and offline if relevant.
6. Define copy requirements for labels, helper text, errors, confirmations, and success feedback.
7. Identify usability risks and simplify decision points before suggesting visuals.

## Core Principles

- Prefer task language over technical language.
- Keep one obvious primary action per screen.
- Put controls near the thing they affect.
- Make system state explicit; users should not infer whether work is saved, generating, failed, shared, or private.
- Reduce repeated decisions in long workflows by using defaults, summaries, and progressive disclosure.
- Treat empty states as workflow starts, not blank placeholders.
- Treat error states as recovery paths, not dead ends.
- Protect sensitive data with visible scope, consent, and confirmation.

## Screen Specification Checklist

For each screen, define:

- Purpose: why this screen exists.
- Primary user action.
- Secondary actions.
- Required data.
- Main sections in visual order.
- Navigation in and out.
- Loading, empty, error, and success states.
- Validation and confirmation behavior.
- Permission or privacy constraints.
- Mobile behavior and content priority.

## Interaction Patterns

### Forms

- Group fields by user mental model, not backend schema.
- Place required fields in the main path; defer advanced or optional details.
- Validate early enough to prevent wasted work.
- Error text must say what is wrong and how to fix it.
- Confirmation copy should explain consequences, especially for sharing, publishing, deleting, or sending data.

### Lists and Detail Views

- Provide search, filters, sort, and empty states only when they match real user tasks.
- Preserve context when moving from list to detail and back.
- Show enough metadata for confident selection.
- Avoid opening detail pages when an inline preview or drawer is enough.

### Creation Workflows

- Show progress for multi-step flows.
- Make draft/save behavior visible.
- Provide a review step before irreversible or externally visible actions.
- Make generated or automated output editable when user trust matters.

### Dashboards

- Start from decisions and actions, not metrics.
- Prioritize exceptions, pending work, and recent activity over decorative charts.
- Make every metric explainable and actionable.

## Output Format

When asked to produce a UX design, use:

- **UX Summary**
- **Users and Jobs**
- **Primary Flow**
- **Screens**
- **State Matrix**
- **Content and Copy Notes**
- **Accessibility and Privacy Notes**
- **Open UX Risks**

When reviewing UX, lead with findings ordered by severity, then provide concrete redesign directions.
