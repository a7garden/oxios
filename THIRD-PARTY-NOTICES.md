# Third-Party Notices

This file contains third-party copyright notices and license information
for software whose ideas, concepts, or code have been incorporated into
the Oxios project.

---

## Ouroboros

- **Repository:** https://github.com/Q00/ouroboros
- **Copyright:** Copyright (c) 2026 Q00
- **License:** MIT License

```
MIT License

Copyright (c) 2026 Q00

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### Usage in Oxios

The `oxios-ouroboros` crate incorporates the **specification-first protocol concept**
(assess → crystallize → execute → review) and the **ambiguity scoring model**
from the Ouroboros project by Q00. The Rust implementation in Oxios is original —
no source code was copied. The concept, naming, and protocol phases are derived from
the Ouroboros specification framework.

---

## Anthropic `skill-creator`

- **Repository:** https://github.com/anthropics/skills/tree/main/skills/skill-creator
- **Copyright:** Copyright (c) Anthropic, PBC
- **License:** Apache License 2.0 — https://www.apache.org/licenses/LICENSE-2.0

```
Copyright (c) Anthropic, PBC

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

### Usage in Oxios

The bundled `skill-creator` default skill (`share/default-skills/skill-creator/`)
incorporates the skill-authoring methodology from Anthropic's `skill-creator`:
the SKILL.md anatomy, the three-level progressive-disclosure model, the writing
guide, and the with/without-skill evaluation workflow. The `references/schemas.md`
JSON schemas and the `agents/{grader,analyzer,comparator}.md` subagent prompts are
ported from the original.

**Modifications:** The deterministic operations that Anthropic implements as
Python scripts (`quick_validate.py`, `package_skill.py`, `aggregate_benchmark.py`,
`generate_review.py`) are reimplemented from scratch in Rust as the native
`skill_forge` agent tool — no Python source was copied. The subagent prompts were
adapted from Claude-Code-specific wording to tool-agnostic agent-run framing so
they work under Oxios's fork/exec + task-subagent model. The LLM grading and
description-improvement steps remain agent-orchestrated (an Oxios agent spawns a
grader/improver subagent), matching Anthropic's original agent-spawns-grader design.
