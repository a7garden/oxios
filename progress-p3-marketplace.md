# Progress: Marketplace Cleanup + Skills Enhancement (Step 4)

**Date:** 2026-05-30
**Status:** ✅ Complete

## Summary

Replaced the full marketplace page with a redirect, enhanced the skills page with detail panels, update checking, enable/disable toggles, and delete functionality.

## Changes Made

### 1. Marketplace Redirect (`web/src/routes/marketplace.tsx`)
- Replaced full marketplace page component with a simple `<Navigate>` to `/skills?search={{ tab: 'marketplace' }}`
- Uses TanStack Router's `validateSearch` to read the `tab` param on the skills page

### 2. Enhanced Skills Page (`web/src/routes/skills.tsx`)

#### New Features:
- **Tab state from URL**: `validateSearch` reads `?tab=marketplace` from URL search params
- **Selected skill state**: `selectedSkill` / `selectedMktSlug` for side panel display
- **Side panel layout**: Grid switches from 1-column to `[1fr_380px]` when a skill is selected
- **Update check integration**: Uses `useSkillUpdates` hook, passes update data to card components
- **Enable/Disable toggle**: Inline buttons on each skill card using `POST /api/skills/{name}/enable` and `POST /api/skills/{name}/disable`
- **Delete with confirmation**: Delete button on cards + in detail panel, with Dialog confirmation
- **Update badge on tab**: Marketplace tab shows update count badge
- **Update indicator on cards**: Cards show "Update available" badge when updates exist
- **Card click selection**: Cards highlight with `ring-2 ring-primary` when selected
- **Action stop propagation**: Inline action buttons don't trigger card selection

### 3. New Components

#### `components/skills/skill-detail.tsx`
- Side panel showing full skill details when an installed skill is selected
- Displays: name, description, format badge, source, version, author, homepage, path, OS, requirements, install specs
- Update indicator badge
- Enable/Disable toggle button
- Delete button with confirmation dialog

#### `components/skills/marketplace-detail.tsx`
- Side panel showing full marketplace skill detail
- Fetches from `GET /api/marketplace/skills/{slug}` 
- Displays: name, summary, version info, changelog, owner info, tags, metadata (OS, systems)
- Install button

#### `components/skills/update-badge.tsx`
- `useSkillUpdates()` hook — queries `GET /api/marketplace/updates` every 5 minutes
- `UpdateBadge` — count badge for tab button
- `SkillUpdateIndicator` — per-slug update indicator badge

### 4. i18n Keys Added

**English (`en/common.json`):**
- `skills.installSuccess`, `skills.detail`, `skills.enable`, `skills.disable`
- `skills.delete`, `skills.deleteConfirm`, `skills.deleteDescription`, `skills.deleteSuccess`
- `skills.updatesAvailable`, `skills.updateAvailable`, `skills.noUpdates`
- `skills.version`, `skills.toggleSuccess`

**Korean (`ko/common.json`):**
- Same keys with Korean translations

### 5. Backend Routes Used (all pre-existing)
| Route | Method | Purpose |
|-------|--------|---------|
| `/api/skills` | GET | List installed skills |
| `/api/skills/{name}/enable` | POST | Enable a skill |
| `/api/skills/{name}/disable` | POST | Disable a skill |
| `/api/skills/{name}` | DELETE | Delete a skill |
| `/api/marketplace/search` | GET | Search marketplace |
| `/api/marketplace/skills/{slug}` | GET | Marketplace skill detail |
| `/api/marketplace/skills/{slug}/install` | POST | Install from marketplace |
| `/api/marketplace/updates` | GET | Check for updates |

## Files Modified
- `web/src/routes/marketplace.tsx` — Rewritten as redirect
- `web/src/routes/skills.tsx` — Enhanced with detail panels, toggles, delete, update badges
- `web/public/locales/en/common.json` — Added 15 new i18n keys
- `web/public/locales/ko/common.json` — Added 15 new i18n keys (Korean)

## Files Created
- `web/src/components/skills/skill-detail.tsx` — Installed skill detail panel
- `web/src/components/skills/marketplace-detail.tsx` — Marketplace skill detail panel  
- `web/src/components/skills/update-badge.tsx` — Update check hook + badge components

## Verification
- TypeScript compilation: No errors in new/modified files (pre-existing errors in unrelated files remain)
- All new API calls match existing backend routes in `marketplace.rs` and `workspace.rs`
