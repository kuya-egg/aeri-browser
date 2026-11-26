# Aeri Phase 12 — Appearance and customisation

> Design written by GPT-6.2 Sol (openai-codex/gpt-6.2-sol, xhigh) on
> 2026-11-20 at the owner's request, from a brief on Aeri's current state and
> stage-1 rules. Competitor claims marked (verify) are checked in
> `docs/research/arc-zen-helium-customisation.md`. Owner decisions and the
> build plan that override this text are in
> `.monozukuri/specifications/phase-12-appearance-and-customisation.md`.

**Default:** Aeri opens to the page, edge to edge. There is no permanent 6 pt gutter, sidebar or toolbar. The left edge reveals the sidebar; the top edge reveals navigation and window controls. `⌘L` opens a floating address bar. The default theme follows macOS. A person who wants a conventional browser can pin the chrome and put tabs on top without changing browser or profile.

All dimensions below are macOS logical points. **Chrome** means Aeri-owned UI, not the Chromium page. Settings changes are committed across all affected windows; an uncommitted theme-editor draft is a preview, not a settings change.

## 1. Competitive inventory

This is a feature-level inventory, not a claim to have tested the latest builds. Arc Search refers primarily to iOS; do not infer feature parity on Android. Zen’s stable and Twilight releases, and Helium releases, can differ. **(verify)** marks a capability, version, or exact control that needs a current-build check. In slice 1, record versions and screenshots for the shipping comparison; do not use unverified items in marketing.

| Capability | Arc for macOS / Arc Search | Zen Browser | Helium | Aeri Phase 12 |
|---|---|---|---|---|
| Light, dark, system appearance | Arc has browser appearance controls; exact interaction with Space themes **(verify)**. Arc Search follows device appearance; independent override **(verify)**. | Light/dark/system chrome; separate website appearance controls inherited from Firefox **(verify)**. | Chromium-style appearance controls; exact system/toolbar options **(verify)**. | **Exceed:** paired palettes from one theme recipe, with an explicit app-wide override. Web behaviour is separately documented. |
| Per-Space colour and gradient | Arc Space themes have an interactive colour canvas, including multicolour treatments. Exact limits **(verify)**. | Workspace themes with a colour/gradient picker; exact generator controls **(verify)**. | No equivalent Space theme known **(verify)**. | **Exceed:** per-Space solid or 2–5-stop gradient, exact positions/angle, hex entry, paired contrast previews and file export. |
| Texture, transparency and visual intensity | Arc theme texture/grain and opacity/intensity controls; exact names and bounds **(verify)**. | Gradient texture, opacity and browser transparency options vary by build **(verify)**. | No equivalent known **(verify)**. | **Match** static grain and adjustable opacity; native material is conditional on the GPUI spike. **Exceed** by keeping text contrast independent of what is behind the window. |
| Theme presets, generation, source colour | Arc’s picker provides guided colour exploration; randomisation and eyedropper **(verify)**. Arc Search theme generator not known. | Gradient generation and workspace presets; randomisation and eyedropper **(verify)**. | Chromium colour presets or custom colour availability **(verify)**. | **Exceed:** 10 offline presets, seeded Shuffle, exact controls, draft undo/redo, optional system colour sampling. |
| Theme sharing and galleries | Arc can share Spaces; portable appearance-only theme files not known **(verify)**. | Zen Mods gallery; Firefox lightweight-theme compatibility and standalone workspace-theme sharing **(verify)**. | Chrome Web Store theme compatibility **(verify)**. | **Exceed** for offline, inspectable appearance-only files. **Do not match** remote galleries or execute extension theme packages as Aeri themes. CEF extension compatibility does not make GPUI chrome themeable by Chrome. |
| Arbitrary chrome changes | No arbitrary Arc chrome CSS known; Arc Boosts address websites, not chrome. | Zen Mods and `userChrome.css` can reshape chrome beyond built-in controls. | No equivalent known **(verify)**. | **Do not match** code/CSS injection. Replace common mods with supported toolbar slots, tab layouts, density, treatments and declarative theme files. |
| Native theme details | Arc styles its own chrome; app icon choices by release **(verify)**. Arc Search alternate icons and tinted icon support **(verify)**. | Chrome details can be altered through mods; built-in radius/font/icon control **(verify)**. | Mostly conventional Chromium presentation; exact options **(verify)**. | **Match or expose** chrome radii, type size, limited heading style and motion strength. **Not in Phase 12:** Dock/app icons, custom fonts, image themes or custom traffic-light skins. |
| Side, horizontal and collapsed tab layouts | Side tabs; no supported conventional horizontal tab strip known **(verify)**. | Single Toolbar, Multiple Toolbars and collapsed sidebar layouts; official horizontal-tab availability **(verify)**. | Horizontal tabs are the baseline; vertical-tab availability **(verify)**. | **Exceed Arc’s known layout range:** side or top tabs. **Match** Zen’s compact intent with independently hideable chrome; no icon-only collapsed tab rail this phase. |
| Sidebar side and width | Resizable sidebar; supported right-side placement **(verify)**. | Resizing and right-side placement; exact limits **(verify)**. | Sidebar/tab rail options **(verify)**. | **Match:** left/right, adjustable width, keyboard resizing. Applies to the library drawer in top-tab mode too. |
| Hidden chrome, edge reveal, fullscreen | Toggleable sidebar with edge reveal; optional toolbar/fullscreen details **(verify)**. Arc Search minimises mobile chrome while browsing. | Compact Mode can hide sidebar, toolbar, or both; hover-reveal options vary **(verify)**. | Standard fullscreen; hover-reveal/compact controls **(verify)**. | **Exceed:** edge-to-edge default, explicit compact targets, keyboard-accessible reveal, no hidden gutter. Does not copy touch or scroll gestures onto desktop. |
| Floating and attached address bar | Floating command/address UI on invocation; optional visible toolbar placement **(verify)**. Arc Search has mobile search UI, not a customisable desktop toolbar. | Floating URL bar modes: keyboard/New Tab only, always, or disabled **(verify exact labels)**. | Attached omnibox; floating alternative **(verify)**. | **Exceed:** all three modes, attached placement, two floating anchors, separate palette. |
| Toolbar contents and rearrangement | Optional toolbar; exact button hide/reorder controls **(verify)**. | Firefox-style toolbar customisation plus Zen layouts; available slots vary **(verify)**. | Chromium toolbar/extension pinning controls **(verify)**. | **Match** with an accessible, bounded slot editor for Aeri buttons. **Do not match** arbitrary widgets or moving mandatory trust UI. Extension action buttons are only eligible if already exposed by Aeri. |
| Favourite, pinned and ordinary tab presentation | Favourite icon grid, pinned tabs, folders, shared favourites across Spaces within a profile. Rename and icon overrides **(verify)**. | Essentials, pinned tabs, workspace placement, folders, and icon/label overrides vary **(verify)**. | Conventional pinned tabs and bookmarks UI; exact display switches **(verify)**. | **Match** grid/rows, icon-first top pins and separate pinned grouping. **Not included:** new folder, rename, per-tab custom icon or auto-archive models. Those change tab data or lifecycle. |
| Workspace/profile identity | Named, coloured, icon-bearing Spaces; separate profiles. | Workspaces, icons, colours and Firefox container integration; exact profile UX **(verify)**. | Profile identity and colour choices **(verify)**. | **Exceed** on scope: app → profile → Space theme, with separately editable identity colours. No new container model. |
| Shortcut customisation | Arc provides shortcut remapping; exact command coverage **(verify)**. | Built-in shortcut editor; exact scope and conflict UX **(verify)**. | Standard Chromium shortcuts; general remapping **(verify)**. | **Exceed:** all Aeri menu/palette commands are bindable, with named conflicts and an atomic swap. |
| Browser menus, native appearance and density | Native menus alongside custom chrome; manual density choices **(verify)**. | Firefox density controls/mods; current density choices **(verify)**. | Standard Chromium UI conventions; density setting **(verify)**. | **Match/exceed:** three measured densities, accessible type scaling, AppKit-native context menus. No theme painting inside `NSMenu`. |
| Website appearance, zoom and recolouring | Arc Boosts recolour or modify sites, including custom CSS/JS in supported versions **(verify exact scope)**. Arc Search offers reader-style presentation such as Browse for Me; Reader controls **(verify)**. | Site scheme, zoom and Reader View; extensions for further changes. | Chromium zoom, dark-page experiments/extensions; exposed settings **(verify)**. | **Match** theme-to-site colour *preference* subject to CEF validation; retain existing page controls/extensions. **Do not add** Boosts, Reader, page rewriting, or a per-site forced-dark engine in this phase. |
| New Tab artwork, mobile wallpaper and presentation | Arc’s floating New Tab flow; Arc Search backgrounds/alternate tab presentation **(verify)**. | New Tab/Home customisation from Firefox and mods; exact defaults **(verify)**. | New Tab background/customisation capabilities **(verify)**. | **Do not add** page wallpaper or a custom New Tab document. The launch surface is Aeri chrome and uses the theme. |
| Split, picture-in-picture and auxiliary browsing surfaces | Arc Split View, Peek/Little Arc, mini-player surfaces. | Split View, Glance, Firefox Picture-in-Picture; exact options **(verify)**. | Relevant split/PiP features **(verify)**. | **Not Phase 12.** Restyle existing Aeri windows, not invent new browsing surfaces or alter CEF tab ownership. |
| Lightweight operation | No performance claim here. | No performance claim here. | Lean Chromium baseline. | **Benchmark, not a parity claim:** no idle animation/sampling; measure Aeri before/after and compare with Helium on the same machine and workload. |

