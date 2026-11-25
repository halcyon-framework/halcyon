#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.as_slice() {
        [] => halcyon::platform::relative_pointer_macos_conformance::run(),
        #[cfg(debug_assertions)]
        [mode] if mode == "--engine" => {
            halcyon::platform::relative_pointer_macos_conformance::run_engine()
        }
        [mode, path] if mode == "--analyze" => {
            halcyon::platform::relative_pointer_macos_conformance::reanalyze(std::path::Path::new(
                path,
            ))
        }
        _ => Err(
            "use the default native capture, debug-only --engine, or --analyze <jsonl-path>".into(),
        ),
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("SPEC-HAL-362 conformance harness requires macOS");
    std::process::exit(1);
}
