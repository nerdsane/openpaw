# Curation 40 Query Manifest

Generated: 2026-05-08

WorkRequest: `en-019e088b-830a-79f0-8a20-9d674b896bf0`
FactoryCase: `en-019e088b-8c48-73c3-b49f-611c1c509e5a`
WorkCycle: `wc-019e088b-8cb7-70e2-9b0c-bddafb1f63fe`

Durable artifact path: `curation/40-query-manifest.md`

## Operational Guard

Do not restart the full curation batch from this manifest until the thumbnail artifact gate has passed.

The active gate is:

1. Decode base64 thumbnail payloads to image bytes before upload.
2. Wait for the `File` entity to reach `Ready` before synthesis completes.
3. Validate browser-renderable image bytes.
4. Set `has_thumbnail=true` only after the file is `Ready` and image-valid.

## Recovery Summary

- Raw campaign prompt list recovered: 45 unique entries.
- Main campaign set recovered: entries 1-40.
- Optional Katagami-only subset recovered: entries 41-45.
- Exact normalized duplicates found: 0.
- Current production `CurationQueries` exposed by OData: 6.
- Current production `CurationQueries` are all failed canary or restart attempts for 2 directions.
- A clean one-to-one durable mapping from the 45 raw prompts to `CurationQuery` records was not present in current entity state.
- Later execution direction labels for the full claimed 40-idea batch remain partially unrecovered and are labeled below rather than guessed.

Status vocabulary used in this manifest: `not submitted`, `failed`, `draft produced`, `under review`, `published`, `blocked`, `unrecovered`.

## Evidence Sources

| Source | Evidence |
| --- | --- |
| `Sessions('ss-019e0439-ddb8-7080-9a55-7a0f39b2ceef')` | Main operator conversation that listed the 45 raw search prompts and launched the first synthesized concept batch. |
| `SessionEntries` for `ss-019e0439-ddb8-7080-9a55-7a0f39b2ceef`, assistant sequence 1 | Recovered the raw 45 prompt strings. |
| `SessionEntries` for `ss-019e04bc-7489-7912-a338-7367d1a19a48` | Recovered batch-one status summary and partial later-batch references. |
| `Sessions('ss-019e0862-4af6-7582-807b-15742f5e6fc3')` | Confirmed later operator need for the full list/progress and the failed memory search. |
| `CurationQueries?$top=100` | Current OData state exposes 6 failed records, all for the two canary directions. |
| `DesignLanguages('<slug-or-entity-id>')` | Current state checked for recovered batch-one languages and current canary language records. |
| `Files('fl-019e07f3-69ad-7bf0-b43d-d53a70f1141e')` and `Files('fl-019e07f4-bcac-7d33-a074-8c8bdafaf2d0')` | Current file state checked for the latest thumbnail blocker examples. |

## Normalization Rule

Each recovered prompt was trimmed, deduplicated by exact case-insensitive text, and assigned a stable manifest key. No prompt text was invented. Where execution direction names or entity links were not recoverable, the row is explicitly marked `unrecovered` or `not submitted`.

## Recovered Raw Query List

These are the recovered campaign prompts from the recent session history. They are preserved as source prompts, not as proof that each prompt was submitted one-to-one as a `CurationQuery`.

