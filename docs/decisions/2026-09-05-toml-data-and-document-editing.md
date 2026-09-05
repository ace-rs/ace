# Use `toml` for configuration data and `toml_edit` for document edits

- **Date:** 2026-09-05
- **PR:** manual
- **Status:** accepted

## Decision

Retain `toml` for typed configuration reads and serialization of generated TOML.
Use `toml_edit` for targeted changes to existing user-authored TOML files, retaining
their document structure and unrelated content. Serde remains the mapping between
Rust configuration structs and serialized data.

These are two responsibilities within TOML handling: interpreting configuration and
editing a person's document. Crate selection follows that responsibility; it is not
an arbitrary choice at each call site. The normative boundary is in
[architecture](../spec/architecture.md#toml-data-and-document-editing).

Acceptance selects this division of responsibilities; it does not certify that the
preserving write paths are implemented. At this decision's date, ACE's config save
function still regenerates files through `toml::to_string_pretty`.

## Why the original `toml` choice was reasonable

Commit `d1d906f` introduced `toml = "0.8"` alongside Serde on 2026-02-17 under
"Add config structs for ace.toml, school.toml, and user config". The commit records no
comparison of TOML libraries. Choosing it for straightforward typed configuration is
a reasonable inference, not a recorded author rationale.

`toml` is an established Serde-compatible TOML library, also used by Cargo. It fits
reading configuration into structs and generating files from data. That does not
make a struct serialization round trip appropriate for editing a user-authored file:
comments, whitespace, and original spelling are absent from ordinary Rust structs.

For example, changing one backend setting should retain the user's explanatory
comments and unrelated keys. Regenerating the entire file from a typed configuration
can discard both comments and fields that the struct does not represent.

## Why two related crates exist

Both crates belong to the [same upstream project][upstream]; "same project" is more
accurate than claiming a single author. The upstream describes `toml` as its Serde
interface and `toml_edit` as its format-preserving editing interface.

`toml_edit` is standalone: its name does not mean it requires the `toml` crate.
With its optional `serde` feature, it can deserialize Rust structs and serialize
them into TOML. Using both is a deliberate division, not a dependency requirement.

Cargo provides concrete precedent for that division: its manifest schema uses
`toml::Value`, while its editable manifest stores `toml_edit::DocumentMut` and writes
the edited document back. These call sites establish usage, not the Cargo maintainers'
historical reasons for retaining both crates; no such rationale is established here.

Maintenance evidence as of 2026-09-05 supports using `toml_edit`: its upstream
[changelog][changelog] records correctness fixes in April, May, and July 2026,
performance improvements in March, and release 0.25.13 on July 14. Cargo's dependency
and this release history support its credibility, without guaranteeing future support.

## Why not use only `toml_edit`?

That is technically possible, including keeping ordinary Serde-derived Rust structs.
It would reduce the number of direct TOML dependencies. The reason to retain `toml`
is its simpler data representation and established fit for typed configuration.

Replacing `toml` data types with document types has specific costs:

- `toml::Value` represents tables uniformly; `toml_edit` distinguishes tables,
  inline tables, and arrays of tables through its `Item` and `Value` types.
- Edited scalars carry formatting metadata through `Formatted<T>`. Retaining and
  cloning that information adds work when only the value matters; ACE has no measured
  performance comparison establishing its practical magnitude.
- `toml::Value` implements `Serialize`, `Deserialize`, and `PartialEq`.
  `toml_edit::Value` does not implement those traits in the evaluated API; its crate's
  separate Serde conversion functions do not make it a drop-in struct-field replacement.

Keeping ACE's own typed structs avoids spreading document types through configuration
logic under either choice. Consolidating on `toml_edit` remains a possible future
decision, but fewer crate names alone do not justify replacing the data interface.

## Why not a `toml` feature or Serde setting?

`toml`'s `preserve_order` feature preserves map order, not comments or formatting.
Serde's ordinary data model represents values and structures, not source comments or
whitespace; no Serde setting restores information absent from those structures.

Even with `toml_edit`'s Serde feature, deserializing a document into a struct and
serializing that struct into a fresh document does not preserve the original text.
Preserving edits must retain the original document and change the intended nodes.
Enabling Serde support in `toml_edit` is therefore optional for this split, not the
mechanism that protects user-authored content.

## Other options and consequences

[Taplo][taplo] offers syntax-tree analysis and formatting with layout preservation.
Its editor-oriented surface would require more integration for ACE's targeted config
edits; no requirement here justifies that broader toolset or a custom TOML editor.

The accepted split adds a direct dependency and two public API surfaces. Version,
feature, and transitive dependency selection determine the final dependency cost;
no build-time or runtime performance improvement is claimed.

`toml_edit` does not promise byte-for-byte preservation under arbitrary mutations:
its documentation identifies dotted-key ordering as a limitation, and replacing a
node can require care with its formatting metadata. Preservation must be verified for
ACE's actual edits, including comments and unrelated recognized and unknown fields.

## Sources

API and maintenance evidence checked on 2026-09-05:

- [Shared upstream project][upstream].
- [TOML feature flags](https://docs.rs/crate/toml/latest/features).
- [Serde data model](https://serde.rs/data-model.html).
- [toml_edit API and preservation limitations](https://docs.rs/toml_edit/latest/).
- [toml_edit Serde deserialization](https://docs.rs/toml_edit/latest/toml_edit/de/).
- [toml_edit Serde serialization](https://docs.rs/toml_edit/latest/toml_edit/ser/).
- [toml Value traits](https://docs.rs/toml/latest/toml/enum.Value.html).
- [toml_edit Value traits](https://docs.rs/toml_edit/latest/toml_edit/enum.Value.html).
- [Cargo manifest schema][cargo-schema] and [document editing][cargo-edit].
- [toml_edit release history][changelog].
- [Taplo API][taplo].

[upstream]: https://github.com/toml-rs/toml
[changelog]: https://github.com/toml-rs/toml/blob/main/crates/toml_edit/CHANGELOG.md
[cargo-schema]:
  https://github.com/rust-lang/cargo/tree/master/crates/cargo-util-schemas/src/manifest
[cargo-edit]:
  https://github.com/rust-lang/cargo/blob/master/src/cargo/util/toml_mut/manifest.rs
[taplo]: https://docs.rs/taplo/latest/taplo/
