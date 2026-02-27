# AGENTS.md

## Tools

### Context7

Provides documentation for libraries, frameworks, platforms, and other tech. Use it as grounding for all code planning and generation tasks.

### GitNexus

This repo is indexed by GitNexus. Use the provided GitNexus skills for tasks involving codebase understanding, debugging, impact analysis, and refactoring.

> If `gitnexus://repo/{name}/context` warns the index is stale, run `npx gitnexus analyze` in the terminal before continuing.

**Tools:**
    -`query` Process-grouped code intelligence — execution flows related to a concept
    -`context` 360-degree symbol view — categorized refs, processes it participates in
    -`impact` Symbol blast radius — what breaks at depth 1/2/3 with confidence
    -`detect_changes` Git-diff impact — what do your current changes affect
    -`rename` Multi-file coordinated rename with confidence-tagged edits
    -`cypher` Raw graph queries (read `gitnexus://repo/{name}/schema` first)
    -`list_repos` Discover indexed repos

**Resources:**
    - `gitnexus://repo/{name}/context` Stats, staleness check
    - `gitnexus://repo/{name}/clusters` All functional areas with cohesion scores
    - `gitnexus://repo/{name}/cluster/{clusterName}` Area members
    - `gitnexus://repo/{name}/processes` All execution flows
    - `gitnexus://repo/{name}/process/{processName}` Step-by-step trace
    - `gitnexus://repo/{name}/schema` Graph schema for Cypher

**Graph schema:**
    - **Nodes**: File, Function, Class, Interface, Method, Community, Process
    - **Edges (via CodeRelation.type)**: CALLS, IMPORTS, EXTENDS, IMPLEMENTS, DEFINES, MEMBER_OF, STEP_IN_PROCESS

```cypher
MATCH (caller)-[:CodeRelation {type: 'CALLS'}]->(f:Function {name: "myFunc"})
RETURN caller.name, caller.filePath
```

### Python
If you need to perform computations as part of your reasoning (e.g., arithmetic, comparisons, logic, string manipulation, datetime operations, etc.), you MUST perform them using `python3` in your terminal, regardless of how trivial. Never do "mental math".

---

## Coding practices

### 1. General rules

- two near-identical entities that are heavily referenced should be refactored
- two near-identical entities that are referenced a few times only are ok, when the third of these entities appears, refactor
- keep definitions small and concerns separate
- aim for homogeneous dependency direction per module/crate; avoid circular dependencies (direct or indirect)
- keep it simple: don't add complexity for the sake of principles until you need it
- name things clearly: avoid encodings (`foos` is better than `fooList`) and abbreviations; use concise and descriptive names; named constants are better than magic values, but don't be silly about it (e.g., `square_perimeter = side_length * 4` is ok, no need for `NUMBER_OF_SIDES_IN_A_SQUARE`)
- **only write the code you need right now unless the user explicitly requests otherwise; no proactive backwards/forward compatibility**
- **use common sense for assertions, validation, and tests; don't guard against absurd cases**

### 2. Simple control flow

- keep core logic straightforward; if you use recursion, ensure depth is clearly bounded (or convert to an explicit stack/loop)
- avoid heavy macro magic or overly clever iterator chains

### 3. Handle untrusted inputs / hot paths carefully


Selectively apply these to critical code:

**3.1 Use bounded loops:**
- when looping over untrusted input (network, files, user data): enforce caps (`take(n)`, maximum sizes, timeouts)
- when looping in latency-sensitive paths: cap retries/backoff attempts

**3.2 Avoid surprise allocation:**
- pre-allocate (`Vec::with_capacity`, `String::with_capacity`)
- reuse buffers (pools, arenas) where performance predictability matters
- measure/limit allocations in request handlers, audio/video loops, render frames, etc.

### 4. Keep functions short

- split parsing/validation/business logic
- push complexity into well-named helpers or modules
- prefer explicit types/newtypes over long “do everything” functions

### 5. Use assertions to check “should never happen”

- use `debug_assert!` for internal invariants
- treat `unwrap()`/`expect()` as assertions: keep them in tests, prototypes, or clearly justified “can’t happen” boundaries
- prefer returning errors over panicking in libraries/services, unless the user has confirmed crashing is an acceptable recovery strategy
- it's okay to loosen assertion thresholds for less critical code

### 6. Smallest possible scope for data

- minimize `mut`
- avoid wide-scope shared state
- keep unsafe globals (`static mut`) out entirely unless you really need them

### 7. Check return values; validate parameters

- at boundaries: validate inputs early (sizes, ranges, UTF-8 expectations, invariants)
- inside: enforce “don’t ignore” with `#[must_use]` and Clippy lints where critical
- prefer types that make invalid states unrepresentable

### 8. Limit macro/`cfg` complexity

- keep macros small and obvious (or replace with functions/generics)
- keep `#[cfg(feature = …)]` from fracturing core logic into many variants
- be extra cautious with proc macros in core correctness paths

### 9. Restrict `unsafe` and raw pointers

- prefer safe Rust, keep `unsafe` in small, well-audited modules
- build safe wrappers around FFI/raw pointer code
- avoid complex aliasing and pointer arithmetic unless unavoidable

### 10. Zero warnings + strong static analysis

- CI gates: `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`
- add: dependency auditing (`cargo-audit`/`cargo-deny`), sanitizers, Miri for tricky UB, fuzzing for parsers