| Key | Group | Status | Entity linkage | Recovered query |
| --- | --- | --- | --- | --- |
| `q001` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. Related concept batch exists below. | minimal black and white manga illustration, coding terminal interface, clean line art, existential cyberpunk mood, polished editorial composition |
| `q002` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. Related concept batch exists below. | monochrome anime hacker scene, sparse command line UI, elegant manga panels, negative space, clean futuristic aesthetic |
| `q003` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. Related concept batch exists below. | black and white cyberpunk manga, terminal windows, lonely programmer, existential atmosphere, precise ink lines, minimalist composition |
| `q004` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. Related concept batch exists below. | clean manga-style coding terminal, stark black and white, y2k interface elements, polished graphic design, quiet sci-fi mood |
| `q005` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. Related concept batch exists below. | minimalist anime terminal screen, monochrome manga aesthetic, cybernetic philosophy, clean composition, high contrast ink illustration |
| `q006` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. Related concept batch exists below. | black and white Japanese sci-fi manga, command line code overlays, empty city at night, existential digital atmosphere |
| `q007` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. Related concept batch exists below. | polished monochrome manga poster, hacker terminal, elegant negative space, 2000s anime influence, clean futuristic design |
| `q008` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. Related concept batch exists below. | minimal cyberpunk manga illustration, terminal UI, ghostly digital presence, black and white palette, refined linework |
| `q009` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. Related concept batch exists below. | Ghost in the Shell inspired cyberpunk anime aesthetic, clean terminal UI, existential android mood, polished minimal composition |
| `q010` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. Related concept batch exists below. | 90s cyberpunk anime interface design, neural network terminal, philosophical sci-fi mood, clean manga line art |
| `q011` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | cybernetic anime portrait, command line overlays, minimalist black and white manga, existential polished art direction |
| `q012` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | Japanese cyberpunk anime still, terminal code reflections, sparse futuristic lab, clean elegant composition, Ghost in the Shell mood |
| `q013` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | existential cyberpunk anime, hacker console, clean vector-like manga linework, restrained black white silver palette |
| `q014` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | Cowboy Bebop inspired anime composition, hacker terminal, noir jazz atmosphere, clean manga line art, restrained futuristic style |
| `q015` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | 90s anime cool sci-fi mood, coding terminal in a dim room, minimalist manga composition, polished cinematic framing |
| `q016` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | retro anime hacker workstation, clean black and white manga style, jazz noir mood, existential loneliness |
| `q017` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | space bounty hunter anime aesthetic, terminal UI graphics, minimalist cyberpunk illustration, polished 90s anime composition |
| `q018` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | analog sci-fi anime interior, command line screen glow, clean manga inkwork, Cowboy Bebop style restraint |
| `q019` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | Y2K cyber interface design, manga hacker illustration, clean terminal windows, chrome accents, polished futuristic aesthetic |
| `q020` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | early 2000s anime computer interface, terminal UI, minimal manga character design, translucent panels, clean cyber aesthetic |
| `q021` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | Y2K anime hacker desktop, command line code, glossy monochrome interface, futuristic editorial poster |
| `q022` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | minimal y2k cyberpunk manga, terminal screen, chrome typography, black white blue palette, polished design system |
| `q023` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | 2000s anime web terminal aesthetic, clean manga line art, digital existential mood, silver black white interface design |
| `q024` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | retro-futuristic anime UI, coding terminal, y2k chrome graphics, clean manga poster composition |
| `q025` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | black white and electric blue manga cyberpunk terminal, clean polished anime aesthetic, existential sci-fi mood |
| `q026` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | minimal manga hacker scene, black white red accent palette, terminal UI, 90s anime cyberpunk atmosphere |
| `q027` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | clean anime terminal interface, monochrome with neon green code glow, y2k cyber aesthetic, polished composition |
| `q028` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | silver black and pale cyan cyberpunk manga, coding terminal overlays, Ghost in the Shell inspired mood |
| `q029` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | cream black and muted red manga coding terminal, vintage 90s anime poster, existential clean art direction |
| `q030` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | black white and magenta y2k anime terminal UI, clean manga linework, polished cyber editorial design |
| `q031` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | deep navy and white anime hacker illustration, terminal command line, 2000s cyberpunk mood, minimal clean composition |
| `q032` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | warm beige monochrome manga scene, computer terminal, analog 90s anime atmosphere, existential quiet mood |
| `q033` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | black white acid green cyber terminal, manga character silhouette, y2k hacker aesthetic, clean polished design |
| `q034` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | chrome silver blue anime cyber interface, coding terminal, minimal manga figure, futuristic existential design |
| `q035` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | artsy minimalist manga poster, terminal code fragments, existential cyberpunk theme, clean black and white composition |
| `q036` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | polished editorial anime illustration, coding terminal abstraction, negative space, philosophical sci-fi mood |
| `q037` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | minimal cyber manga composition, floating terminal windows, existential text fragments, refined black and white design |
| `q038` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | high fashion cyberpunk anime poster, terminal UI, clean manga ink lines, y2k graphic design influence |
| `q039` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | museum-grade manga cyberpunk illustration, command line interface, sparse composition, existential digital atmosphere |
| `q040` | Main 1-40 | not submitted | No one-to-one `CurationQuery` found. | clean conceptual anime art, terminal screen as portal, black and white manga aesthetic, polished surreal sci-fi |
| `q041` | Optional Katagami-only subset | not submitted | No one-to-one `CurationQuery` found. | katagami inspired cyberpunk manga, terminal UI patterns, black and white paper-cut composition, clean futuristic design |
| `q042` | Optional Katagami-only subset | not submitted | No one-to-one `CurationQuery` found. | Japanese stencil art meets coding terminal, minimal anime cyberpunk, black white and blue palette, polished graphic style |
| `q043` | Optional Katagami-only subset | not submitted | No one-to-one `CurationQuery` found. | katagami paper cut manga hacker scene, y2k terminal interface, clean monochrome composition |
| `q044` | Optional Katagami-only subset | not submitted | No one-to-one `CurationQuery` found. | traditional Japanese stencil pattern fused with Ghost in the Shell cyber terminal, minimalist manga poster |
| `q045` | Optional Katagami-only subset | not submitted | No one-to-one `CurationQuery` found. | black and white katagami cyber interface, anime hacker silhouette, clean existential sci-fi design |

