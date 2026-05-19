# Oxios Design System

> shadcn/ui + Tailwind CSS v4, Zinc base, OKLCH color space, New York style.

## Design Tokens

### Base Color: Zinc

Oxios는 Agent OS -- 기술적이고 전문적인 제품. Zinc의 중성적인 톤이 UI의 내용이 돋보이게 한다.

### Color Space: OKLCH

모든 색상은 OKLCH 컬러 스페이스로 정의:
- 지각적 균일성 (Perceptual uniformity)
- Dark/Light 모드 간 일관된 느낌
- 미래 CSS 표준

### Radius

```
--radius: 0.625rem (기본)
```

Scale: sm(0.375) / md(0.5) / lg(0.625) / xl(0.875) / 2xl(1.125)

### Typography

```css
font-family: 'Inter', system-ui, -apple-system, sans-serif;
```

## Token Reference

| Token | Role | Light | Dark |
|-------|------|-------|------|
| background | Page surface | White | Zinc 950 |
| foreground | Default text | Zinc 950 | White |
| card | Elevated surface | White | Zinc 900 |
| primary | Actions, brand | Zinc 900 | White |
| secondary | Supporting fill | Zinc 100 | Zinc 800 |
| muted | Subtle surface | Zinc 100 | Zinc 800 |
| accent | Hover, active | Zinc 100 | Zinc 800 |
| destructive | Errors, danger | Red 600 | Red 500 |
| border | Separators | Zinc 200 | White 10% |

## Component Patterns

### Status Colors (Custom Tokens)

```
success  -> emerald  -- Running, Active, Approved
warning  -> amber    -- Idle, Pending
error    -> red      -- Stopped, Failed, Rejected
neutral  -> zinc     -- Archived, Disabled
```

### Badge Variants

| Variant | Usage |
|---------|-------|
| default | General |
| secondary | Supporting info |
| destructive | Errors |
| outline | Neutral state |
| success | Positive state |
| warning | Caution state |

### Button Variants

| Variant | Usage |
|---------|-------|
| default | Primary actions |
| destructive | Delete, kill |
| outline | Secondary actions |
| secondary | Tertiary actions |
| ghost | Inline, toolbar |
| link | Navigation |

## File Locations

| File | Purpose |
|------|---------|
| `src/index.css` | Theme tokens, OKLCH variables |
| `components.json` | shadcn/ui configuration |
| `src/components/ui/` | Base UI components |
| `src/lib/utils.ts` | cn() helper |

## Adding Components

```bash
# shadcn/ui CLI로 컴포넌트 추가
bunx shadcn@latest add [component-name]

# 예시
bunx shadcn@latest add dialog
bunx shadcn@latest add dropdown-menu
bunx shadcn@latest add select
bunx shadcn@latest add progress
bunx shadcn@latest add sonner
```
