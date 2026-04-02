# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

`symref` is a Rust CLI tool for symbolic variable storage and dereferencing. It stores validated key-value pairs as symbolic `$VAR` references in a session-scoped JSON file, and substitutes those references in text or JSON content on demand.

symref is a component of the **agent-sentinel** security framework, implementing the **symbolic dereferencing** aspect of the Dual LLM pattern from the "Design Patterns for Securing LLM Agents against Prompt Injections" paper. In this pattern, a privileged LLM works with opaque `$VAR` references rather than raw untrusted content. symref manages the variable store and performs substitution at execution time.

**Key properties:**
- Deterministic — no LLM, no network, pure data transformation
- Session-scoped — variables persisted in a JSON file per session
- Composable — designed to be called from shell hook scripts via stdin/stdout

## Common Commands

### Building
```bash
cargo build
cargo build --release
```

### Testing
```bash
cargo test
cargo test -- --nocapture
```

### Linting
```bash
cargo +nightly fmt --check
RUSTFLAGS="-D warnings" cargo check
cargo clippy -- -D warnings
```

### Quick CI check
```bash
cargo +nightly fmt --check && RUSTFLAGS="-D warnings" cargo check && cargo clippy -- -D warnings && RUSTFLAGS="-D warnings" cargo test
```

## CLI Interface

symref exposes two subcommands:

### `symref store`

Reads a validated JSON object from a file (or stdin), assigns symbolic `$PREFIX_FIELD_N` references to each value, appends them to the session's variable store (vars.json), and writes a summary + refs JSON to stdout.

```bash
symref store \
  --session /path/to/session-dir \
  --prefix TC42 \
  --input validated.json

# Or from stdin:
cat validated.json | symref store --session /path/to/session-dir --prefix TC42
```

**Input** (validated.json — flat or nested JSON object):
```json
{
  "requirements": [
    {"id": "REQ_1", "summary": "OAuth2 login flow", "priority": "high"},
    {"id": "REQ_2", "summary": "Session persistence", "priority": "medium"}
  ],
  "acceptance_criteria": [
    {"id": "AC_1", "description": "Users can authenticate via OAuth2"}
  ]
}
```

**Output** (stdout — summary + $VAR refs):
```json
{
  "refs": {
    "$TC42_REQ_1": {
      "summary": "OAuth2 login flow",
      "ref": "$TC42_REQ_1"
    },
    "$TC42_REQ_2": {
      "summary": "Session persistence",
      "ref": "$TC42_REQ_2"
    },
    "$TC42_AC_1": {
      "summary": "Users can authenticate via OAuth2",
      "ref": "$TC42_AC_1"
    }
  },
  "store_path": "/path/to/session-dir/vars.json"
}
```

**Side effect**: appends entries to `<session-dir>/vars.json`:
```json
{
  "$TC42_REQ_1": {"id": "REQ_1", "summary": "OAuth2 login flow", "priority": "high"},
  "$TC42_REQ_2": {"id": "REQ_2", "summary": "Session persistence", "priority": "medium"},
  "$TC42_AC_1": {"id": "AC_1", "description": "Users can authenticate via OAuth2"}
}
```

**Naming convention**: `$PREFIX_ARRAYFIELD_N` where:
- `PREFIX` comes from `--prefix` (e.g., `TC42`)
- `ARRAYFIELD` is the JSON array field name uppercased (e.g., `requirements` → `REQ`, `acceptance_criteria` → `AC`)
- `N` is the 1-based index within the array

For non-array fields, the reference is `$PREFIX_FIELD` (e.g., `$TC42_BACKGROUND` for a `background` string field).

### `symref deref`

Reads text or JSON containing `$VAR` references from a file (or stdin), substitutes each reference with its stored value from vars.json, and writes the concrete result to stdout.

```bash
symref deref \
  --session /path/to/session-dir \
  --input template.json

# Or from stdin:
echo "Implement $TC42_REQ_1" | symref deref --session /path/to/session-dir
```

**Input** (template.json — JSON with $VAR references):
```json
{
  "summary": "Implement OAuth2 login",
  "description": "## Acceptance Criteria\n- $TC42_AC_1\n\n## Requirements\n- $TC42_REQ_1"
}
```

**Output** (stdout — concrete values substituted):
```json
{
  "summary": "Implement OAuth2 login",
  "description": "## Acceptance Criteria\n- Users can authenticate via OAuth2\n\n## Requirements\n- OAuth2 login flow"
}
```

