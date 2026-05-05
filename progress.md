# Phase 0-B: Documentation ClawGarden Cleanup — COMPLETE

## Changes Made

### DESIGN.md (10 replacements)
- `Container Garden (Apple Container)` → `Container (Apple Container)`
- Unix↔Oxios table: `Garden` → `Container`
- Crate structure: `garden.rs` → `container_manager.rs`, `GardenManager` → `ContainerManager`
- Responsibilities: `Garden Manager` → `Container Manager`
- Dependencies: `pi2oxi/oxi-ai` → `oxi/oxi-ai`, `pi2oxi/oxi-agent` → `oxi/oxi-agent`
- Container Isolation section: `Each Garden` → `Each Container`, CLI `garden` → `container`
- Command Interface: all `oxios garden` → `oxios container`
- Build Order: `Garden lifecycle` → `Container lifecycle`, `Gardens` → `Containers`
- Project Info: `pi2oxi path dependency` → `oxi path dependency`

### AGENTS.md (7 replacements)
- Architecture: `Container Garden` → `Container`
- Architecture: `garden per project` → `container per project`
- Engine: `../pi2oxi/` → `../oxi/`
- Kernel Modules table: `garden | Garden lifecycle manager` → `container_manager | Container lifecycle manager`
- Key Principles #3: `pi2oxi` → `oxi`
- Dependency Map: `../pi2oxi/oxi-agent` → `../oxi/oxi-agent`, `../pi2oxi/oxi-ai` → `../oxi/oxi-ai`
- Best Practices: `Test in garden` → `Test in container`

## Preserved (intentional)
- All `ClawGarden`/`clawgarden` references in AGENTS.md §"Reusable Code from ClawGarden" — these reference the external project source, not our naming

## Verification
- `grep -in "garden" DESIGN.md AGENTS.md` returns only `clawgarden` references
- `grep -in "pi2oxi" DESIGN.md AGENTS.md` returns zero results
