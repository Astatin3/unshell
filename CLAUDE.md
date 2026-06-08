
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
- Save progress, checkpoint, resume → invoke context-save or context-restore
- Code quality, health check → invoke health

## Execution standards

- Use exact, current dependency versions. Never guess package versions. Check the latest available release before adding or updating any dependency. If a version conflict appears, first try to resolve it while keeping dependencies on their latest compatible releases. Only fall back to an older version when the conflict is truly unavoidable, and explain why.
- Leave the project warning-free. Fix all compiler, linter, and tooling warnings before finishing. If a warning cannot be eliminated cleanly, silence it in the narrowest possible scope and add a short rationale.
- Document code thoroughly. Add rustdoc, module docs, examples, and inline comments where they improve comprehension. Public APIs should be documented with clear meaning and examples. Non-obvious internal logic should also be documented. Comments should explain intent, invariants, and behavior, not restate syntax.
- Maintain clear architecture. Do not allow files or functions to grow without bound. When code becomes too large or mixes concerns, split it into smaller modules, helper files, or folders with clear names. Prefer structure that improves readability, navigation, and maintenance. 
- If a file is longer than 500 lines, split it up however seen fit. Create a rust module in place of the file, then split each component of the file into it's own file. Split utils into their own files. If it's a really big struct, split the functions into their own files with pub(super) to prevent confusion.
- If a function is longer than 150 lines, it must be split up as well. In this case, create a master function around multiple 'steps' to this larger one, describing in more detail how it works with appropriate comments.
- Research library behavior when needed. Do not assume library APIs, feature flags, version compatibility, or known issues. Verify them, including online research when appropriate, before making decisions.
- Commit at every real milestone when implementation is allowed and the user has not forbidden commits. Create a local git commit each time a meaningful milestone is reached. Commit messages must be accurate, specific, and reflect the actual change.
- Explain unintuitive choices. Whenever an implementation, algorithm, or control flow could appear backwards, surprising, or overly indirect, add a short rationale comment or documentation note explaining why it is correct.
- Track work with TODOs. Use a task list throughout the work so progress, remaining steps, and milestone boundaries stay explicit.
- Maintain ignored project learnings in `learnings/`. During every non-trivial task, read relevant existing markdown files in `learnings/` before making broad changes, then update or create concise markdown records when you discover durable facts, architectural decisions, verified commands, pitfalls, or user preferences. Prefer updating an existing record over creating a duplicate. Keep these records factual and specific: include file paths, feature names, command results, decisions, and rationale. Because `learnings/` is intentionally gitignored, do not stage or commit those files unless the user explicitly asks.
- ALL Sub-agents must be told to read this file before continuing.

## Comments

Because everything must be documented, comments should look like the below. This is a very unimportant function that isn't called often. Use significantly more description for more important ones.

```rust
/// Attaches `strace` to `process` and decodes reads/writes on `fd`.
///
/// This is passive: it observes the legacy host's serial traffic and never
/// writes to the MCU device. It requires permission to attach to the target
/// process and will return an error if the process is not running.
pub fn trace_serial(process: &str, fd: u32) -> io::Result<()> { ... }
```

```rust
/// Human-readable mapping for Elegoo `DeviceSensorStatus` sensor ids.
///
/// Source trail:
/// - `serial-test/src/protocol/device_sensor_status.rs` shows `0x48` starts with
///   a stable `sensor_id` and existing traces contain ids `0`, `1`, and `3`.
/// - `config/cc2/printer_dsp.cfg` defines the corresponding CC2 sensors:
///   `[ztemperature_sensor box] sensor_pin=PH0 #GPADC0`, `[heater_bed]
///   sensor_pin=PH1 #GPADC1`, and `[extruder] sensor_pin=toolhead:PA3`.
/// - `serial-test` samples show sensor id `1` carrying extruder up/down telemetry
///   markers (`0x96`/`0x97`), so id `1` is the toolhead/extruder stream.
///
/// This is deliberately separate from `0x3d` live status: live `0x3d` fields are
/// useful telemetry, but they are not stable object ids in the captured stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensorName { ... }
```

Add documentation for not what each struct and function does, but WHY as well. It's very important for debug purposes.

In the case that a function is either user-facing in a library, or is used widely enough in a project to be considered a reference, add comments describing an example in how to use the function or struct.

Also, add documentation inside of functions like the below:

```rust
pub fn is_watertight(&self) -> bool {
    // Create the map of edges with an approximate amount of unique edges
    let mut edge_map: AHashMap<(usize, usize), usize> =
        AHashMap::with_capacity(self.indices.len() * 3 / 2);
    
    let mut check_edge = |a: usize, b: usize| {
        // Always choose smaller edge first
        let (a, b) = if b < a { (b, a) } else { (a, b) };
    
        // Find the pair of edges in the hash map
        *edge_map.entry((a, b)).or_insert(0) += 1;
    };
    
    // Check each edge on each triangle
    for (a, b, c) in &self.indices {
        check_edge(*a, *b);
        check_edge(*b, *c);
        check_edge(*a, *c);
    }
    
    // Check if all edges come in pairs
    edge_map.iter().all(|(_, checked)| *checked == 2)
}
```

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
- relevant `learnings/` markdown files were reviewed and updated with durable new facts
- progress was tracked
- the milestone was committed locally if implementation was allowed
