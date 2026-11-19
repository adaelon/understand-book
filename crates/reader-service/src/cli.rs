use crate::host::{
    start_server, start_server_with_model_settings, RunningServer, ServerHostConfig,
};
use desktop_support::library_settings::LibrarySettingsStore;
use runtime::{model_settings::ModelSettings, ProviderConfig};
use std::path::PathBuf;

#[cfg(target_os = "linux")]
static STOP_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "linux")]
extern "C" fn request_stop(_: libc::c_int) {
    STOP_REQUESTED.store(true, std::sync::atomic::Ordering::Release);
}

pub fn run() {
    if crate::acp_evidence::serve_stdio_if_requested() {
        return;
    }
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--local-profile")) {
        if let Err(error) = serve_profile(std::env::args_os().skip(2).map(PathBuf::from).collect())
        {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    #[cfg(target_os = "linux")]
    unsafe {
        // The handler only sets an atomic notification; shutdown runs on the main thread.
        for signal in [libc::SIGINT, libc::SIGTERM] {
            if libc::signal(signal, request_stop as *const () as libc::sighandler_t)
                == libc::SIG_ERR
            {
                eprintln!(
                    "failed to install stop signal handler: {}",
                    std::io::Error::last_os_error()
                );
                std::process::exit(1);
            }
        }
    }
    if std::env::args().nth(1).as_deref() == Some("--presentation-preview-probe") {
        #[cfg(target_os = "linux")]
        let result = std::thread::scope(|scope| {
            let cancellation = runtime::run_context::CancellationToken::default();
            let worker_token = cancellation.clone();
            let worker = scope.spawn(move || {
                crate::presentation_preview::run_probe_with_cancellation(&worker_token)
            });
            while !worker.is_finished() {
                if STOP_REQUESTED.load(std::sync::atomic::Ordering::Acquire) {
                    cancellation.cancel();
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            worker.join().unwrap_or(1)
        });
        #[cfg(not(target_os = "linux"))]
        let result = crate::presentation_preview::run_probe();
        std::process::exit(result);
    }
    let (book_dir, reader_only) = parse_args(
        std::env::args().skip(1),
        std::env::var("UNDERSTAND_BOOK_DIR").ok(),
    )
    .unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let mut config = ServerHostConfig::from_env(book_dir);
    config.reader_only = reader_only;
    match start_server(config) {
        Ok(server) => {
            eprintln!("understand-book server listening at {}", server.url);
            #[cfg(target_os = "linux")]
            {
                while !STOP_REQUESTED.load(std::sync::atomic::Ordering::Acquire) {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                server.shutdown();
                eprintln!("understand-book server stopped cleanly");
            }
            #[cfg(not(target_os = "linux"))]
            server.wait();
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

pub fn serve_profile(args: Vec<PathBuf>) -> Result<(), String> {
    if args.len() > 1 {
        return Err("Usage: understand-book serve [book-directory]".into());
    }
    let data_dir =
        crate::local_profile::resolve_data_root(&crate::local_profile::installed_data_home()?)?;
    let book = args
        .into_iter()
        .next()
        .map(std::fs::canonicalize)
        .transpose()
        .map_err(|e| e.to_string())?;
    let settings = LibrarySettingsStore::new(data_dir.join("settings.json"));
    let root = settings.initial_root(&data_dir.join("library"))?;
    let models = match settings.model_settings()? {
        Some(value) => {
            Some(serde_json::from_value::<ModelSettings>(value).map_err(|e| e.to_string())?)
        }
        None => settings
            .provider_settings()?
            .map(|p| {
                ProviderConfig::from_values(&p.mode, p.api_key, p.base_url, p.model)
                    .map(|config| ModelSettings::from_legacy(&config))
                    .map_err(|e| e.message)
            })
            .transpose()?,
    };
    std::env::set_var("UNDERSTAND_BOOK_MEMORY_DIR", data_dir.join("memory"));
    std::env::set_var("UNDERSTAND_BOOK_PRIVATE_DIR", data_dir.join("private"));
    let _ = dotenvy::from_path(data_dir.join(".env"));
    let packaged_resources = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent()?.parent().map(|p| p.to_owned()))
        .filter(|resources| resources.join("web-dist/index.html").is_file());
    if let Some(resources) = &packaged_resources {
        let runtime = resources.join("reader-runtime");
        std::env::set_var(
            "UNDERSTAND_BOOK_NODE",
            runtime.join(if cfg!(windows) { "node.exe" } else { "node" }),
        );
        std::env::set_var(
            "UNDERSTAND_BOOK_STAGE_RUNNER",
            runtime.join("workbench-stage.mjs"),
        );
    }
    let web = std::env::var_os("UNDERSTAND_BOOK_WEB_DIST")
        .map(PathBuf::from)
        .or_else(|| packaged_resources.map(|p| p.join("web-dist")))
        .unwrap_or_else(|| crate::workspace_root().join("packages/web/dist"));
    let mut config = ServerHostConfig::desktop(root.clone(), web);
    if let Ok(value) = std::env::var("UNDERSTAND_BOOK_ADDR") {
        let addr: std::net::SocketAddr = value.parse().map_err(|_| {
            "UNDERSTAND_BOOK_ADDR must be a loopback IP address and port for a local profile"
        })?;
        if !addr.ip().is_loopback() {
            return Err("The local profile must listen on a loopback address".into());
        }
        config.addr = addr.to_string();
    }
    config.book_dir = book;
    let server: RunningServer = start_server_with_model_settings(config, models)?;
    if settings.load()?.is_none() {
        settings.save_root(&root)?;
    }
    server.configure_model_settings(
        None,
        "settings",
        std::sync::Arc::new(move |models| {
            let root = settings.load()?.ok_or("Local library is unavailable")?;
            settings.apply_model_settings(
                &root,
                serde_json::to_value(models).map_err(|e| e.to_string())?,
            )
        }),
    )?;
    eprintln!("Local Reader ready at {}", server.url);
    server.wait();
    Ok(())
}

fn parse_args(
    args: impl IntoIterator<Item = String>,
    env_book: Option<String>,
) -> Result<(String, bool), String> {
    let mut book = None;
    let mut reader_only = false;
    let mut positional = false;
    for arg in args {
        if !positional && arg == "--" {
            positional = true;
            continue;
        }
        if !positional && arg == "--reader-only" {
            reader_only = true;
            continue;
        }
        if !positional && arg.starts_with('-') {
            return Err(format!("unknown option: {arg}"));
        }
        if book.replace(arg).is_some() {
            return Err("expected one book directory".into());
        }
    }
    book.or(env_book)
        .filter(|value| !value.is_empty())
        .map(|book| (book, reader_only))
        .ok_or_else(|| {
            "usage: reader [--reader-only] <book_dir> (or set UNDERSTAND_BOOK_DIR)".into()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_and_reader_cli_preserve_book_selection() {
        for (args, env, book, mode) in [
            (vec!["C:\\books\\demo"], None, "C:\\books\\demo", false),
            (vec![], Some("env-book"), "env-book", false),
            (
                vec!["--reader-only", "/books/中文 空格"],
                None,
                "/books/中文 空格",
                true,
            ),
            (vec!["--reader-only"], Some("env-book"), "env-book", true),
            (vec!["book", "--reader-only"], Some("ignored"), "book", true),
        ] {
            assert_eq!(
                parse_args(
                    args.into_iter().map(str::to_string),
                    env.map(str::to_string)
                )
                .unwrap(),
                (book.into(), mode)
            );
        }
        assert!(parse_args(vec![], None).is_err());
        assert!(parse_args(vec!["--typo".into()], Some("book".into())).is_err());
        assert!(parse_args(vec!["one".into(), "two".into()], None).is_err());
    }
}
