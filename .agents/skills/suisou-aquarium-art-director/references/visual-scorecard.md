# Visual scorecard

Score each broad direction out of 100.

## Hard failure gates

Reject the direction before scoring if:

- it changes security or persistence architecture for visual reasons
- long Korean text is difficult to read
- mobile content is obscured by decoration
- reduced-motion mode loses required state information
- it requires remote runtime assets
- it imitates an identifiable product, brand, artist or artwork
- it represents nonexistent product capabilities
- its identity depends only on color, fish or bubbles

## Scoring

### Product-to-concept integration — 20

- Does the aquatic metaphor explain real product behavior?
- Do search, reasoning, sources and history have distinct expressions?

### Original identity — 15

- Is the interface recognizable as Suisou without reading its logo?
- Would another AI product look wrong inside the same design?

### Information hierarchy — 15

- Is the primary action obvious?
- Can long research answers be read comfortably?
- Are source and status elements subordinate but discoverable?

### Spatial composition — 10

- Is depth created through composition rather than card stacking?
- Are foreground, midground and background clearly controlled?

### Signature interaction — 15

- Is there one memorable transition tied to research?
- Does it remain meaningful without animation?

### Responsive behavior — 10

- Is mobile intentionally composed rather than merely compressed?
- Are controls touch-friendly?

### Accessibility — 10

- Are focus, contrast, labels and reduced motion preserved?
- Is state communicated without color alone?

### Feasibility and performance — 5

- Can it be implemented in Sycamore/CSS/SVG without fragile dependencies?
- Is the animation budget reasonable?

## Threshold

A direction is ready for implementation only when:

- no hard failure gate applies
- total score is at least 80
- product-to-concept integration is at least 15/20
- information hierarchy is at least 12/15
- accessibility is at least 8/10

## Required screenshot matrix

Capture or inspect when applicable:

| State | Desktop light | Desktop dark | Mobile |
| --- | --- | --- | --- |
| Welcome | required | required | required |
| Existing conversation | required | required | required |
| Research progress | required | required | required |
| Streaming answer | required | required | required |
| Sources panel | required | required | required |
| Settings/API key | required | required | required |
| Error/cancelled | required | one theme minimum | required |
| Long Korean answer | required | required | required |
| Reduced motion | one theme minimum | one theme minimum | required |