## 2. Design principles and differentiators

1. **The page gets the whole window by default.** Transient reveals overlay it; only a pinned region or a deliberate resize changes the viewport.
2. **Make one colour decision, get two usable appearances.** One to three colours are enough; the editor explains adjustments and previews both appearances before saving. Advanced gradient controls don't require sacrificing readability.
3. **Make the common mods first-class, and keep them composable.** Layout, buttons, keymap and appearance are separate. Trying a theme cannot move a button, bind a key or change edge-reveal behaviour.
4. **Make customisation portable without making it executable.** A theme is a small local file of colour and drawing choices. No download-on-open, scripts, remote gallery, page capture in the background, or file references.

## 3. Theme system

### 3.1 Tokens and rendering contract

Use a typed `ResolvedTheme` in Rust. GPUI views consume semantic tokens; do not replace 80 inline colours with 80 view-specific aliases. Use the same tokens for secondary windows and overlay children. Values below are the **Aeri** preset in sRGB; `#RRGGBBAA` uses alpha last.

| Token | Light | Dark | Use |
|---|---|---|---|
| `surface.frame` | `#F2F3F5` | `#17191D` | Frame and behind-card colour |
| `surface.sidebar` | `#F7F8FA` | `#202329` | Sidebar/drawer |
| `surface.strip` | `#F2F3F5` | `#191C21` | Navigation strip and top tab lane |
| `surface.panel` | `#FFFFFF` | `#23262D` | Settings, Library, editors |
| `surface.raised` | `#FFFFFF` | `#2A2E36` | Floating bar, palette, suggestion surface |
| `surface.field` | `#E9ECF0` | `#171A1F` | Attached address field and inputs |
| `surface.inset` | `#EFF1F4` | `#1B1E24` | Inset groups, preview wells |
| `surface.tooltip` | `#272B32` | `#EEF0F4` | Aeri tooltip background |
| `text.primary` | `#16191F` | `#F4F5F7` | Labels, page host, selected row titles |
| `text.secondary` | `#454D59` | `#C1C7D0` | Inactive titles, secondary labels |
| `text.tertiary` | `#56606E` | `#ABB3BF` | Hints, shortcut glyphs, metadata |
| `text.disabled` | `#757D88` | `#858D99` | Genuinely disabled controls only |
| `text.onTooltip` | `#FFFFFF` | `#17191D` | Tooltip text |
| `icon.primary` / `icon.muted` | `#303743` / `#56606E` | `#E1E5EC` / `#ABB3BF` | Meaningful glyphs; don’t use decorative border colours |
| `accent.solid` | `#244FB7` | `#A3B9FF` | Filled accents, active indicators |
| `accent.onSolid` | `#FFFFFF` | `#151D33` | Text and glyphs on solid accent |
| `accent.text` | `#21489F` | `#B7C8FF` | Accent links/text |
| `accent.tint` | `#E3EAFF` | `#303D60` | Tinted badge/group fill |
| `state.hover` | `#151C2B0C` | `#FFFFFF0D` | Over opaque base |
| `state.pressed` | `#151C2B17` | `#FFFFFF18` | Over opaque base; replaces hover |
| `state.selected` / `state.selectedHover` | `#DFE7FA` / `#D6E1F9` | `#303D60` / `#384970` | Active tab, palette result, selected list item |
| `state.selectionText` / `state.onSelectionText` | `#244FB7` / `#FFFFFF` | `#A3B9FF` / `#151D33` | Text selection in Aeri fields |
| `border.separator` | `#D9DDE4` | `#393E47` | Decorative divider, 1 device pixel |
| `border.control` | `#697382` | `#919BAA` | Where the boundary is needed to identify the control |
| `border.card` | `#171D2A29` | `#FFFFFF25` | Decorative page edge when inset |
| `shadow.card` | `#14203720` | `#00000060` | Offset `(0, 2)`, blur 8 |
| `shadow.overlay` | `#14203738` | `#00000085` | Offset `(0, 8)`, blur 24 |
| `focus.ring` / `focus.keyline` | `#244FB7` / `#FFFFFF` | `#B7C8FF` / `#17191D` | 2 pt ring with 1 pt contrasting keyline |
| `status.danger` / `status.dangerTint` | `#A21C30` / `#FCE6E9` | `#FFB1BB` / `#49252D` | Error/destructive text, tint |
| `status.warning` / `status.warningTint` | `#774600` / `#FFF0D4` | `#FFD18E` / `#49371F` | Warning text, tint |
| `status.success` / `status.successTint` | `#146341` / `#E4F4EB` | `#9CE2B8` / `#214032` | Success text, tint |
| `effect.scrim` | `#11182738` | `#00000055` | Modal GPUI scrim only; never on ordinary edge reveal |
| `effect.material` / `effect.blurFallback` | `none` / `0 pt` | `none` / `0 pt` | Optional AppKit sidebar material; no GPUI blur of CEF content |
| `effect.grainAmount` | `0` | `0` | Static decorative grain |
| `effect.themeBackdrop` | `default` | `default` | Default, solid or gradient recipe |

**Paint order:** base → decorative treatment if present → semantic surfaces → interaction fill → text/icons → focus ring. Selection takes precedence over hover; selected-hover has its own token. Do not stack hover, pressed, selection and grain underneath a label. Destructive and trust roles are not raw user colours. Badges use `accent.onSolid` or a contrast-checked neutral ink, never white by assumption.

### 3.2 Recipe, colour derivation and geometry

A recipe contains `base`, background, accent, grain, material/opacity, page radius, density, typography and motion. `base` is `system`, `light` or `dark`; it does not prevent rendering either variant in preview. Both palettes are resolved from the same recipe.

