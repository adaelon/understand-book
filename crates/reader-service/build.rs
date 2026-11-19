fn main() {
    // Reopening persisted chat turns uses the same Reader startup path as the
    // desktop host, including deserialization of the complete teaching history.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        for binary in ["reader", "understand-book"] {
            println!("cargo:rustc-link-arg-bin={binary}=/STACK:16777216");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
