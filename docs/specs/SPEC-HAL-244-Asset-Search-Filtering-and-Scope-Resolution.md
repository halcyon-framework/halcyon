# SPEC-HAL-244: Asset Search, Filtering, and Scope Resolution

**Status:** Draft
**Layer:** ViewModel Query Layer / Search & Scope Binding
**Scope:** Asset search input, filtering UI, tag-based and scoped search semantics
**Owner:** Application Asset Infrastructure Layer
**Applies To:** Asset Browser, Picker Controls, Search Panels

---

## 0. Purpose

Declares the standard interface for searching and filtering assets within view contexts.
Includes textual search, tag filters, type filters, and scope resolution (e.g., folder, project).

---

## 1. Invariants

### INV-244.1: Search Query Is Immutable Input

The current query is a raw user input string; it is not mutated or pre-processed by the ViewModel.

- Binding only
- Displayed verbatim in input field
- All normalization is done by the resolver/backend

### INV-244.2: Filter Criteria Are Structured

Filters (e.g., asset type, tags, status) are structured query components with explicit binding.

- Represented as a list of typed filter tokens
- Mutated via UI or programmatic intents

### INV-244.3: Scope Is Resolved Separately

Search scope (e.g., "current folder", "entire project") is a separately bound state in the ViewModel.

- Can be changed independently
- Affects the query resolution backend
- Not part of the query string

### INV-244.4: Result Authority Is Remote

The ViewModel does not compute results. All search/filtering is delegated to a backend or resolver.

- ViewModel binds to results
- Can reflect loading, empty, or error states

---

## 2. Query Components

```rust
struct SearchQuery {
    text: String,
    filters: Vec<FilterToken>,
    scope: SearchScope,
}

enum SearchScope {
    CurrentFolder,
    ProjectWide,
    WorkspaceGlobal,
    Custom(String),
}
```

---

## 3. UI Responsibilities

- Bind search input to ViewModel query state
- Display active filters, allow editing/removal
- Provide scope selector UI
- Reflect search results or errors

---

## 4. Out of Scope

- Semantic matching (handled by backend)
- Tag taxonomy management
- Saved search presets