- **Background:** `default` uses the reference surfaces; `solid` uses one seed; `gradient` accepts 2–5 sRGB stops from 0% to 100%. Angle is 0° toward the top, 90° toward the right; stops increase along that vector. Interpolate mapped stops in **linear sRGB**. The quick picker uses one colour for a solid, or two/three evenly spaced stops for a gradient.
- **Grain:** 0–12 strength. Draw one fixed, bundled 64×64 one-channel tile, zero mean, tiled at 1 pt per texel. Strength 12 is a maximum ±0.012 linear-light channel offset; clamp channels. Never animate or regenerate it. Grain is decorative only.
- **Material:** `none` or `sidebar`. `opacity` is 85–100% of the tinted decorative background over AppKit material. This does not lower the NSWindow’s opacity or reveal the website through a control. The fallback is the same treatment at 100% opacity. No live blur, wallpaper reads or CEF texture sampling. See slice 5 for the release gate.
- **Radius:** page card 0–14 pt, default 10. Attached fields, overlay surfaces and tabs use the fixed control radii in §8. Edge-to-edge page radius is always 0. A 10 pt picture frame around a square CEF window is not acceptable: use actual tested clipping or fall back to a square page edge.
- **Typography:** `standard` or `large` (+1 pt on Aeri UI labels and input text, not on numeric badge or native menu metrics). Use system text weights Regular, Medium and Semibold only. `system` or `rounded` for headings only; rounded falls back to standard system design if unavailable. URLs and origin text always use standard system text. Resource counters and hex input may use system monospaced text. No imported fonts.
- **Density:** Compact, Standard or Spacious. Density is visual geometry, not a reveal mode. Autohide timing is unchanged. **Motion:** `standard`, `subtle` or `off`; applies only to Aeri chrome transitions in §5.

**Reference resolver.** Convert tagged sRGB input to OKLCH (D65). When converting back, gamut-map by reducing chroma at fixed lightness and hue; don't clip channels to force a hue into gamut.

1. For each decorative stop, with input lightness `L`, map to `0.86 + 0.11L` for Light or `0.20 + 0.14L` for Dark. Cap chroma at `0.075` Light, `0.095` Dark. Preserve hue. This decorative treatment is used on the frame, strip and sidebar when those regions exist; it does not tint the page.
2. For a custom background, mix 12% of the mapped midpoint into **opaque** `frame`, `sidebar`, `strip`, `panel`, `raised`, `field` and `inset` base surfaces in linear sRGB. Use these solids as control/group backplates on top of the treatment. In a gradient, keep decorative areas visible between groups; put labels, focus contours and list hit rects on predictable surfaces, not directly on noisy, translucent pixels. Use the same rule if macOS material is active.
3. Accent seed order is in §3.3. If a recipe has no explicit accent, use the first background seed with `C ≥ 0.025`, else reference blue. For non-reference accents start at OKLCH `L=.48` Light / `.80` Dark with `C=min(seed.C,.17)`. The seed contributes hue and chroma, not unrestricted lightness. Choose on-solid ink from the reference dark ink and white, taking the higher contrast. Derive `accent.text` from solid; start `accent.tint` at a 12% solid-accent mix over `surface.sidebar`, `state.selected` at 16%, and `selectedHover` at 21%. Text selection uses the solid/on-solid pair. A reference background with no accent override uses the table verbatim.
4. Audit the **actual composited pairs**, including hover/pressed, dropdown selection, status tints and accent buttons. Use WCAG 2.2 relative luminance in linear sRGB (not an OKLCH distance); require **4.5:1 for all readable text** and **3:1 for meaningful icons, boundaries and focus indication**. For an accent foreground that fails, move lightness by `.01` toward the higher-contrast end, gamut-mapping at each step. Use the first passing candidate. For a failed neutral or semantic pair, reduce custom tint in `.02` steps down to zero; use the tested, opaque reference pair if needed. For emphasis that still fails, use the reference accent pair. If a vibrant fill can't meet a focus/boundary target, use a contrasting keyline or opaque surround; never drop the indicator. Keep a test matrix of component/token pairs rather than assuming all foregrounds can sit on all tokens.
5. Apply OS accessibility overrides last. In Increase Contrast, make structural dividers and required boundaries at least 3:1, use opaque backplates, and target 7:1 for ordinary text where possible; AA is still the hard floor. Reduce Transparency forces opaque rendering. Reduce Motion always wins over recipe motion. The editor retains the original seed and shows an **Adjusted for contrast** note with the rendered swatch when it differs; it never silently rewrites the recipe.

### 3.3 Scope and inheritance

| Element | Resolution |
|---|---|
| Theme reference | `app default → profile override → Space override`. An override selects a **complete** recipe, not a partially merged gradient. `Inherit` removes the override. **Edit Space Theme…** starts a copy of the effective recipe if the Space inherits; it never edits the parent implicitly. |
| Base appearance | App setting `Follow Theme` (default), `System`, `Light`, `Dark`. The last three override the base of every recipe. `Follow Theme` uses the effective recipe’s `base`; Aeri’s bundled default uses `system`. Set effective AppKit appearance per Aeri window/overlay where supported. |
| Accent | **Use Space Colour in Chrome** → explicitly assigned Space recipe’s accent (including a derived accent) → profile accent override → effective recipe’s accent. Profile badge and Space identity dot always retain their own identity colours. A profile can use its identity colour as its chrome accent independently of theme. |
| Density | Explicit profile density override wins over recipe density. Other layout settings are never inherited from a recipe. |
| Browser and children | Frame, strip, sidebar, page edge/shadow, floating address bar and palette use the active Space’s resolved theme. Capture that context when creating a popup. If the Space changes, dismiss transient address suggestions/menu before moving focus or showing a different theme. |
| Native menus and window controls | Keep `NSMenu`, checkmarks, key equivalents, font sizes and system highlight. Use the owning window’s effective appearance where AppKit honours it; the system may control appearance for some menus. Do not apply gradients, fonts or custom menu backgrounds. Real AppKit traffic lights keep system colours. |
| Other windows | Library, Resources and Space Editor use the *owning profile* theme, without an active Space override. Space Editor embeds an explicit Space preview. Profile Manager and Settings shell use app theme. Settings’ profile selector changes controls/preview context, not the shell. |

Publish a revisioned settings change on the main UI thread to all affected windows and children in the same main-loop turn. Cache one resolved token set per recipe/appearance/accent/accessibility combination; invalidate only on a changed input. **Save & Apply** writes the recipe and target reference in one transaction if in one DB; across DBs, write the new immutable recipe first, then the reference, then publish after both succeed (an unreferenced recipe is safe to collect). Failed writes keep the old visible state. Observe effective macOS appearance and accessibility display changes at runtime; a system change is resolved immediately, not persisted as a user preference.

### 3.4 Theme editor

One Settings detail view: **Theme Library** on the left; scope and editor on the right. Minimum editor window `900 × 640`; at smaller available displays stack the preview above a scrolling editor. The right pane starts with **Apply to: App Default / [Profile] / [Space]**. Secondary-window actions open the same editor with that scope selected.

