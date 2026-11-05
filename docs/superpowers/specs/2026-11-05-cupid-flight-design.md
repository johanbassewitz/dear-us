# Cupid: gentle flights around the app

## Intent
Make Cupid feel alive. The user chose flying around, with the explicit constraint that he must not obstruct use of the app.

## Approaches
- Stay in place with animated expressions: simpler, but the user wants flight.
- Fly freely across the whole window: lively, but can cross writing areas and controls.
- Take occasional short flights along the outer edges: recommended implementation of the user's flight preference. Keep the center clear and stop near interaction.

## Behavior
- Preserve Cupid's existing appearance, menu, chat, hide action, and drag placement.
- Add gentle banking and wing motion during flight and quiet movement at rest.
- Start from the saved position. Use short journeys along the nearest edge, separated by longer rests; never take shortcuts across the center.
- Pause immediately when the pointer approaches, Cupid receives keyboard focus, his menu or chat opens, or dragging begins. Do not make him evade someone trying to click him.
- Pause while the user types or scrolls. Resume only after an idle interval, so Cupid does not compete with writing or reading.
- Keep him inside the window when it is resized. Dragging sets his new resting position; automatic flight does not repeatedly write preferences.
- Respect the operating system's reduced-motion preference: no autonomous flight or decorative animation.
- Suspend flights while the app is hidden and while Cupid is asleep or hidden.
- Keep animation effects within Cupid's small bounds, without a full-screen layer that intercepts clicks.

## Implementation choices
Use the existing React app and local assets, with a small flight controller and CSS animation. No additional service, network request, or animation dependency is needed. Exact timings and distances are implementation choices, tuned for a calm companion.

## Validation
Tests cover pausing, reduced motion, dragging, resizing, idle resumption, and timer cleanup. Verify the animation visually, then run the complete frontend test suite and production build. Any release uses the established signed GitHub update workflow.
