---
name: suisou-aquarium-art-director
description: Direct, critique, implement, and visually validate Suisou's aquarium-themed UI. Use for redesigns, styling, layout, animation, illustration, generated visual assets, responsive behavior, design-system work, screenshot review, visual QA, or any request to make the interface more creative, distinctive, polished, aquatic, or immersive.
---

# Suisou Aquarium Art Director

Create an original, usable interface rather than decorating a generic SaaS
layout with aquatic colors.

## Read first

Read:

1. `AGENTS.md`
2. `src/app.rs`
3. `src/icons.rs`
4. `src/models.rs`
5. `styles.css`
6. `index.html`

For broad design work, also read:

- `references/art-direction.md`
- `references/visual-scorecard.md`

Inspect existing assets and screenshots when available.

## Classify the task

Use one of four modes:

- **Audit:** diagnose the current design without editing.
- **Direction:** develop art directions without editing.
- **Implementation:** implement an approved or delegated direction.
- **Critique:** inspect screenshots or a diff and propose targeted corrections.

Honor explicit read-only or no-edit requests.

For a small visual fix, do not force a full art-direction exercise; make the
smallest consistent change.

## Inventory the real interface

Before broad design work, inventory:

- components
- interaction hierarchy
- theme tokens
- breakpoints
- empty, loading, success and error states
- panels and overlays
- long-form content behavior
- keyboard and accessibility behavior
- existing motion
- available offline assets

Distinguish visual problems from functional or architectural problems.

## Generate directions

When no direction has been selected, create three structurally different
directions.

Each direction must differ in:

- spatial metaphor
- silhouette and composition
- material language
- lighting
- typography
- navigation treatment
- progress representation
- source/evidence representation
- signature interaction

Do not count palette-only or radius-only variations as separate directions.

For each direction provide:

1. Name
2. One-sentence visual thesis
3. Emotional goal
4. Spatial composition
5. Material and lighting system
6. Typography hierarchy
7. Functional metaphor mapping
8. Three signature interactions
9. Required CSS/SVG/raster assets
10. Accessibility and performance risks
11. Implementation scope
12. Reason it might become generic

Score each direction using `references/visual-scorecard.md`.
Recommend one.

If the user delegated the decision, choose the highest-scoring viable direction.
Otherwise wait for selection before a large implementation.

## Apply the anti-generic test

Reject or revise a direction when:

- replacing "Suisou" with another AI product name still works unchanged
- the aquarium identity depends only on teal, gradients, fish or bubbles
- most sections are interchangeable rounded cards
- the design loses all identity when motion is disabled
- decoration competes with Korean long-form reading
- mobile is only a compressed desktop composition
- sources and research progress look like generic badges or steppers

## Define the system before broad implementation

Create or identify:

- semantic color tokens
- light and dark environmental layers
- surface opacity hierarchy
- type scale
- spacing scale
- corner language
- border and highlight language
- depth rules
- motion durations and easing
- state-specific lighting rules
- asset budget

Do not choose arbitrary one-off values for every component.

## Build a vertical slice

For a broad redesign, implement this path first:

1. Welcome state
2. Composer
3. Mode selection
4. Send transition
5. One research-progress state
6. Start of an assistant response
7. One source appearing

Do not redesign every panel until the vertical slice proves the direction.

Preserve behavior, signals, IPC calls and native security boundaries.

## Use image generation deliberately

This repository does not use `OPENAI_API_KEY`.

Use `$imagegen` only when the current session actually exposes the built-in
`image_gen` tool. The presence of the `$imagegen` skill or an enabled feature
flag does not prove that the tool is available.

Never:

- request `OPENAI_API_KEY`
- invoke or propose the OpenAI Image API CLI fallback
- block a design task because raster generation is unavailable

When built-in `image_gen` is unavailable, continue with:

- CSS gradients, masks, filters and layered backgrounds
- repository-native SVG
- deterministic canvas graphics when justified
- user-supplied local assets
- a precise asset brief for a separately configured local generator

Use built-in image generation when available and raster imagery materially
improves the direction.

Good candidates:

- moodboards
- caustic-light textures
- atmospheric depth layers
- distant organic silhouettes
- original aquatic companion illustrations
- non-textural concept frames

Keep UI text and controls out of generated images.

For a project-bound asset:

1. Generate variants.
2. Inspect them.
3. Select one.
4. Move the selected asset into `public/assets/aquarium/`.
5. Use a descriptive filename.
6. Optimize its dimensions and format.
7. Verify it works in light and dark contexts.
8. Ensure the app does not depend on a remote source.

Prefer CSS or SVG when an element must scale precisely, inherit theme colors,
remain accessible, or match `src/icons.rs`.

## Motion design

Tie motion to meaning:

- idle: subtle breathing or drifting
- connecting: pressure seal or initial descent
- searching: expanding sonar or lateral exploration
- reasoning: converging currents or coordinated lights
- writing: progressive illumination
- completed: restrained settling
- error: degraded or unstable light
- cancelled: controlled ascent

Use transform and opacity first.
Avoid rapid flicker, large animated blur and motion behind long-form text.
Provide a reduced-motion equivalent for every meaningful transition.

## Visual validation

Render affected states whenever tools permit.

Compare:

- desktop and mobile
- light and dark
- normal and reduced motion
- empty and content-heavy
- idle and active research
- source/settings panels
- keyboard focus
- error and disabled states

Use `view_image` for saved screenshots.

When Browser is available, review the rendered interface, not only the source
code. When Browser is unavailable, ask for screenshots only if local rendering
cannot provide them.

After each screenshot pass, identify:

1. the strongest distinctive decision
2. the most generic remaining area
3. the first element to remove
4. the hierarchy problem with the highest impact
5. any accessibility or performance regression

Make targeted revisions rather than restarting the design.

## Finish

Before completion:

- run the required repository checks
- inspect the full diff
- verify assets are local
- verify no secrets or remote dependencies were introduced
- report tested viewport/state combinations
- report untested visual states
- summarize the selected visual thesis and signature interaction