- **Preview:** side-by-side Light and Dark miniatures (each min `208 × 152`) with strip, sidebar, active/hover/resting tab, address field, and warning chip. A switch displays system Increase Contrast/Reduce Transparency effects; those switches are preview-only. The miniature is GPUI chrome with a neutral mock page, not a website screenshot. Sliders render only on input, coalesced to a single UI update per display interval.
- **Quick colours:** one to three editable swatches; add/remove with labelled buttons. A swatch opens a colour control plus exact `#RRGGBB` field. **Advanced Gradient** displays the stop track, a keyboard-operable stop list (colour + position), and a 0–359° angle field/dial. Stop endpoints are locked; interior stops cannot cross. **Default Background** restores neutral surfaces without changing the accent.
- **Other controls:** separate Accent (`Derived` or swatch); Base popup; Grain slider 0–12; Material popup with opacity enabled only for `Sidebar`; Page Radius stepper 0–14; Density popup; UI Size, Heading Style and Motion popups. If material is unavailable, retain the authored setting for portability, display “Rendered opaque on this Mac/build,” and preview the fallback.
- **Presets and shuffle:** selecting a preset loads a draft copy. **Shuffle Colours** generates an offline random hue and offsets of 35° and 110° for the additional quick swatches, preserving colour count, positions, angle, material and layout. Use `OKLCH(.62 .12 h)` seeds, gamut-mapped; store sampled values in the draft, never a seed in the file. Undo reproduces prior swatches exactly.
- **Eyedropper:** use the public macOS colour sampler if it works on both CEF pixels and visible desktop on supported OS versions. A deliberate click samples one displayed pixel, converted to sRGB; cancel returns no colour. No persistent permission broker, wallpaper enumeration, full-screen capture or storing a screenshot. If the sampler doesn’t work, ship hex entry and the colour picker without it.
- **Commit:** 50-step local undo/redo; one step per slider drag, text-edit commit, preset or shuffle. **Revert Draft** returns to last saved values; **Use Aeri Defaults** creates a new default-valued draft; **Inherit** is a separate scope action. **Save & Apply** validates and updates affected windows. Closing/switching scope with a dirty draft offers **Save & Apply / Discard / Cancel**. `⌘Z` in a text field edits text before it edits the draft.

### 3.5 Files

Extension **`.aeritheme`**, UTF-8 JSON, **16 KiB maximum** before parsing. No BOM, duplicate keys, trailing data, network references, embedded images or binary textures. Grain is a number, not a resource. Name is displayed as plain text, never interpreted as a path. Built-ins are immutable; imported recipes get a new local UUID. Import does **not** auto-apply or replace a same-name recipe.

Canonical schema (JSON Schema draft 2020-12; application validations below):

```json
{
  "type": "object",
  "required": ["version", "name", "base", "background"],
  "additionalProperties": false,
  "properties": {
    "version": { "type": "integer", "const": 1 },
    "name": { "type": "string", "minLength": 1, "maxLength": 48 },
    "base": { "enum": ["system", "light", "dark"] },
    "background": {
      "oneOf": [
        {
          "type": "object", "additionalProperties": false, "required": ["kind"],
          "properties": { "kind": { "const": "default" } }
        },
        {
          "type": "object", "additionalProperties": false, "required": ["kind", "color"],
          "properties": { "kind": { "const": "solid" }, "color": { "$ref": "#/$defs/color" } }
        },
        {
          "type": "object", "additionalProperties": false, "required": ["kind", "angle", "stops"],
          "properties": {
            "kind": { "const": "gradient" },
            "angle": { "type": "integer", "minimum": 0, "maximum": 359 },
            "stops": {
              "type": "array", "minItems": 2, "maxItems": 5,
              "items": {
                "type": "object", "additionalProperties": false, "required": ["at", "color"],
                "properties": {
                  "at": { "type": "integer", "minimum": 0, "maximum": 100 },
                  "color": { "$ref": "#/$defs/color" }
                }
              }
            }
          }
        }
      ]
    },
    "accent": { "$ref": "#/$defs/color" },
    "grain": { "type": "integer", "minimum": 0, "maximum": 12 },
    "material": { "enum": ["none", "sidebar"] },
    "opacity": { "type": "integer", "minimum": 85, "maximum": 100 },
    "pageRadius": { "type": "integer", "minimum": 0, "maximum": 14 },
    "density": { "enum": ["compact", "standard", "spacious"] },
    "uiSize": { "enum": ["standard", "large"] },
    "headings": { "enum": ["system", "rounded"] },
    "motion": { "enum": ["off", "subtle", "standard"] }
  },
  "$defs": {
    "color": { "type": "string", "pattern": "^#[0-9A-Fa-f]{6}$", "minLength": 7, "maxLength": 7 }
  }
}
```

Missing optional fields mean derived accent, grain `0`, material `none`, opacity `100`, radius `10`, Standard density/type/motion, System headings. Gradient positions must be strictly increasing, first `0`, last `100`. Name must already be trimmed and contain no control characters or bidi formatting controls. The name is the only free text; it has no effect on rendering other than the library label. Export only canonical recipe fields, never IDs, profile/Space names as metadata, author machine information, settings, or matching URLs.

| Validation failure | Message shown under “Theme not imported” |
|---|---|
| Too large, unreadable, invalid encoding | “Theme files must be readable UTF-8 JSON and no larger than 16 KiB.” |
| Syntax, duplicate key, trailing input | “Invalid theme JSON at line {line}, column {column}: {reason}.” |
| Unsupported/missing version | “This file uses theme version {version}. Aeri supports version 1.” / “Theme version is missing.” |
| Unknown key, at any depth | “`{path}` is not an allowed appearance setting.” |
| Wrong type, invalid colour or out-of-range field | “`{path}` must be {expected}.” E.g. “`background.stops[1].color` must be a colour like `#4C71D9`.” |
| Unordered/missing endpoints | “Gradient stops must increase from 0% to 100%, with no duplicate positions.” |
| Invalid name | “Use a theme name of 1–48 characters without surrounding whitespace or control characters.” |
| Save error | “Aeri couldn’t save this theme. Your current theme hasn’t changed.” |

Open with `NSOpenPanel`, bounded read; validate the entire document **before** inserting it. Use duplicate-key detection before typed Rust deserialisation; use `deny_unknown_fields` or equivalent at every object boundary. A malformed file never becomes a partially valid draft and never affects a window. Export through `NSSavePanel`. No clipboard import, auto-discovery of external folders, or remote gallery in Phase 12.

### 3.6 Bundled themes

Unspecified fields use recipe defaults. Presets are examples of the same file format, not renderer special cases.

| Name | Base | Background seeds | Accent seed | Other values |
|---|---|---|---|---|
| Aeri | System | Default | Derived/reference blue | Reference tokens |
| Paper | Light | `#E0CAA8` solid | `#784B32` | Grain 3, radius 8 |
| Slate | Dark | `#66788D` solid | `#6F9AE8` | Compact, radius 8 |
| Tide | System | `#2866C7 → #30AEA2`, 120° | `#197F9E` | Grain 2 |
| Orchard | System | `#6769C1 → #8F649E → #E4A577`, 145° | `#7C55AA` | Rounded headings |
| Ember | Dark | `#854F6B → #CC693E`, 45° | `#D48856` | Grain 4, subtle motion |
| Lichen | System | `#648870 → #B1A563`, 110° | `#4E803E` | Radius 12 |
| Frost | Light | `#A9C7E6 → #D2CAEF`, 155° | `#4169BE` | Spacious, sidebar material, opacity 94 |
| Ink | Dark | `#292B36` solid | `#A6A9C0` | Radius 0, motion off |
| Dusk | System | `#404D89 → #91679E → #D18A94`, 70° | `#825AB0` | Grain 5, radius 12 |

### 3.7 Web content

The desired site signal is the **active tab’s effective theme base**, after the app appearance override: expose that as `prefers-color-scheme` without imposing page colours. Sites may ignore it. GPUI theme changes never inject CSS, JS, backgrounds or typefaces into a page.

**CEF gate:** test whether stock CEF supplies the desired result from native appearance, first at app scope and then per browser. Don't assume `NSWindow.appearance` scopes Chromium’s media preference. If needed, spike CEF’s public per-browser DevTools method for `Emulation.setEmulatedMedia`. Test before first meaningful paint where possible, re-navigation, cross-origin frames, popups, DevTools attachment, parked/restored tabs and runtime `matchMedia` changes. Don't ship that path if it replaces unrelated media overrides, disrupts DevTools, or leaks an appearance across profiles. If the shipped binary cannot give a reliable per-tab result, use verified app-wide or system-following behaviour, show the actual result in Appearance, and mark per-Space page appearance unsupported. **Do not expose per-site Force Dark.** A process/renderer switch is not a dependable per-site preference; recommend an existing page extension such as Dark Reader for deliberate recolouring.

