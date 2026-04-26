
## Skill routing

When the user's request matches an available skill, ALWAYS invoke it using the Skill
tool as your FIRST action. Do NOT answer directly, do NOT use other tools first.
The skill has specialized workflows that produce better results than ad-hoc answers.

Key routing rules:
- Product ideas, "is this worth building", brainstorming → invoke office-hours
- Bugs, errors, "why is this broken", 500 errors → invoke investigate
- Ship, deploy, push, create PR → invoke ship
- QA, test the site, find bugs → invoke qa
- Code review, check my diff → invoke review
- Update docs after shipping → invoke document-release
- Weekly retro → invoke retro
- Design system, brand → invoke design-consultation
- Visual audit, design polish → invoke design-review
- Architecture review → invoke plan-eng-review
- Save progress, checkpoint, resume → invoke checkpoint
- Code quality, health check → invoke health

## Execution standards

- Use exact, current dependency versions. Never guess package versions. Check the latest available release before adding or updating any dependency. If a version conflict appears, first try to resolve it while keeping dependencies on their latest compatible releases. Only fall back to an older version when the conflict is truly unavoidable, and explain why.
- Leave the project warning-free. Fix all compiler, linter, and tooling warnings before finishing. If a warning cannot be eliminated cleanly, silence it in the narrowest possible scope and add a short rationale.
- Document code thoroughly. Add rustdoc, module docs, examples, and inline comments where they improve comprehension. Public APIs should be documented with clear meaning and examples. Non-obvious internal logic should also be documented. Comments should explain intent, invariants, and behavior, not restate syntax.
- Maintain clear architecture. Do not allow files or functions to grow without bound. When code becomes too large or mixes concerns, split it into smaller modules, helper files, or folders with clear names. Prefer structure that improves readability, navigation, and maintenance.
- Research library behavior when needed. Do not assume library APIs, feature flags, version compatibility, or known issues. Verify them, including online research when appropriate, before making decisions.
- Commit at every real milestone. Create a local git commit each time a meaningful milestone is reached. Commit messages must be accurate, specific, and reflect the actual change.
- Explain unintuitive choices. Whenever an implementation, algorithm, or control flow could appear backwards, surprising, or overly indirect, add a short rationale comment or documentation note explaining why it is correct.
- Track work with TODOs. Use a task list throughout the work so progress, remaining steps, and milestone boundaries stay explicit.
- ALL Sub-agents must be told to read this file before continuing.

## Plan mode rules

- Plan mode is strictly read-only. When plan mode is active, do not edit files, write output files, change configuration, make commits, or perform any system modifications.
- Read, inspect, and plan only. In plan mode, searching, reading, analysis, and clarifying questions are allowed. Building and presenting a plan is allowed. Executing the plan is not.
- Do not override read-only restrictions. Even if another instruction suggests implementation, plan mode takes priority until the user explicitly exits planning.

## Decision-making expectations

- Ask clarifying questions when requirements are ambiguous, especially before making broad architectural assumptions.
- Prefer minimal, correct solutions over unnecessarily complex ones.
- If a tradeoff matters, surface it explicitly instead of silently choosing one path.

## Definition of done

A task is not complete unless:

- dependency versions were verified
- warnings were removed or narrowly silenced with rationale
- code structure is readable and appropriately split
- documentation, comments, and examples were added where needed
- unintuitive logic has rationale
- progress was tracked
- the milestone was committed locally if implementation was allowed
