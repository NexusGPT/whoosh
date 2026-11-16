# Provider marks: where each file came from (downloaded 2026-11-09, unmodified)

| File | Mark | Source | Usage / licence |
|---|---|---|---|
| anthropic.svg | Claude Spark, Clay (#D97757) | https://www.anthropic.com/press-kit (serves the zip "Anthropic media resources"; file "Anthropic logos/Claude logos/3 Claude Spark/SVG/Claude Spark - Clay.svg"). https://www.anthropic.com/brand answered 404. | Anthropic's press kit for media use; the kit holds no licence file. Claude and the Spark are Anthropic's trademarks. |
| openai.svg | OpenAI blossom (black in light, white in dark, per the file's own style) | https://chatgpt.com/cdn/assets/favicon.6a33781f61.svg (linked from chatgpt.com; its header names openai/openai chatgpt/web/src/assets/favicon.svg). https://openai.com/brand answered 403 to every request, so its guidelines were not read. | OpenAI's trademark; usage terms unread (403). To confirm against openai.com/brand. |
| gemini.svg | Gemini sparkle (Google's gradient, an embedded JPEG clipped to the sparkle) | https://www.gstatic.com/lamda/images/gemini_sparkle_aurora_33f86dc0c0257da337c63.svg (linked from https://gemini.google.com/). | Google's trademark; served by Google's own Gemini site. Google brand-resource terms not read. |
| jev.svg (SUPERSEDED 2026-11-09, round 2: the app no longer draws it; Jev and Laya carry the Nexus mark, brief settingswindow-r2 and SPEC A81 "Jev from the Nexus brand assets". Kept as the record.) | TypeSafe glyph (#1E1E1E; even-odd fill). Jev is TypeSafe's API | The header logo inline in https://typesafe.ai/ (the page's `<svg viewBox="0 0 21 32">`, saved as served, downloaded 2026-11-09). https://typesafe.ai/brand and /press answered 404; the favicon (https://framerusercontent.com/images/aNFzSFxM4fjICmnibw7npfZjcQ.png) is the same glyph on a pink disc, PNG only. Dark theme uses #FEFEFE, the white the site sets its wordmark in on dark. | TypeSafe's mark; no brand terms published. |
| openrouter.svg | OpenRouter glyph (#7624F4) | https://openrouter.ai/brand/v2/openrouter-glyph-light.svg (linked from https://openrouter.ai/brand). | OpenRouter's brand page asset. |

The app draws each mark from the file's path data, verbatim (ProviderMarks.swift, GeminiMarkImage.swift); these files are the record.

## Round 2 additions (2026-11-09)

| File | Mark | Source | Usage / licence |
|---|---|---|---|
| nexus-mark-ink.svg, nexus-mark-white.svg | The Nexus mark, four paths (the third even-odd). Ink #0E1116 in light, white in dark. The app's mark for Jev and for Laya. | The org brand kit, copied unmodified from general-context/brand/design-system-v1/assets/logo/ (rule file logo.md in the same folder: "Ink or white only"). | Nexus's own mark. |
| mistral.svg | Mistral's M, ten squares in five fixed colours (#FFAF01 #FF8204 #FA500F #E10500 #C4001D) | https://mistral.ai/favicon.svg, served by mistral.ai (HTTP 200). https://mistral.ai/brand links the brand-kit zips (https://cms.globalaegis.net/api/documents/file/Mistral_Logos_2026.zip), which answered 403 to every request, so the zip was not read. | Mistral's trademark; brand terms unread (zip refused). To confirm against https://mistral.ai/brand. |

## FINDING: assets and rows not in the tree

| Provider | What is missing | Path the app expects | Effect |
|---|---|---|---|
| Apple Intelligence | The real mark. Apple publishes no downloadable Apple Intelligence logo file (not found by this lane; no SF Symbol carries it in colour). A drawn stand-in is refused by the brief. | Sources/Features/SettingsWindow/Marks/apple-intelligence.svg (then one MarkArt in ProviderMarks.swift, case `.apple`, colour lines marked `// brand-mark: apple`) | `ProviderMark.apple.present` is false: nothing is drawn, `rows mac-settings-ai.logos` serves `present: false`. No Settings row uses it today, so no row is blank. |
| Mistral | No Settings row: Mistral is in the core only as a speech provider (Voxtral, core/crates/ai-providers/src/table.rs) and the Speech group has the one "On this Mac" row. | none (asset is present) | The mark is staged and probed; it shows when a Mistral row exists. |