When a `$VAR` reference is substituted and the stored value is a JSON object (not a scalar), the substitution behavior depends on context:
- **In a JSON string value**: the object is serialized to a compact JSON string
- **In plain text**: the object's most representative field is used (first of: `summary`, `description`, `text`, `value`, or the first string field found)

Unresolved references (not found in vars.json) are left as-is in the output and logged to stderr as warnings.

## Architecture

### Core Dependency

The `subst` crate ([docs.rs/subst](https://docs.rs/subst), BSD-2-Clause OR Apache-2.0) provides the substitution engine:

- `$VAR` and `${VAR}` syntax (shell-style variable references)
- Custom `HashMap` variable maps (not limited to env vars)
- Substitution inside JSON and YAML values (feature-gated: `json`, `yaml`)
- `VariableMap` trait for custom lookup behavior

symref wraps `subst` with:
- A file-backed variable store (`vars.json`)
- A CLI interface (`clap`)
- The `store` command (ingestion + reference assignment)
- The `deref` command (lookup + substitution)

### Data Flow

```
store command:
  input JSON → parse → assign $VAR names → append to vars.json → output refs

deref command:
  input text/JSON + vars.json → subst crate substitution → output concrete text/JSON
```

### Dependencies

| Crate | Purpose | Version constraint |
|---|---|---|
| `subst` | `$VAR` substitution engine | latest, with `json` feature |
| `clap` | CLI argument parsing (derive API) | 4.x |
| `serde` | Serialization framework | 1.x |
| `serde_json` | JSON parsing and generation | 1.x |
| `anyhow` | Error handling | 1.x |

### Project Structure

```
symref/
├── Cargo.toml
├── CLAUDE.md
├── src/
│   ├── main.rs         ← CLI entry point (clap subcommands)
│   ├── store.rs        ← store command: ingest JSON, assign refs, write vars.json
│   ├── deref.rs        ← deref command: load vars.json, substitute via subst crate
│   ├── naming.rs       ← $VAR naming convention (PREFIX_FIELD_N generation)
│   └── types.rs        ← shared types (VarStore, VarRef, StoreOutput)
└── tests/
    ├── store_tests.rs  ← store command tests
    ├── deref_tests.rs  ← deref command tests
    └── fixtures/       ← test JSON files
```

## Testing Strategy

### Unit Tests

- **naming.rs**: test $VAR name generation from JSON structure
  - Array fields → `$PREFIX_FIELD_N` (1-based)
  - Scalar fields → `$PREFIX_FIELD`
  - Nested objects → flatten or use dot notation
  - Edge cases: empty arrays, missing fields, special characters

- **store.rs**: test ingestion and vars.json writing
  - Fresh store (no existing vars.json)
  - Append to existing store (merge, don't overwrite)
  - Different prefixes coexist
  - Output format (refs JSON)

- **deref.rs**: test substitution
  - Simple `$VAR` in plain text
  - `$VAR` in JSON string values
  - `${VAR}` syntax (braces)
  - Unresolved references → left as-is + stderr warning
  - Object values → summary extraction for plain text context
  - Nested substitutions (value contains another `$VAR` — should NOT recurse)

### Integration Tests

- Round-trip: `store` then `deref` produces expected output
- Stdin/stdout piping: `cat input.json | symref store ... | symref deref ...`
- Multiple prefixes in one session
- Large stores (hundreds of variables)

## Important Implementation Notes

### vars.json Format

The store is a flat JSON object. Keys are `$VAR` names (including the `$` prefix). Values are the original JSON values (objects, strings, numbers, etc.):

```json
{
  "$TC42_REQ_1": {"id": "REQ_1", "summary": "OAuth2 login flow", "priority": "high"},
  "$TC42_REQ_2": {"id": "REQ_2", "summary": "Session persistence", "priority": "medium"},
  "$TC42_AC_1": {"id": "AC_1", "description": "Users can authenticate via OAuth2"},
  "$TC42_BACKGROUND": "Implement user authentication for the platform"
}
```

### Concurrency

symref is called sequentially from hook scripts (one tool call at a time within a Claude Code session). No concurrent access to vars.json is expected. File locking is not required but a simple advisory lock (flock) is acceptable as defense-in-depth.

### Error Handling

- Invalid JSON input → exit 1 with error message to stderr
- Missing `--session` directory → exit 1
- Missing vars.json on `deref` → exit 1 (store must be called first)
- Unresolved `$VAR` reference → warning to stderr, leave reference as-is in output, exit 0 (partial success)

### No Network, No LLM

symref must never make network calls or invoke LLMs. It is a pure data transformation tool. All dependencies must be offline-capable. The `subst` crate satisfies this (no network features).