## 4. Layout customisation

### 4.1 Regions and modes

| Control | Values and behaviour |
|---|---|
| Tab position | **Side** default: sidebar order is resting address (if placed here), Space header, New Tab, favourites, pinned group, normal tab scroll, bottom Space switcher. **Top:** top tab lane below the strip; leading Space chip opens switcher, then pinned icons, horizontally scrolling normal tabs, trailing `+`. Drawer keeps favourites, Space header and switcher, not a duplicate tab list. The strip holds the address by default in this layout. |
| Sidebar / drawer position | **Left** default / Right. Mirror geometry, divider and reveal direction, not the text/read order of rows. |
| Sidebar width | Saved **240**, range **184–360**, in 4 pt steps, via 6 pt resize hit strip or Settings stepper. Clamp actual width to `window width − 320` without overwriting the preferred width. Browser minimum content area `520 × 340`. Resize by keyboard in the stepper; only update the CEF viewport once per display interval during a pinned drag. |
| Sidebar / top chrome visibility | Independently **Auto-hide** default / Shown. Shown reserves layout; Auto-hide overlays. Top chrome means strip plus tab lane if tabs are on top. `⌘S` temporarily toggles sidebar visibility without changing the preference: a called sidebar stays until pointer/focus leave or Escape; a temporarily hidden pinned sidebar restores on next call or new window. The pin control changes the saved preference. |
| Compact Mode | **Off** default. When on, forces target regions to Auto-hide without replacing their saved visibility. Target **Both** default / Sidebar / Top Chrome. A palette/toolbar toggle restores previous visibility when turned off. |
| Full screen | Force **both** regions to transient Auto-hide, even if Shown. Calls work normally, but don't pin or change the stored layout; restore on exit. Keep AppKit's system menu/titlebar reveal and safe area. Presentation starts hidden unless a blocking prompt requires attention. |
| Favourites | **Icon Grid** default / Rows. Both show the same favourites. Grid fills columns left to right, no reorder on hover; rows show title and favicon. Space menu exposes the same two choices. |
| Split view | Out of scope. One page card per active tab; no additional CEF viewport or ownership model. |

When no region reserves layout, CEF occupies the entire usable content rect, with no stroke, radius or inset. If a region reserves layout, subtract pinned regions first and inset the remaining page rect **6 pt on all sides**; apply page edge and shadow. In fullscreen transient overlays do not inset the page. If a horizontal tab lane overflows, preserve pinned order, bound the pin group to one-third of the tab lane’s usable width, and put extra pins in an overflow menu. Scroll the active ordinary tab into view only on selection, not while the user is manually scrolling.

### 4.2 Reveal and focus

- Active-window edge lanes: **4 pt** at the configured side and **4 pt** at the top. In a shared corner the top wins. A pointer must remain in the lane **120 ms** to open; departure cancels the timer. Pointer exit from region plus lane closes after **320 ms** when neither keyboard focus nor an owned menu, drag or popover is inside.
- In macOS full screen, the top lane reaches the display edge. The tab strip sits below the system menu and title bar, and stays open as the pointer moves down to it. Reuse the existing mouse-move monitor for events outside Aeri's content only while Aeri is active; do not intercept system events or poll the pointer.
- Pointer reveal never focuses address, moves the CEF viewport or intercepts a click on the unrevealed page edge. Keyboard calls reveal immediately and focus their designated target; `⌘S` focuses the active tab row or drawer header on a keyboard reveal. Escape closes innermost menu/suggestion list, then transient editing surface, then the called region and restores prior page focus. VoiceOver focus inside a region holds it open.
- Hover hide pauses during button-down/drag or a native menu. On app deactivation, cancel timers and hide non-modal transient chrome; do not activate a different child to do so. No hover-tracking timer while the mouse is stationary except the one-shot show/hide deadlines.

### 4.3 Address and strip controls

| Control | Specification |
|---|---|
| Floating behaviour | **Keyboard & New Tab** default: `⌘L`, `⌘T` and the New Tab button float; clicking the resting field edits attached. **Always Float:** the resting affordance shows the host but opening it, including pointer activation, starts a float. **Attached:** reveal the selected attached region, focus the field there, and anchor suggestions there. If `⌘T` is invoked from an existing tab, preserve existing New Tab creation semantics in all three modes. |
| Attached placement | **Automatic** default: sidebar for side tabs, strip for top tabs. Explicit **Sidebar** or **Top Strip** available in either layout. The fallback at insufficient width is a dedicated address row in that region, not a silent change of mode. |
| Floating position | **Page Centre** default / **Upper Centre**. Centre the field on the usable page rect; its top is at the page vertical midpoint minus half the field height, or 64 pt from the usable top for Upper Centre. Clamp the full field/result surface to 16 pt inside that rect, opening results above if below doesn't fit. Scroll results instead of letting them overflow the window. Palette always floats independently. |
| Toolbar editor | Three ordered slots, Leading / Centre / Trailing; drag or use Move Up/Down and Move to Slot buttons. Defaults: Leading `Back, Forward, Reload/Stop`; Centre address when applicable; Trailing fixed trust cluster and profile badge. Optional movable buttons: Sidebar, New Tab, Library, Compact Mode. Back/Forward/Reload and optional buttons can be hidden or moved. One instance per control. Address placement is controlled separately. Fixed cluster stays trailing; no theme or layout hides security notices. |
| Window controls | **Show with Top Chrome** default / Hidden. When pinned use real AppKit buttons in the frame’s reserved strip. For transient top chrome, use real controls only if the spike validates correct ordering and behaviour without moving the CEF viewport. Otherwise use labelled close/minimise/full-screen buttons that target the owning frame using public AppKit actions. Document that the proxy green button doesn't provide the system’s tiling hover panel; Window menu remains available. In system fullscreen, use the system’s own controls rather than duplicates. Hidden does not remove Window menu or key equivalents. |

Strip controls can overflow into a labelled `More` menu before dropping mandatory indicators or shrinking hit targets; overflow optional buttons first, then navigation. If a trust notice requires action while chrome is hidden, reveal its anchored surface with a keyboard/VoiceOver path. Passive notices and ordinary downloads must not keep the strip open indefinitely. Origin text remains part of the address UI, not a rearrangeable decoration.

### 4.4 Density metrics

All values apply without UI Size `Large`, which changes text by +1 pt and never reduces hit areas. Rows use single-line truncation. Chromium content and native menu dimensions are unaffected.

| Metric | Compact | Standard | Spacious |
|---|---:|---:|---:|
| Strip height | 36 | 44 | 50 |
| Top tab lane height / tab hit height | 30 / 26 | 34 / 30 | 40 / 36 |
| Sidebar tab/list/Space row height | 28 | 32 | 38 |
| Sidebar group inset / row horizontal padding | 6 / 8 | 8 / 10 | 10 / 12 |
| Strip icon button hit square | 26 | 30 | 34 |
| Fav grid cell / gap | 30 / 4 | 36 / 4 | 42 / 6 |
| Attached address field height | 28 | 32 | 36 |
| Floating field height | 38 | 42 | 46 |
| Suggestions and palette row height | 28 | 34 | 40 |
| Main row label / secondary text size | 12 / 11 | 13 / 11 | 13 / 12 |
| Leading row glyph / toolbar glyph | 14 / 14 | 16 / 16 | 16 / 16 |

## 5. Motion and performance

Use transient GPUI invalidation only while a transition is active. No display link, ticking timer or repaint loop owned by Appearance after it settles. Native system animations, such as fullscreen and `NSMenu`, remain under AppKit control.

