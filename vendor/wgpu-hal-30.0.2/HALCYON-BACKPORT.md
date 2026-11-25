# Halcyon changes for wgpu-hal 30.0.2

Base: the published `wgpu-hal` 30.0.2 crate, SHA-256
`a57452fe453658e141aafdb09de1028afa498805fab518238ddf372a518813ff`.
Its [MIT](LICENSE.MIT) and [Apache-2.0](LICENSE.APACHE) license texts are retained.

## macOS first-frame backport

This is the published wgpu-hal 30.0.2 crate with the two Metal source changes
from [upstream PR 10302](https://github.com/gfx-rs/wgpu/pull/10302), merged as
[`05ae83af5ed55e826b91ea339f4210d586dfa337`](https://github.com/gfx-rs/wgpu/commit/05ae83af5ed55e826b91ea339f4210d586dfa337).

- [`src/metal/mod.rs`](src/metal/mod.rs) adds the macOS surface's
  `has_been_visible` atomic flag.
- [`src/metal/surface.rs`](src/metal/surface.rs) initializes that flag to
  `false` and latches the first actual Visible observation across configure
  and unconfigure. This allows acquisition before the first visibility
  observation. Later occluded acquisitions still refuse. The constructor
  and acquisition import use the release's `core::sync::atomic` API.

The published backend can reject Halcyon's first hidden drawable before the
App can prepare and reveal its first frame. Keep this temporary backport until
a compatible published family includes the fix and passes native first-window
verification.

Halcyon's root Cargo patch applies in this workspace. External applications
must select the same patched source in their own root Cargo manifests to
receive the repair.

## Windows DX12 dependency alignment (2026-11-23)

The published wgpu-hal 30.0.2 manifest accepts `windows` 0.62.x, while
`gpu-allocator` 0.28.0 limits its Windows API dependency to versions
`>=0.53, <=0.62`. The shared compatible version is 0.62.0. Resolving
`gpu-allocator` to 0.57 or 0.58 and wgpu-hal to 0.62.2 produces distinct
`ID3D12Device` types and prevents DX12 compilation.

The vendored [`Cargo.toml`](Cargo.toml) pins its Windows-only `windows`
dependency to exactly 0.62.0. Halcyon's root `Cargo.lock` selects 0.62.0 for both
`gpu-allocator` and `wgpu-hal`; independent crates that need older
Windows API bindings retain them. No DX12 allocation logic or COM-pointer
conversion was changed.

The `x86_64-pc-windows-msvc` cross-target `cargo check --locked --offline
-p halcyon --example showcase` passed on 2026-11-23. This establishes
Windows-target Rust compilation, not Windows executable linking, GUI startup,
or physical GPU rendering.
