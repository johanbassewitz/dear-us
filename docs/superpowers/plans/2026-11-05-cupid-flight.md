# Cupid Flight Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans. Build and update approved by the user.

**Goal:** An animated Cupid who makes gentle edge flights without disrupting interaction.
**Architecture:** Pure bounded route calculation plus a React flight hook; layered existing artwork supplies wing movement. Keep the existing menu, chat, and saved drag position.
**Tech Stack:** React, TypeScript, CSS, Vitest; no new dependencies.
**Spec:** ../specs/2026-11-05-cupid-flight-design.md

## Constraints and decisions
- Work in the existing checkout on a feature branch; preserve the user's files.
- Stay on the nearest edge; when manually placed centrally, stay there until dragged to an edge rather than cross content.
- Flights span at most 180px, last 4 seconds, and rest for 12 seconds.
- Pause immediately within 90px of the pointer, on focus, drag, open menu/chat, typing/scrolling, reduced motion, sleep, or hidden page.
- Automatic movement never persists settings. Dragging does.

## Tasks
- [ ] 1. Add failing route/hook tests for bounds, no central crossing, pause/resume, reduced motion, hidden page, and cleanup. Implement `flightTarget(position, viewport, direction)` and `useCupidFlight({position, setPosition, paused})` in src/cupid/useCupidFlight.ts. Verify focused tests.
- [ ] 2. Integrate hook into Cupid, freeze on pointer-down/focus, preserve dragging, and add a layered CupidSprite with wing motion and banking styles. Add interaction tests for menu, chat, dragging, hiding, and resizing. Verify focused tests and visually inspect preview.
- [ ] 3. Run full tests and build, get independent review, resolve findings, bump to 1.0.3, commit and push main plus release tag. Monitor GitHub release, verify/download installer, install and read back installed version.

## Review focus
Pointer approaches a moving target; drag begins mid-flight; narrow/resized viewport; active text input without fresh keystrokes; OS motion preference changes live. Tests should cover each.

## Progress
Design approved; implementation and update authorized. Existing 1.0.2 baseline passed 56 tests and build in this session.

Tasks 1 and 2 complete. Final suite: 69 tests pass; typecheck and production build pass. Independent review found an unfocused-window initialization bug, reproduced with a failing test and fixed using persistent focus state. Scroll capture also covers scrollbar/touch scrolling. No remaining definite correctness findings.

Visual preview could not be completed: browser access host unavailable and native-window approval timed out. Wing clipping seams remain a visual verification limitation. Release version: 1.0.3.
