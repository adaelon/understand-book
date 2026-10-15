# Konva 10.7.0

Product-bundled browser build, MIT license in `LICENSE`.
Source: https://unpkg.com/konva@10.7.0/konva.min.js
Copied byte-for-byte from the frozen EX10 vendor resource on 2026-09-28.

`crates/server/src/presentation_libraries.rs` embeds these files in the server at build time. Each author candidate selecting `libraries:["konva"]` saves its own copy and logical script reference; preview and Reader use those version files. A later bundled version does not alter saved pages.
