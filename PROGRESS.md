# Korean Translation Progress

## Date: 2026-05-31

## Status: ✅ COMPLETE — 100% Translated (879/879 keys)

## Summary

### Issue Found
The `en.json` file had been **accidentally overwritten with Korean translations** during the i18n sync commit (`1dc5a03: fix(web): i18n — bundled translations, fixed missing keys, EN/KO sync`). Both `en.json` and `ko.json` contained identical Korean text, making comparison impossible.

### Resolution
1. **Restored `en.json`** from the original English source (commit `d310a6f`, which was in TS format) and added proper English values for 203 new keys that were added after the original.
2. **Verified `ko.json`** is already fully translated to Korean — all 879 keys have proper Korean translations.

### Translation Statistics

| Metric | Value |
|--------|-------|
| Total keys | 879 |
| Korean translated | 879 (100%) |
| Intentionally same as English (technical terms) | 9 |
| New keys added (vs original) | 203 |

### Intentionally Untranslated (Technical Terms / Brand Names)
These correctly remain identical in both languages:
- `common.git` = "Git" (brand name)
- `common.oxiosBrand` = "Oxios Agent OS" (brand name)
- `settings.jsonElkLoki` = "JSON (ELK/Loki)" (technical format)
- `settings.groupAi` = "AI" (abbreviation)
- `engine.ctx` = "ctx" (abbreviation)
- `resources.cpu` = "CPU" (abbreviation)
- `sessions.id` = "ID" (abbreviation)
- `a2a.direction` = "From → To" (directional notation)
- `git.title` = "Git" (brand name)

### New Sections Added (203 keys)
These were added after the original English source and already have Korean translations:
- `common.*` — 10 new common UI strings
- `settings.routing.*` — 10 model routing config strings
- `settings.group*` — 5 setting group labels
- `engine.*` — 2 engine state strings
- `agents.*` — 37 agent detail/trace strings + `logLevel` sub-object
- `seeds.*` — 28 ouroboros phase/evaluation strings
- `sessions.*` — 3 session management strings
- `skills.*` — 13 skill management strings
- `budget.*` — 18 budget management strings
- `agentGroups` — 10 new section (agent group monitoring)
- `a2a` — 13 new section (A2A protocol monitor)
- `memory.*` — 64 memory tier/dream/management strings

### Files Modified
- `surface/oxios-web/web/src/i18n/locales/en.json` — Restored to proper English
- `surface/oxios-web/web/src/i18n/locales/ko.json` — Verified complete (no changes needed)
- `ko-translated.json` — Output copy of the complete Korean translation