## Recovered Execution Progress

The operator-facing "how are we doing?" answer should use this section, because the raw prompt list above was not stored as one `CurationQuery` per prompt.

### Batch 1 Synthesized Concept Directions

These ten directions were recovered from session history as the first launched concept batch. Current `DesignLanguage` states were checked through OData on 2026-05-08. The historical query attempt counts are from the session summary and are retained as provenance, not as current `CurationQuery` state because those older records are not exposed by the current `CurationQueries?$top=100` result.

| Concept | Current status | DesignLanguage links | Historical attempt note |
| --- | --- | --- | --- |
| monochrome manga terminal noir | under review | `DesignLanguages('silent-shell-manga-noir')`, `DesignLanguages('null-koma-terminal-noir')`, `DesignLanguages('monochrome-manga-terminal-noir')` | 5 failed attempts reported in session history; three language records now exist and are `UnderReview`. |
| Ghost-in-the-Shell shellcode minimalism | under review | `DesignLanguages('optical-shellcode-terminal')`, `DesignLanguages('optic-shellcode-pearl-grid')` | 4 failed attempts reported in session history; two language records now exist and are `UnderReview`. |
| Cowboy Bebop jazz terminal lounge | published | `DesignLanguages('bebop-noir-terminal-lounge')` | 3 attempts reported: 1 completed, 2 failed. |
| Y2K chrome manga console | under review | `DesignLanguages('y2k-chrome-manga-console')` | 4 failed attempts reported in session history; language record now exists and is `UnderReview`. |
| clean vapor-terminal anime | published | `DesignLanguages('vapor-terminal-ova-console')` | 3 attempts reported: 1 completed, 2 failed. |
| ink-and-phosphor hacker zine | published | `DesignLanguages('ink-phosphor-hacker-zine')` | 3 attempts reported: 1 completed, 2 failed. |
| existential station interface | published | `DesignLanguages('existential-station-interface')` | 3 attempts reported: 1 completed, 2 failed. |
| cyber-noir editorial manga | published | `DesignLanguages('cyber-noir-manga-dossier')` | 3 failed attempts reported; later session notes indicate manual publication, and current OData status is `Published`. |
| Dreamcast-era web terminal | published | `DesignLanguages('planetweb-aqua-terminal')`, `DesignLanguages('aqua-browser-noir-terminal')` | 4 attempts reported: 1 completed, 3 failed; one related record is `UnderReview`, one is `Published`. |
| paper-white android diary | published | `DesignLanguages('paper-white-android-diary')` | 3 attempts reported: 1 completed, 2 failed. |

### Partial Later-Batch Recovery

| Batch scope | Status | Recovered linkage | Notes |
| --- | --- | --- | --- |
| Batch 11-20 | under review | `DesignLanguages('powder-gray-wired-bedroom-console')`, `DesignLanguages('nerv-command-line-theology')`, `DesignLanguages('shibuya-payphone-packet-radio')` | Session history reported `batch_counts Failed:10` and `batch_language_counts Draft:1 UnderReview:9`. Only these three direction labels were cleanly recovered from available artifacts. The other seven labels are `unrecovered`. |
| Batch 21-30 | blocked | Latest visible canaries: `ceramic android service kiosk`, `minimal mecha cockpit notation` | Current entity state exposes repeated failed restart/canary attempts for only these two directions. The other eight direction labels are `unrecovered`; do not infer them from old entity IDs alone. |
| Batch 31-40 | unrecovered | None | No durable session or entity artifact with clean labels was recovered. |
| Optional Katagami-only subset 41-45 | not submitted | Raw prompts `q041` through `q045` | Recovered as prompts, but no durable execution records were found. |

