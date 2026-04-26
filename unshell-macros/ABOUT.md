# UnShell Macros

This crate owns the compile-time declaration layer for UnShell application-facing
leaves.

## Purpose

The protocol crate intentionally stays generic: it knows how to route packets,
validate framing, and deliver local events, but it should not need handwritten
registration code for every leaf.

The macro layer exists to move as much of that registration work as possible to
compile time.

In practical terms, the macro system is responsible for:

- deriving canonical leaf identities
- deriving canonical procedure identifiers
- generating compile-time procedure inventories for leaves
- binding one leaf declaration to separate endpoint and TUI host structs without
  repeating the metadata on each host
- generating dispatch glue for simple call-driven leaves

## Model

There are three layers in the intended design.

### 1. Leaf declaration

One declaration is the source of truth for one protocol leaf.

The declaration answers:

- what is this leaf called on the wire?
- which procedure suffixes belong to it?
- which host structs implement its endpoint and TUI roles?

The goal is that this information is written once and reused everywhere.

### 2. Host structs

One leaf can have multiple host structs with different responsibilities.

- the endpoint host owns runtime state and protocol-side behavior
- the TUI host owns user-interface state and interpretation behavior

Those hosts should not each have to repeat the leaf name or procedure inventory.
They bind to the declaration instead.

### 3. Procedure and method metadata

Procedures and future typed remote methods need stable canonical identifiers.

The macro layer generates those identifiers from the leaf declaration and the
local suffix for each procedure or method. That lets the runtime consume a
compile-time inventory instead of handwritten lists.

## Current direction

The current migration keeps the older derive-based APIs working while adding a
new declaration-first API.

That migration is intentionally incremental:

1. keep `#[derive(Leaf)]`, `#[derive(Procedure)]`, and `#[procedures]` working
2. introduce one declaration macro for compile-time leaf metadata
3. let endpoint and TUI structs bind to the declaration instead of duplicating
   metadata
4. remove leaf-owned endpoint-construction boilerplate by generating leaf specs
   from the declaration
5. add typed remote-method metadata on top of the same declaration model

## Design constraints

The system is optimized for a few constraints that matter to this repository.

- compile-time declaration should replace handwritten runtime registration where
  possible
- protocol-visible names should remain deterministic and canonical
- generated code should stay explicit enough to debug
- endpoint and TUI roles should share metadata but not be forced into the same
  runtime trait when their behavior differs
- migration should be low-breakage for the existing examples and tests

## Non-goals

This crate does not own transport, connection management, or packet execution.
Those remain in `unshell-protocol` and higher application layers.

The macro crate should generate metadata and glue, not hide the runtime model.