| Event | Standard duration | Curve and affected pixels |
|---|---:|---|
| Sidebar or strip reveal / hide | 160 / 130 ms | Reveal `cubic-bezier(.2,.8,.2,1)`; hide `(.4,0,1,1)`. Translate in from its edge and clip to the reveal region. Never animate CEF position during a transient reveal. Use one-shot show/hide if child ordering/window movement fails the spike. |
| Bar / palette enter / exit | 140 / 100 ms | Same enter/exit curves; opacity and 6 pt vertical translation, no scale. Field is focused/interactive immediately. |
| Suggestions appear / disappear | 90 / 0 ms | Linear reveal of the panel decoration only; results and focus update without animated reorder. |
| Space switch | 120 ms | Ease-out on accent indicator and decorative chrome. Commit model, list and CEF visibility atomically; no previous-page image, fade, or intermediate view of a parked Space. |
| Theme change | 120 ms | Linear crossfade of **decorative fill only**. Switch semantic foreground/background pairs in one paint. If the effective light/dark or accessibility state changes, snap the entire theme without a crossfade. |
| Tab open / close | 110 / 90 ms | Enter/exit curves above; 4 pt reveal/clip on the row or top tab. Focus and order update immediately. A closing row is non-interactive and removed from the AX tree immediately; no ghost browser or reorder animation. |
| Hover, pressed, focus, window resize, drag, other state | 0 ms | Immediate. No shimmering/resting animation. Use AppKit's resize/fullscreen animation only where AppKit owns it. |

**Subtle** halves duration and travel, rounded to whole ms/pt; **Off** and **Reduce Motion** set both to zero. Reveal intent delays remain the same. If Reduce Motion is enabled during a transition, jump to the correct end state now. Don't hold a snapshot of a window or CEF page for a transition. Avoid per-window grain textures; share the tile and bound small resource caches by active use.

## 6. Shortcuts

### Model and defaults

One Rust command registry: stable ID, title, category, target resolver, enabled predicate, execution callback and optional default chord. Menu items, palette results and context actions bind to that registry. E.g. **Edit Space Theme…** with no context-menu target resolves to the active Space. A row that selects a data record is not a new command ID; use a bindable action such as **Switch to Next Space** or **Choose Theme…**. AppKit/OS-owned menu commands outside Aeri's control are not Aeri commands. Existing Aeri menu actions with no current binding start unbound. Test registry coverage across all Aeri-owned menus and the palette.

| Command ID | Default |
|---|---|
| `tab.new`, `tab.close`, `window.new`, `address.focus` | `⌘T`, `⌘W`, `⌘N`, `⌘L` |
| `tab.reopen`, `tab.cycle` | `⇧⌘T`, `⌃Tab` |
| `tab.select.1` … `tab.select.9` | `⌘1` … `⌘9`; preserve Phase 11 ordering and 9 semantics in regression tests |
| `space.previous`, `space.next` | `⌥⌘←`, `⌥⌘→` |
| `bookmark.add`, `library.bookmarks`, `library.history` | `⌘D`, `⌥⌘B`, `⌘Y` |
| `layout.sidebar.toggle`, `window.fullscreen`, `palette.open` | `⌘S`, `⌃⌘F`, `⇧⌘P` |
| `app.quit`, `settings.open` | `⌘Q`, `⌘,` |
| New commands in §7 other than `settings.open` | Unbound |

App-wide map: command ID → no override, explicit `unbound`, or one replacement chord. One chord = one key with modifiers; no timed sequences. Store macOS virtual key code and normalised modifier mask; display the key under the current input source and update display on source change. The shipped defaults use conventional macOS command equivalents; test default punctuation and number shortcuts on non-US layouts before enabling recording. Persist defaults separately from recorded physical overrides so changing input source doesn't silently redefine the shipped equivalents. Resolve collisions on **effective** chords for the active layout, not menu glyph strings. If a source change introduces a default/override collision, prefer the explicit override, disable the colliding default in that layout, and surface it in Settings; never fire both.

### Recorder and interception

- Shortcuts Settings shows searchable rows: **Command / Shortcut / Source**. Choose **Record**, then press one chord. Escape cancels; Clear sets `unbound`; Reset restores a single default. **Reset All Shortcuts…** confirms before deleting overrides. During recording, dispatch is suspended for that window, including palette actions.
- Require Command or Control; don't bind bare characters, modifier-only keys, Option-only dead keys, or IME composition. Reserve Escape, system task switching (`⌘Tab`, `⇧⌘Tab`, `⌘\``), Force Quit (`⌥⌘Esc`), default system screenshot chords (`⇧⌘3/4/5/6`), and power/media/Fn controls. Preserve system-delivered shortcuts; report “macOS handles this shortcut” for known reserved chords. macOS can intercept user-configured global hotkeys before Aeri sees them; do not claim to enumerate those or ask for Input Monitoring.
- Collision: “`⌘L` is assigned to **Focus Address Bar**.” Offer **Swap Shortcuts** if the edited command has a chord the other command can take; if it is unbound, offer **Move Shortcut**, explicitly saying which command becomes unbound. Confirm both changes in a single DB transaction, or keep both old values. Reset uses the same conflict path; Reset All restores the full built-in map together.
- Dispatch through one key resolver for frame, GPUI children, native menu equivalents and the existing CEF pre-page hook. A matched, enabled Aeri command consumes the event exactly once, including when the page is in an editable field; deduplicate the AppKit/CEF path. Suppress repeats for non-repeatable commands (open, close, dialogs); allow intentional repeat only if registered. Do not capture keys from a native menu/modal or composing text unless its responder participates in dispatch. Disabled/unmatched chords follow the normal native/page route. Native editing uses its normal responder path; Aeri-owned edit menu actions, if offered, are registered and route to the focused editor. Test sites and extensions with overlapping bindings: Aeri wins only when an enabled Aeri action is actually bound and eligible.

## 7. Settings and quick access

One native AppKit-hosted, GPUI-drawn Settings window (`⌘,`), not a tab. Default `820 × 580`, min `740 × 520` for ordinary sections; editor uses the layout described in §3.4. Left navigation `174` pt; header has an explicit profile selector for profile sections (defaults to the invoking window’s profile). Search matches control labels and command names, opens the containing section and focuses the control. Changes take effect on selection except theme drafts, shortcut recordings/conflicts, and confirmed resets.

| Section | Control and default | Storage / scope |
|---|---|---|
| Appearance | App Appearance **Follow Theme**; App Default Theme **Aeri** | `app.db`, global |
| Appearance | Profile Theme **Inherit**; Space Theme **Inherit** for selected Space | `profile.db`, profile and Space-ID keyed refs |
| Appearance | Profile Identity Colour **existing value** (colour picker; retain migration); Use Profile Colour in Chrome **Off** | `profile.db`, profile. Keep existing monogram/identity assignment. |
| Appearance / Space Editor | Space Colour **existing value** (extend fixed palette to exact/custom colour); Use Space Colour in Chrome **Off**; existing name/icon unchanged | `profile.db`, Space record/settings keyed by Space ID |
| Theme Library | **10 bundled themes**, no user themes on install. Import, Export, Duplicate, Edit, Rename, Delete user recipe. Editor controls/defaults in §3.4–3.5. | `app.db`, local appearance-only recipes, available to all profiles. If in use, Delete is refused and explains which scopes must detach; no silent fallback. |
| Layout | Tab Position **Side**, Sidebar Side **Left**, Width **240**, Sidebar Visibility **Auto-hide**, Top Chrome Visibility **Auto-hide** | `profile.db`, affects all windows of that profile |
| Layout | Density **Follow Theme**; Compact Mode **Off**, Compact Target **Both**; Favourites **Icon Grid** | `profile.db`, profile |
| Toolbar & Address | Floating Behaviour **Keyboard & New Tab**; Attached Placement **Automatic**; Floating Position **Page Centre** | `profile.db`, profile |
| Toolbar & Address | Toolbar slots **Back, Forward, Reload/Stop; automatic address slot; fixed trailing cluster**; Window Controls **Show with Top Chrome**. Reset Toolbar restores layout, not theme. | `profile.db`, profile |
| Shortcuts | Shipped map in §6; no overrides; record/clear/reset | `app.db`, global |
| Appearance footer | Read-only system Reduce Motion, Reduce Transparency, Increase Contrast, and effective website appearance support. **Open macOS Accessibility Settings** where supported. | OS state; never save a copy as a user choice |

