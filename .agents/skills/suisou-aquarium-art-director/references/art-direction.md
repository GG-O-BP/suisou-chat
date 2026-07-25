# Suisou aquarium visual language

## North star

Suisou is a quiet bioluminescent research observatory.

It is not a normal AI chat interface with blue decoration.
The aquarium world must explain research, evidence, depth and progress.

## Emotional qualities

Prioritize:

- calm
- concentration
- curiosity
- scientific precision
- organic life
- restrained wonder
- trustworthiness

Avoid:

- spectacle without meaning
- childishness
- novelty-game interfaces
- cyberpunk aggression
- visual noise
- luxury glass effects without hierarchy

## Spatial system

Use environmental depth rather than many independent cards.

- Background: low-contrast water, light and distant organisms
- Midground: navigation, research state and contextual information
- Foreground: readable text and primary controls
- Reading surface: stable and minimally distorted
- Peripheral atmosphere: more expressive than the center reading zone

## Theme character

### Light

A sunlit aquarium gallery:

- mineral off-white surfaces
- shallow aqua light
- soft water shadows
- restrained coral accents
- dark green-blue typography
- generous negative space

### Dark

An abyssal observatory:

- near-black blue-green depth
- dim turquoise instrumentation
- selective bioluminescence
- warm coral only for critical emphasis
- opaque reading surfaces
- controlled halo effects

Do not implement dark mode by simply inverting light colors.

## Functional metaphor

| Product behavior | Aquarium expression |
| --- | --- |
| New question | Load research capsule |
| Quick | Surface-water pass |
| Search | Reef and sonar exploration |
| Deep | Abyssal descent |
| Connecting | Seal and pressure check |
| Searching | Sonar expansion |
| Reasoning | Currents converge |
| Writing | Findings illuminate |
| Source | Numbered specimen |
| History | Dive log |
| Settings | Life-support controls |
| Error | Light or signal degradation |
| Cancel | Controlled ascent |
| Local storage | Private observation tank |

## Signature moment

The primary signature moment is the transition from question to evidence:

1. The question is committed.
2. The composer behaves like a sealed research capsule.
3. Environmental light shifts according to research mode.
4. Progress is represented as descent or exploration, not a generic spinner.
5. Answer text begins to illuminate.
6. Sources appear as restrained numbered specimens.
7. The environment settles when research completes.

This moment must remain legible and understandable in reduced-motion mode.

## Shape language

Prefer:

- observation windows
- pressure seals
- lens rings
- asymmetric editorial composition
- fluid but controlled curves
- narrow telemetry marks
- specimen labels
- depth lines

Avoid using a rounded rectangle as the default answer to every grouping problem.

## Typography

- Keep Korean body copy highly readable.
- Use strong size and weight contrast before decorative type.
- Use editorial serif selectively for atmosphere and major headings.
- Use monospace only for telemetry, indices, model names and technical labels.
- Do not depend on an unbundled font being installed.
- Never place long text over moving or high-detail imagery.

## Asset policy

Use CSS/SVG for:

- UI geometry
- gauges
- sonar
- progress
- icons
- borders
- masks
- particles that need deterministic animation

Use raster generation for:

- atmospheric depth
- organic silhouettes
- caustic textures
- illustrated creatures
- moodboards

Generated assets must not contain interface text, logos, buttons or controls.