## Current Canary Failures

The current production `CurationQueries` records preserve the active blocker. Both latest canaries produced draft `DesignLanguage` records, but both query workflows are `Failed` because thumbnail handling did not satisfy the synthesis completion gate.

| Direction | Manifest status | CurationQuery | DesignLanguage | Thumbnail file | Failure reason |
| --- | --- | --- | --- | --- | --- |
| ceramic android service kiosk | blocked | `CurationQueries('en-019e07e4-6f7a-72b3-9642-1d668105454d')` | `DesignLanguages('en-019e07e9-958a-7151-a2be-2abe15e8af3e')`, draft, slug `porcelain-android-service-ritual` | `Files('fl-019e07f4-bcac-7d33-a074-8c8bdafaf2d0')`, current state `Created`, mime `image/jpeg` | Synthesis requires a valid gallery thumbnail before review; file is `Created`, expected `Ready`. |
| minimal mecha cockpit notation | blocked | `CurationQueries('en-019e07e4-7279-7963-ad12-933c42390ae6')` | `DesignLanguages('en-019e07eb-20b2-7ba1-9e99-d97c57d1d6d9')`, draft, slug `silent-gantry-notation` | `Files('fl-019e07f3-69ad-7bf0-b43d-d53a70f1141e')`, current state `Ready`, mime `image/jpeg` | Synthesis failure records that the thumbnail stores base64 text and must be uploaded as decoded browser-renderable image bytes. |

### Preserved Previous Canary/Restart Failures

| Direction | CurationQuery | DesignLanguage | Failure summary |
| --- | --- | --- | --- |
| ceramic android service kiosk | `CurationQueries('en-019e05d1-b465-7ef2-91e8-1a38be670ec1')` | `DesignLanguages('en-019e05e8-49da-73d0-a71d-c70918e17ea6')`, draft, slug `porcelain-service-android-kiosk` | Thumbnail file stored base64 text instead of decoded browser-renderable bytes. |
| minimal mecha cockpit notation | `CurationQueries('en-019e05d2-0620-7e63-8b3d-514e921e9400')` | `DesignLanguages('en-019e05eb-1008-72c0-ae28-b09a449fe2a5')`, draft, slug `graphite-cockpit-notation` | Thumbnail file had non-image mime/state for gallery validation. |
| ceramic android service kiosk | `CurationQueries('en-019e072c-2272-7cb3-bde1-10e65cffebaf')` | `DesignLanguages('en-019e0731-b513-7df2-bb34-230ac96cb1cf')`, draft, slug `porcelain-service-terminal` | Thumbnail file stored base64 text instead of decoded browser-renderable bytes. |
| minimal mecha cockpit notation | `CurationQueries('en-019e072c-2531-7ba3-bb89-99efeed2f219')` | `DesignLanguages('en-019e0730-c391-7c30-a610-c6f22fad8f4e')`, draft, slug `graphite-cockpit-annotation` | Thumbnail file stored base64 text instead of decoded browser-renderable bytes. |

## Unrecovered Items

The following gaps are intentionally explicit:

| Gap | Label |
| --- | --- |
| One-to-one `CurationQuery` records for raw prompts `q001` through `q045` | unrecovered |
| Seven clean labels from later batch 11-20 | unrecovered |
| Eight clean labels from batch 21-30 beyond the two canaries | unrecovered |
| Ten clean labels from batch 31-40 | unrecovered |
| Durable entity-backed campaign manifest record | unrecovered; this file is the durable local artifact until an entity-backed model is designed and approved |

## ADR Decision

No ADR was added for this work. This patch creates a durable local recovery artifact and proof packet only; it does not change entity specs, WASM integrations, Cedar policies, storage models, triggers, deployment behavior, or agent capability surfaces. If this manifest is later promoted into an entity-backed campaign record, that will be a material architecture change and should get an app-scoped ADR before implementation.