Persist durable choices only; do not save temporary reveal state, picker preview, suggestion selection or animation state. Migrate by filling absent settings, not rewriting existing space/profile identities or tab data. Keep your active configuration and last selected Settings section independent of the theme recipe.

**Palette additions:** `settings.open` — **Settings…**; `settings.appearance`, `settings.layout`, `settings.toolbar`, `settings.shortcuts`; `theme.choose`, `theme.edit.app`, `theme.edit.profile`, `theme.edit.space`, `theme.import`, `theme.export.current`; `appearance.system`, `appearance.light`, `appearance.dark`, `appearance.follow_theme`; `layout.tabs.side`, `layout.tabs.top`, `layout.sidebar.left`, `layout.sidebar.right`, `layout.sidebar.pin_toggle`, `layout.top.pin_toggle`, `layout.compact.toggle`; `address.mode.keyboard`, `address.mode.always`, `address.mode.attached`. Expose the alternate choice in command titles or checked state. All use the same binding registry.

**Quick access:** Space header/switcher menu: **Edit Space Theme…**, **Use Inherited Theme**, **Edit Space…**, **Favourites → Icons / Rows**. Sidebar background menu: **Pin Sidebar / Auto-hide Sidebar**, **Move Sidebar Right / Left**, **Layout Settings…**. Strip menu: **Customise Toolbar…**, **Compact Mode**, **Pin Top Chrome / Auto-hide Top Chrome**. Profile badge menu: **Edit Profile Theme…**. Native menu equivalents update when a binding changes.

## 8. Visual polish

Match macOS in type, alignment, menu ownership, focus and restraint, not by simulating a Tahoe-specific glass effect on older systems. Use SF Symbols where the existing symbol bridge supports them, otherwise equivalent monochrome vector assets. Snap line positions to backing pixels; radii and text remain in points. Measurements below are Standard unless otherwise stated.

| Surface | Spec |
|---|---|
| Sidebar and regular rows | Sidebar group inset `8`; row `32 × available`, radius `6`, padding `10`, leading icon `16`, title gap `8`. Right action slot `24` square, gap `4`; keep it reserved when an action is appearing so text doesn't jump. Ordinary title Regular, active title Medium. Hover fills only the row. Selected fill + 2 pt × 14 pt accent bar inset `3` on leading edge; selection is still visible in an unfocused window. |
| Pinned tabs | Sidebar: pinned group above ordinary rows with `8` pt intergroup gap and decorative divider when both nonempty; 11 pt Medium **Pinned** label and pin glyph when needed. Top: pinned icon hit rect `30 × 30`, rounded `6`, title via tooltip/VoiceOver. Top ordinary tabs default `144` pt wide, clamp `96–200`; `6` pt gaps, `8` pt padding, `16` pt favicon, `24` pt close hit. |
| Tab status | Resting/unloaded: unchanged title contrast, outlined resting glyph in trailing slot, tooltip “Resting — opens when selected”; no blanket opacity. Playing/muted: speaker glyph with slash for muted; if mute exists, label the button and expose menu action; if not, render status only. Hover exposes close without covering an active audio action; if both require space, close goes to context menu at minimum width. During typed navigation, render the pending **host** in Medium with a static load indicator until real title/host commits; don't present typed text as a verified origin. Do not add a permanent “typed” badge or history flag. |
| Favourites | Grid cells as in §4; radius `8`, icon `18`, no persistent title; title/host via tooltip and AX. Row style uses tab row geometry and title. Mono initial fallback in accent tint, 12 pt Semibold, contrast checked. |
| Space header and switcher | Header `32` high, `10` padding, identity dot `8`, gap `8`, name 13 pt Semibold, chevron `12` in a `24` pt hit rect. Header has separate AX menu action. Bottom switcher uses `28` pt high chips, `6` gap, `14` pt symbols, active icon plus selection fill and accessible selected state. Keep New Space at trailing edge; overflow chips into a Space menu rather than scaling hit targets. |
| Strip and page card | Strip `44` high, horizontal inset `8`, navigation gap `2`; 16 pt icons in 30 pt hits. Put a 1-device-pixel divider at the reserving edge only. Reserve native traffic-light space from actual button frames plus 8 pt, not a guessed position. Trust badge labels 11 pt Medium when expanded, all meaning also via icon/text. Profile badge: `22` pt identity circle in `30` pt hit, 11 pt Semibold monogram. Card takes §4 inset and theme radius; on edge-to-edge there is no artificial border/shadow. |
| Attached address | `32` high, radius `7`, padding `9`, 16 pt origin/action glyph, `7` pt gap. Show host at rest with 13 pt Medium. On edit, show actual input and full address where available; retain real origin/security information in its own slot. Never dim the registrable domain below normal text contrast. Drawing names and trust notices is not delegated to the theme. |
| Float and suggestions | Preferred width `560`, clamp to usable page rect minus `32` (minimum preferred `320` where space permits). Combined surface radius `12`, 1-device-pixel edge and overlay shadow; field `42` high, side inset `12`, edit text 15 pt Regular (+1 for Large). Up to 7 visible suggestion rows of `34`, content inset `6`, row radius `6`, `16` icon, `8` gap; title `13`, host/metadata `11`. Leading and trailing clip before origin becomes ambiguous; accessible label carries full text. Keep list highlight separate from the text cursor. |
| Palette | Same floating shell; query label “Run a command,” no fake traffic lights or URL security glyph. Up to 8 visible rows; control name `13` Medium, category `11`, right-aligned shortcut `11` in tertiary ink. Show selected fill and keyboard focus; Return executes only the selected enabled result. Disabled entries label why if shown. |
| Context menus | Use `NSMenu`, proper ellipsis for actions opening UI, system separator between action groups, checks for the current scope or placement. Don't add tinted menu ornaments, shortcut text that isn't a real equivalent, or visual density overrides. |
| Secondary windows | Library default `800 × 540`, min `600 × 400`, `180` pt navigation column; Profile Manager `520 × 420`, min `440 × 340`; Space Editor `440 × 400`, min `400 × 340`; Resources `720 × 480`, min `560 × 360`. Use `44` pt header, `18` pt content inset, `16` pt section spacing, `8` pt label/control gaps, `16` pt Semibold headings, `13` pt labels, and density-matched list rows. Use `surface.panel` and `surface.inset`, not nested translucent cards. Resources uses right-aligned tabular numerals and text plus icon for status. Empty states: one explanation and one applicable action; no motion. |

**CEF/AppKit ordering contract:** the GPUI frame is below the CEF child. Draw transient sidebar/strip and floating UI in ordered GPUI child surfaces above CEF; clipping in the frame alone can't cover a child window. Prefer one reusable child per independently revealed region, created lazily; a closed child must not intercept hits, appear in app switching or participate in AX. Z-order: frame → active CEF page → revealed chrome → address/palette/owned popovers → native menus/modal surfaces. Only the active CEF child may receive page input. Apply frame and child rects in a coordinated main-thread update on movement, screen/scale change, resize and tab switch. No new CEF browser is created to restyle or move a region.

## 9. Accessibility

