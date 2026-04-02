# symref

Symbolic variable storage and dereferencing.

symref implements the **symbolic dereferencing** aspect of the Dual LLM pattern from the paper [Design Patterns for Securing LLM Agents against Prompt Injections](https://arxiv.org/pdf/2506.08837). In this pattern, a privileged LLM works with opaque `$VAR` references rather than raw untrusted content. symref manages the variable store and performs substitution at execution time.

**Key properties:**
- **Deterministic** -- no LLM, no network, pure data transformation
- **Session-scoped** -- variables persisted in a JSON file per session
- **Composable** -- designed to be called from shell hook scripts via stdin/stdout

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
# Binary at target/release/symref
```

## Usage

symref exposes two subcommands: `store` and `deref`.

### Store: ingest validated JSON and assign symbolic references

```bash
symref store \
  --session /path/to/session-dir \
  --prefix X7F \
  --input validated.json
```

Given this input (`validated.json`):

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

symref assigns symbolic references and writes them to `<session-dir>/vars.json`:

```json
{
  "$X7F_REQ_1": {"id": "REQ_1", "summary": "OAuth2 login flow", "priority": "high"},
  "$X7F_REQ_2": {"id": "REQ_2", "summary": "Session persistence", "priority": "medium"},
  "$X7F_AC_1": {"id": "AC_1", "description": "Users can authenticate via OAuth2"}
}
```

And outputs a summary to stdout:

```json
{
  "refs": {
    "$X7F_REQ_1": { "summary": "OAuth2 login flow", "ref": "$X7F_REQ_1" },
    "$X7F_REQ_2": { "summary": "Session persistence", "ref": "$X7F_REQ_2" },
    "$X7F_AC_1": { "summary": "Users can authenticate via OAuth2", "ref": "$X7F_AC_1" }
  },
  "store_path": "/path/to/session-dir/vars.json"
}
```

Stdin also works:

```bash
cat validated.json | symref store --session ./session --prefix X7F
```

### Deref: substitute references with stored values

```bash
symref deref \
  --session /path/to/session-dir \
  --input template.json
```

Given this template:

```json
{
  "summary": "Implement OAuth2 login",
  "description": "## Acceptance Criteria\n- $X7F_AC_1\n\n## Requirements\n- $X7F_REQ_1"
}
```

Output (concrete values substituted):

```json
{
  "summary": "Implement OAuth2 login",
  "description": "## Acceptance Criteria\n- Users can authenticate via OAuth2\n\n## Requirements\n- OAuth2 login flow"
}
```

Plain text works too:

```bash
echo "Implement $X7F_REQ_1" | symref deref --session ./session
```

### Naming convention

| Input field | Type | Variable name |
|---|---|---|
| `requirements[0]` | Array item | `$PREFIX_REQ_1` |
| `acceptance_criteria[0]` | Array item | `$PREFIX_AC_1` |
| `background` | Scalar | `$PREFIX_BACKGROUND` |

- **Array fields**: `$PREFIX_ABBREV_N` where `ABBREV` is the first letter of each underscore-separated word (or first 3 chars for single words), and `N` is a 1-based index.
- **Scalar fields**: `$PREFIX_FIELD` where `FIELD` is the full field name uppercased.

### Substitution behavior

When a stored value is a JSON object, the most representative field is used for substitution (checked in order: `summary`, `description`, `text`, `value`, then the first string field found).

Unresolved `$VAR` references are left as-is in the output and logged as warnings to stderr.

Both `$VAR` and `${VAR}` syntax are supported.

## Development

```bash
# Build
cargo build

# Test
cargo test

# Lint (full CI check)
cargo +nightly fmt --check && \
  RUSTFLAGS="-D warnings" cargo check && \
  cargo clippy -- -D warnings && \
  RUSTFLAGS="-D warnings" cargo test
```

## Architecture

```
store command:
  input JSON --> parse --> assign $VAR names --> append to vars.json --> output refs

deref command:
  input text/JSON + vars.json --> subst crate substitution --> output concrete text/JSON
```

The [subst](https://docs.rs/subst) crate provides the `$VAR`/`${VAR}` substitution engine. symref wraps it with a file-backed variable store, a CLI interface, and the store/deref commands.

## Acknowledgements

symref is built on top of the [subst](https://crates.io/crates/subst) crate, which provides the `$VAR`/`${VAR}` substitution engine. Big thanks to its maintainers.

## License

See [LICENSE](LICENSE) for details.
