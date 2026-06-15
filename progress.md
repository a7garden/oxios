# Progress Update — Chat Input Mobile + DataTable Card View

## Completed

### New file
- **`src/hooks/use-is-touch.ts`** — Touch detection hook using `matchMedia('(pointer: coarse)')` with `ontouchstart` fallback

### Modified files

1. **`src/components/chat/chat-input.tsx`**
   - Imported `useIsTouch` from `@/hooks/use-is-touch`
   - Added `const isTouch = useIsTouch()` call
   - Wrapped Enter-to-send with `if (!isTouch)` guard — touch users won't accidentally send on Enter
   - Added `hidden sm:block` to keyboard shortcut hint (hides hint on mobile)
   - Changed Send button from `h-8 w-8` to `h-11 w-11 rounded-lg` (large touch target) with `sm:h-9 sm:w-9` (restore on desktop)

2. **`src/components/shared/data-table.tsx`**
   - Added `mobilePriority?: 'primary' | 'secondary' | 'hidden'` to `Column<T>` interface
   - Added `mobileCardView?: boolean` to `DataTableProps<T>` interface
   - Added `CardRow<T>` helper component — renders primary + secondary fields as a card
   - Table section: `overflow-x-auto` gets conditional `hidden md:block` when `mobileCardView` is set
   - Added mobile card view section after pagination: `<div className="divide-y md:hidden">` with `CardRow` elements

3. **`src/routes/agents/index.tsx`**
   - Imported `Column` type
   - Added explicit `Column<AgentListItem>[]` type
   - Added `mobilePriority` on all 7 columns: name=primary, status=secondary, cost=secondary, duration=hidden, created=hidden, session=hidden, tokens=secondary

4. **`src/routes/sessions/index.tsx`**
   - Imported `Column` type
   - Added explicit `Column<Session>[]` type
   - Added `mobilePriority` on all 7 columns: id=hidden, title=primary, agent=secondary, messages=secondary, createdAt=hidden, updatedAt=hidden, ''=hidden

5. **`src/routes/seeds/index.tsx`**
   - Imported `Column` type
   - Added explicit `Column<Seed>[]` type
   - Added `mobilePriority` on all 3 columns: goal=primary, constraints=secondary, created=hidden

## Typecheck
- 0 new errors in modified files (10 pre-existing errors in unrelated files remain)