- **Contrast and shape:** use §3’s pair checks in both modes and interaction states. Colour is never the sole distinction for pinned, resting, playing, warning, selected, or invalid. Current theme and radio values have checkmarks plus AX state. Disabled controls have a programmatic disabled state; their explanations use readable text.
- **Focus:** 2 pt ring plus 1 pt keyline, visible and not clipped by a row or child-window edge. Selection and focus are distinct. Keep focussed items scrolled into view. Main browser focus order follows the visible hierarchy; hidden chrome is removed from AX and Tab order but each region has a menu/palette route to reveal and focus it.
- **Editor operation:** Tab/Shift-Tab traverses controls; arrows move within radio sets/lists or adjust sliders. Steppers and editable numeric fields duplicate angle/stop/radius drag interactions. Palette/suggestion arrows move active result, Return confirms, Escape unwinds. Toolbar rearrangement and all theme operations work without a pointer.
- **VoiceOver:** assign roles, labels, value and actions, e.g. “Sidebar visibility, Automatic, pop-up button”, “Favourites, grid”, “Aeri Docs, pinned, selected, tab”, “Gradient stop 2, 47 percent, colour #…”. Announce import failure once, shortcut conflicts once and actual Space switches once. Don't announce an entire palette repaint or every slider-derived swatch. Restore focus to the invoking control or active page on closing a child.
- **OS settings:** observe accessibility display option changes and effective appearance on the main thread. Increase Contrast strengthens meaningful contours and turns off material; Reduce Transparency replaces material with solid fills and removes decorative translucency behind controls; Reduce Motion is unconditional. Don't assume GPUI automatically exposes the right AppKit accessibility tree or that AX traverses across sibling children—verify with VoiceOver and Accessibility Inspector, including a focused CEF input and transient overlay. If necessary, implement an AppKit AX bridge before enabling keyboard auto-hide in release.

## 10. Delivery plan

| Slice | Build scope | Tester acceptance | Risk / gate |
|---|---|---|---|
| **1. Token spine and system appearance** | Typed resolver with reference values; migrate frame and existing windows; appearance observation and settings transaction skeleton. Record competitor builds and a Phase 11 perf baseline. | Open two profiles and all secondary windows. Change macOS appearance and Aeri override live; compare same-tick updates, relaunch persistence and default palettes. Run pair tests against both appearances. | **Spike:** GPUI/AppKit effective appearance per child, AX ownership and CEF page-scheme propagation, including first navigation. Establish honest fallback before promising web parity. |
| **2. Page-first browser shell** | Reveal regions, keyboard holds, pinned/hidden geometry, tab and focus-safe child ordering, fullscreen. | Default shows no chrome/gutter; hover lanes reveal/hide at specified delays; `⌘S` and `⌘L` work with pointer nowhere near the edge. Check app switching, fullscreen on two displays, click edges, and moving the window across backing scales. | **Spike first:** NSWindow ordering, hit testing, native title controls and page clipping. Use public-action proxy controls and square page edge if needed. Never trade away correct input ownership for animation. |
| **3. Layout, density and toolbar** | Side/top tabs; right drawer; favourites/density; toolbar slots; compact targets and three address modes. | With two Spaces and overflowing pins, flip tab position and drawer side while a live page is open. Check page scroll/form state survives, drag-resize only when intended, and each address route focuses and dismisses correctly in fullscreen. Verify trust UI survives overflow. | **Spike:** moving layered NSWindows while changing reserved regions/top lane; test fast switches, popup anchors and multi-display fullscreen. One CEF browser per tab stays unchanged. |
| **4. Theme recipe and scopes** | Derivation, 10 bundled presets, profile/Space resolve, editor preview/undo, colour pickers. | Make a 3-colour Space gradient and a profile accent; verify precedence in two browser windows, menus and Library. Change Light/Dark while editing. Test high/low lightness and saturated seeds with a pair property-test corpus. | GPUI gradient and font rendering; if a gradient path allocates a frame-sized texture per child, use a bounded GPU primitive before shipping. Eyedropper is optional behind its spike. |
| **5. Theme portability and materials** | Strict import/export, local library, grain, optional native material. | Round-trip every bundled theme; reject unknown nested fields, oversized payloads, duplicates, invalid stops and a recipe containing a URL field; confirm no active theme changes. Test import with network unavailable. Toggle Reduce Transparency while material is active. | **Spike:** AppKit material behind GPUI with opaque control backplates, no persistent compositor cost, no hit/focus regression. Ship the opaque fallback if it fails; no custom blur substitute. |
| **6. Commands, Settings and keymap** | Complete Settings shell, command registry, recorder/swaps, menu/palette integration. | Bind an existing/new command, provoke `⌘L` conflict, swap, reset and relaunch. Test CEF text input, CEF popup, native menus, IME, ISO/ANSI layouts, input-source change and native system chords. Confirm all Aeri menu/palette commands have IDs. | CEF versus AppKit event order and double execution; preserve system and editor responder routes. Keep a Phase 11 defaults regression suite. |
| **7. Visual polish, AX and motion** | Finish view metrics/status, menu equivalence, secondary windows, reduced motion, AX focus restoration. | Use keyboard and VoiceOver without the mouse to edit a theme, move a button, open and close hidden chrome, mute where supported, and return to the active page. Toggle OS display options mid-animation; check both modes in screenshots at 1× and 2×. | Gate release on usable cross-child AX, correct focus, and first-paint text contrast. Snap transitions or use opaque surfaces rather than ship bad states. |
| **8. Release hardening and perf** | End-to-end stress/migration, instrumentation, benchmark and release notes for verified limitations. | Run idle, reveal, resize, theme-change, 20-tab/two-profile and fullscreen workloads. Test malformed files and switching Spaces during suggestions/menu/lifecycle events. Record Helium and before/after Aeri results. | Don't confuse OS/compositor/CEF work with Appearance work. Profile separately and disable optional effects that fail the budgets below. |

**Release budgets:** On the same Apple Silicon machine, OS build and power mode, use repeated warmed runs of identical locally served static pages, no extensions, a fixed tab count, and a 60 s settled idle sample. Record app plus CEF process-tree CPU and physical footprint; compare identical Aeri view states against Phase 11/base instrumentation and publish Helium separately as a reference. Require **zero appearance-owned periodic callbacks or draw submissions at idle**, median idle CPU delta **≤0.1 percentage point of one core** across five runs (investigate noise rather than round it away), and warmed footprint delta **≤8 MiB for a typical two-window session**, excluding Chromium-content variance verified by a control run. Track lazily created child windows separately; aim for **≤1 MiB incremental footprint per additional themed browser window above its base state**. A theme switch must not launch a renderer or retain a page screenshot. If a budget fails, cut material/crossfade/extra surface costs before relaxing it.

## 11. Open questions for the owner

| Question | Recommendation |
|---|---|
| If real traffic lights cannot overlay CEF without moving the page, prefer proxy controls or a briefly resized page? | **Proxy controls for transient chrome; real AppKit buttons for pinned chrome.** Viewport stability matters more than replicating the green button’s hover menu in the transient case. |
| If stock CEF cannot reliably expose a per-Space page colour preference, should it block appearance shipping? | **No.** Ship a clearly labelled, verified system/app-wide site preference and keep per-Space chrome. Don't emulate per-Space support with page injection. |
| Should native material be mandatory at release? | **No.** Ship opaque themes first if the material spike fails. The format can retain material intent without committing every build to render it. |
| Do we need an icon-only collapsed rail in addition to edge reveal and pinned chrome? | **Defer.** Test the right/left, top/side and Compact Mode combinations first. An icon rail adds ambiguity for similarly named tabs and another accessibility/focus state. |
| Should Phase 12 include Arc Boosts-style site changes or Zen-style executable mods? | **No.** Keep source themes declarative, expose commonly wanted layout controls as product features, and leave site rewriting to extensions. |
| Should the first Phase 12 launch retain Phase 11’s always-visible presentation for existing installs? | **Use the page-first default for everyone without an explicit choice.** Preserve all identity and browsing data; provide a one-time non-blocking hint for `⌘S`, `⌘L` and **Pin Sidebar**. |