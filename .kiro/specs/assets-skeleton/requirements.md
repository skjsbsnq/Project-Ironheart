# Requirements Document

## Introduction

This spec covers Phase 2.1 of the V3 roadmap: the skeleton of the new `hoi4-assets` crate.
It is a 3-day foundation task whose only deliverable is the **bytes-and-cache layer** that all
of Phase 2.2 through 2.8 (the `.gfx`, DDS, `.fnt`, `.gui`, `.gui` runtime, `.mesh`, and
`.asset` parsers/loaders) will consume. No actual asset format is parsed in this phase.

The crate sits one layer above `hoi4-paths` (Phase 0.1, already implemented). `hoi4-paths`
answers "where on disk does `interface/topbar.gui` resolve to, given the active mod chain?".
`hoi4-assets` answers three further questions:

1. **Bytes**: give me the raw bytes of an asset by its logical path.
2. **Typed cache**: hold a parsed-and-decoded form of an asset so the next caller does not
   re-parse or re-decode, with an LRU eviction policy dimensioned in bytes (because a single
   decoded DDS mip chain can be ~20 MB and there are ~21 000 of them).
3. **Error chain**: when an asset is missing, the error states which file *referred to* it
   (e.g. "`interface/topbar.gui` requested `gfx/interface/icons/oil.dds`, not found in any
   mod or vanilla"), so a designer can trace the dangling reference back to its source.

The crate intentionally does not understand any specific asset format. Each downstream
crate (Phase 2.2-2.8) defines its own parsed type and passes a parse closure into the
cache; the cache stores parsed values as `Arc<dyn Any + Send + Sync>` keyed by
`(TypeId, AssetPath)`. This decouples the cache from per-format details.

## Glossary

- **Asset**: any file under the HOI4 install root or a mod root that the engine consumes
  at runtime. In this phase, an asset is treated as opaque bytes.
- **AssetPath**: a forward-slash, install-root-relative logical path such as
  `interface/topbar.gui` or `gfx/interface/icons/oil.dds`. AssetPaths never contain
  drive letters, never contain `..`, and never start with `/`.
- **AssetDb**: the trait defined by this crate. Its surface is `open(AssetPath)` for raw
  bytes, `list(dir)` for directory enumeration across the mod chain, and
  `get_or_load<T>(AssetPath, parse_fn)` for a typed parse-and-cache slot.
- **FsAssetDb**: the production implementation of `AssetDb`, backed by `hoi4-paths::PathConfig`
  and the local filesystem.
- **TestAssetDb**: an in-memory implementation of `AssetDb` used by unit tests; it is
  populated from a `HashMap<AssetPath, Vec<u8>>` and requires no HOI4 install.
- **ModChain**: the ordered list of mod roots provided by `hoi4-paths::PathConfig`,
  highest-priority first, with vanilla implicitly appended at the end and `replace_path`
  directives respected.
- **Referrer**: a logical breadcrumb describing **which asset triggered a load**. When
  `interface/topbar.gui` is being parsed and asks the cache for `gfx/oil.dds`, the
  topbar's path is the referrer for the dds load. Referrers are pushed and popped via a
  scoped guard.
- **AssetCache**: the LRU-by-bytes container that holds typed parsed values. Capacity is
  expressed in bytes; entries are tagged with a caller-supplied byte cost.
- **AssetError**: the typed error returned by every `AssetDb` method. Carries the
  failing AssetPath, the cause, and the full chain of referrers active at the time of
  failure.

## Requirements

### Requirement 1: Open an asset by path

**User Story:** As a Phase 2.2-2.8 author, I want to read the raw bytes of an asset by its
install-root-relative logical path, so that I can feed those bytes to my own parser without
duplicating mod-chain resolution.

#### Acceptance Criteria

1. THE AssetDb SHALL expose a method `open(path: &AssetPath) -> Result<Arc<[u8]>, AssetError>`.
2. WHEN `open` is called with an AssetPath that resolves to an existing file via the
   underlying ModChain, THE AssetDb SHALL return an `Arc<[u8]>` containing the entire
   file contents.
3. WHEN `open` is called twice with the same AssetPath and no eviction has occurred between
   the two calls, THE AssetDb SHALL return two `Arc<[u8]>` handles whose dereferenced byte
   slices are equal.
4. IF the AssetPath does not exist in any mod root or vanilla, THEN THE AssetDb SHALL
   return `Err(AssetError::NotFound { .. })`.
5. IF a filesystem I/O error occurs while reading the resolved physical file, THEN THE
   AssetDb SHALL return `Err(AssetError::Io { .. })` with the underlying `std::io::Error`
   kind preserved.
6. THE AssetDb SHALL accept any AssetPath whose string form contains only forward slashes,
   no leading slash, no drive letter, and no `..` component.
7. IF an AssetPath contains a `..` component, a leading `/`, a backslash, or a Windows
   drive prefix, THEN THE AssetDb SHALL return `Err(AssetError::InvalidPath { .. })`
   without touching the filesystem.

### Requirement 2: Mod-chain resolution semantics

**User Story:** As a player loading a mod, I want my mod's files to override vanilla's
files at the same logical path, so that the engine renders my mod's UI and not vanilla's.

#### Acceptance Criteria

1. WHEN multiple mod roots and vanilla each contain a file at the same AssetPath, THE
   AssetDb SHALL select the file from the highest-priority mod root in the ModChain
   (matching `hoi4-paths::PathConfig::find` semantics).
2. WHEN a mod declares `replace_path` covering an AssetPath's prefix and the mod root does
   not itself contain that AssetPath, THE AssetDb SHALL return `Err(AssetError::NotFound)`
   rather than falling back to vanilla.
3. WHEN no mod root contains an AssetPath and no `replace_path` blocks vanilla for that
   prefix, THE AssetDb SHALL return the vanilla file at that AssetPath.
4. THE AssetDb SHALL delegate physical-path resolution to `hoi4-paths::PathConfig` and
   SHALL NOT reimplement mod-chain or `replace_path` logic.

### Requirement 3: Directory enumeration across the mod chain

**User Story:** As the Phase 2.2 `.gfx` loader author, I want to enumerate every file in
`interface/` across the mod chain (because each mod adds its own `*.gfx` files), so that
I can register every sprite definition without losing mod-added entries.

#### Acceptance Criteria

1. THE AssetDb SHALL expose a method `list(dir: &AssetPath) -> Result<Vec<AssetPath>, AssetError>`
   that returns every AssetPath whose parent directory equals `dir` and which is reachable
   through the ModChain or vanilla.
2. WHEN the same logical filename exists in multiple mod roots and vanilla, THE AssetDb
   SHALL include that AssetPath exactly once in the returned `Vec`, deduplicated by logical
   path.
3. THE AssetDb SHALL return AssetPaths sorted lexicographically by their string form, so
   that load order is deterministic across runs and platforms.
4. WHEN a `replace_path` directive covers `dir`, THE AssetDb SHALL exclude vanilla entries
   under `dir` from the returned `Vec`.
5. IF `dir` does not exist in any mod root or vanilla, THEN THE AssetDb SHALL return an
   empty `Vec` (not an error), so that callers can treat "directory absent" identically to
   "directory empty".

### Requirement 4: Typed parse-and-cache

**User Story:** As the Phase 2.3 DDS author, I want to ask the cache for a decoded
`Texture` once and have every subsequent caller in the same session receive the same
`Arc<Texture>` without re-decoding 20 MB of BC1 data, so that opening a screen with 50
icons does not trigger 50 redundant decodes.

#### Acceptance Criteria

1. THE AssetCache SHALL expose a method
   `get_or_load<T>(path: &AssetPath, parse: impl FnOnce(&[u8]) -> Result<(T, ByteCost), E>)
   -> Result<Arc<T>, AssetError>`
   where `T: Any + Send + Sync + 'static` and `E: Into<AssetError>`.
2. WHEN `get_or_load::<T>` is called with an AssetPath that has not been loaded as type `T`
   before, THE AssetCache SHALL invoke the supplied parse closure exactly once and store
   the resulting value under the key `(TypeId::of::<T>(), AssetPath)`.
3. WHEN `get_or_load::<T>` is called with an AssetPath that is already cached as type `T`
   and has not been evicted, THE AssetCache SHALL return the cached `Arc<T>` without
   invoking the parse closure.
4. THE AssetCache SHALL allow the same AssetPath to be cached under more than one type
   simultaneously (e.g. once as `Arc<RawBytes>` and once as `Arc<DecodedDds>`), keyed
   independently by `TypeId`.
5. IF the parse closure returns `Err(e)`, THEN THE AssetCache SHALL return
   `Err(e.into())` and SHALL NOT store any value for that key, so that a transient parse
   failure does not poison the cache.

### Requirement 5: Capacity and eviction

**User Story:** As an engine operator running on a 16 GB machine, I want the asset cache
to stay under a configurable byte budget, so that loading the full 1 GB of DDS files does
not exhaust system memory.

#### Acceptance Criteria

1. THE AssetCache SHALL accept a capacity expressed in bytes at construction time.
2. WHEN inserting a new cache entry whose byte cost would push the total stored bytes
   above the capacity, THE AssetCache SHALL evict least-recently-used entries until
   total stored bytes are at or below capacity, then insert the new entry.
3. WHEN a cache entry is accessed via `get_or_load` and produces a cached hit, THE
   AssetCache SHALL mark that entry as most-recently-used.
4. WHEN an entry is evicted from the cache, THE AssetCache SHALL drop its strong
   reference; outstanding `Arc<T>` clones held by callers SHALL remain valid until those
   callers drop their clones.
5. WHERE a single inserted entry's byte cost exceeds the entire cache capacity, THE
   AssetCache SHALL store the entry and SHALL evict every other entry, so that giant
   assets are still serviceable in a small cache.
6. THE AssetCache SHALL expose a method `current_bytes() -> usize` returning the sum of
   byte costs of currently resident entries, for diagnostic use.

### Requirement 6: Referrer-aware error chain

**User Story:** As a mod author who just got a "missing asset" error, I want the error to
tell me which file referred to the missing asset, so that I can fix the dangling reference
without grepping 142 `.gfx` files.

#### Acceptance Criteria

1. THE AssetDb SHALL expose a scoped guard `enter_referrer(path: &AssetPath) -> ReferrerGuard`
   that pushes `path` onto a per-thread referrer stack on construction and pops it on drop.
2. WHEN any `AssetDb` or `AssetCache` operation returns an `AssetError`, THE AssetError
   SHALL include a `Vec<AssetPath>` field `referrer_chain` containing every AssetPath
   currently on the active thread's referrer stack, ordered from outermost (oldest push)
   to innermost (newest push).
3. WHEN no referrer guard is active and an `AssetError` is produced, THE AssetError's
   `referrer_chain` SHALL be the empty vector.
4. THE `Display` implementation of `AssetError` SHALL render the failing AssetPath, the
   cause, and the referrer chain in a single human-readable message of the form
   `"<asset>: <cause> (referenced by: a -> b -> c)"`.
5. WHEN a `ReferrerGuard` is dropped while panicking, THE AssetDb SHALL still pop the
   corresponding entry from the stack, so that subsequent operations on the same thread
   see a consistent stack.

### Requirement 7: Thread-safety guarantees

**User Story:** As a future developer adding parallel asset preloading, I want to share a
single `AssetDb` instance across worker threads, so that I do not have to design around a
single-threaded cache.

#### Acceptance Criteria

1. THE `AssetDb` trait SHALL be `Send + Sync`, and every concrete implementation in this
   crate SHALL satisfy `Send + Sync`.
2. WHEN two threads concurrently call `open` for the same AssetPath, THE AssetDb SHALL
   return semantically equal `Arc<[u8]>` to both threads without panicking and without
   data races.
3. WHEN two threads concurrently call `get_or_load::<T>` for the same `(TypeId, AssetPath)`
   key on a not-yet-cached entry, THE AssetCache SHALL invoke the parse closure at most
   once across both threads, and both threads SHALL receive the same `Arc<T>`.
4. THE referrer stack defined in Requirement 6 SHALL be per-thread, so that referrer
   chains from concurrent loads on different threads do not interleave.

### Requirement 8: In-memory test fixture

**User Story:** As a CI engineer, I want unit tests for `hoi4-assets` and its downstream
consumers to run without a HOI4 installation, so that the workspace builds green on a
clean GitHub runner.

#### Acceptance Criteria

1. THE `hoi4-assets` crate SHALL provide a `TestAssetDb` type that implements `AssetDb`
   and is constructed from a `HashMap<AssetPath, Vec<u8>>` (or equivalent builder API).
2. THE `TestAssetDb` SHALL satisfy every behavioral requirement in Requirements 1, 3, 4,
   5, 6, and 7 that is independent of the real filesystem (i.e. mod-chain physical-file
   semantics in Requirement 2 are satisfied by the test harness's chosen population, not
   by `hoi4-paths`).
3. THE `TestAssetDb` SHALL NOT depend on `hoi4-paths::PathConfig` and SHALL NOT touch the
   filesystem during any operation.
4. WHEN a property-based test populates a `TestAssetDb` with arbitrary
   `(AssetPath, Vec<u8>)` pairs, THE `TestAssetDb` SHALL behave consistently with the
   correctness properties enumerated below.

## Correctness Properties

These are formal invariants intended to be checked with property-based tests (e.g.
`proptest`) against `TestAssetDb`, since `TestAssetDb` does not require a HOI4 install.
Each is phrased as a universally-quantified statement so that a PBT generator can drive it.

1. **Open is deterministic over content.**
   FOR ALL `(path, bytes)` pairs inserted into a fresh `TestAssetDb`, `db.open(path)`
   SHALL return `Ok(arc)` such that `&*arc == &bytes[..]`.

2. **Open is idempotent within a session.**
   FOR ALL `path` that resolves successfully, two consecutive calls `db.open(path)` and
   `db.open(path)` SHALL return byte-equal results, regardless of intervening cache
   state on unrelated keys.

3. **Cache hit elides the parse closure.**
   FOR ALL `path` and FOR ALL parse closures `f`, after a successful first call to
   `cache.get_or_load::<T>(path, f1)`, a second call `cache.get_or_load::<T>(path, f2)`
   with a closure `f2` that *panics if invoked* SHALL return the cached value without
   panicking, provided no eviction occurred between the two calls.

4. **Round-trip identity through the cache.**
   FOR ALL `(path, bytes)` and FOR ALL identity parse closures `|b| Ok((b.to_vec(), b.len()))`,
   the cached `Arc<Vec<u8>>` SHALL dereference to bytes equal to the original `bytes`.

5. **Eviction never overshoots.**
   FOR ALL sequences of `(path, byte_cost)` insertions into an `AssetCache` with capacity
   `C`, after every insertion the cache invariant `current_bytes() <= max(C, last_cost)`
   SHALL hold (the `max` term covers the "single oversized entry" case in Req 5.5).

6. **Eviction is LRU.**
   FOR ALL access traces over a fixed key universe, when an eviction occurs, the evicted
   key SHALL be the one whose most recent access was furthest in the past among
   currently-resident keys.

7. **Mod-chain priority preservation.**
   FOR ALL pairs of `TestAssetDb` populations `(M_high, M_low)` where `M_high` and `M_low`
   contain overlapping AssetPaths, `db.open(p)` SHALL return the bytes from `M_high`
   for any `p` present in both, mirroring `hoi4-paths::PathConfig::find` priority.

8. **List enumeration is a permutation up to dedup and sort.**
   FOR ALL populations, `db.list(dir)` SHALL return a strictly-sorted, duplicate-free
   `Vec<AssetPath>` whose set equals `{ p | parent(p) == dir AND p reachable through
   ModChain }`.

9. **Referrer chain is well-formed.**
   FOR ALL push/pop sequences performed via `enter_referrer`, the `referrer_chain` field
   on any error produced inside a nested guard scope SHALL equal the stack of guard paths
   from outermost to innermost, exactly matching the ordered push history that has not
   yet been popped.

10. **Concurrent `get_or_load` invokes the parser at most once.**
    FOR ALL key `(TypeId, AssetPath)` and FOR ALL `n` concurrent threads each calling
    `get_or_load` with a closure that increments a shared atomic counter, the final
    counter value SHALL equal `1` and all threads SHALL receive an `Arc<T>` whose
    pointer-equality holds pairwise.

## Non-Goals

The following items are explicitly out of scope for Phase 2.1 and SHALL NOT be addressed
by this spec. Reviewers should not request them; they belong to later phases or to
follow-up specs.

1. **Format-specific parsing.** No `.gfx`, `.gui`, `.fnt`, `.dds`, `.mesh`, or `.asset`
   parser implementation lives in this crate. Those are Phase 2.2-2.8.
2. **GPU upload.** `AssetCache` holds CPU-side parsed values only. wgpu texture creation
   is Phase 2.3's responsibility.
3. **Hot reload.** No filesystem watcher, no inotify/ReadDirectoryChangesW integration,
   no cache invalidation on file mtime change. The cache is populated once per session
   and only changes via eviction. Hot reload is a possible Phase 2.6+ addition.
4. **Memory-mapping.** `open` returns `Arc<[u8]>` from `std::fs::read`, not a `Mmap`. A
   future spec may switch to `memmap2` if profiling shows it matters; the API is chosen
   to not preclude that change.
5. **Compression / packed archives.** Vanilla HOI4 assets ship as loose files, not in
   archive containers. No `.zip` / `.bsa` / `.pak` reader is in scope.
6. **Async I/O.** All `AssetDb` methods are synchronous. A future spec may add an async
   facade; this skeleton does not.
7. **Localization fallback.** Language-specific path resolution (`localisation/english/`
   vs `localisation/simp_chinese/`) is handled by a separate spec; it is not folded into
   `AssetDb::open`.
8. **Cross-process cache sharing.** The cache is per-process, in-memory only; no shared
   memory or on-disk cache file.
9. **Logging side-effects.** This crate exposes errors and diagnostics via return values
   and `Display`; it does not call `tracing` or `log` directly.
10. **Eviction callbacks.** No on-evict hook is exposed; downstream crates must not rely
    on observing eviction events.
