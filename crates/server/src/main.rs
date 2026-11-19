#[cfg(target_os = "linux")]
static STOP_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "linux")]
extern "C" fn request_stop(_: libc::c_int) {
    STOP_REQUESTED.store(true, std::sync::atomic::Ordering::Release);
}

fn main() {
    if std::env::args().nth(1).as_deref() != Some("--multi-user") {
        reader_service::cli::run();
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
    let result = parse_multi_args(std::env::args().skip(2).collect())
        .and_then(server::host::start_multi_user_server);
    match result {
        Ok(server) => {
            eprintln!(
                "understand-book multi-user backend listening at {}",
                server.url
            );
            #[cfg(target_os = "linux")]
            {
                while !STOP_REQUESTED.load(std::sync::atomic::Ordering::Acquire) {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                server.shutdown();
            }
            #[cfg(not(target_os = "linux"))]
            server.wait();
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

fn parse_multi_args(args: Vec<String>) -> Result<server::host::MultiUserConfig, String> {
    if !(args.len() == 3 || args.len() == 5)
        || args[1] != "--origin"
        || (args.len() == 5 && args[3] != "--addr")
    {
        return Err("usage: server --multi-user <absolute-service-root> --origin https://reader.example [--addr 127.0.0.1:8787]".into());
    }
    Ok(server::host::MultiUserConfig {
        root: args[0].clone().into(),
        origin: args[2].clone(),
        addr: args
            .get(4)
            .map(String::as_str)
            .unwrap_or("127.0.0.1:8787")
            .parse()
            .map_err(|_| "Invalid backend address".to_string())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multi_user_cli_requires_explicit_root_origin_and_separate_mode() {
        let config = parse_multi_args(vec![
            "/srv/reader".into(),
            "--origin".into(),
            "https://reader.example".into(),
        ])
        .unwrap();
        assert_eq!(config.origin, "https://reader.example");
        assert!(config.addr.ip().is_loopback());
        for args in [
            vec![],
            vec!["/srv/reader"],
            vec!["/srv/reader", "https://reader.example"],
            vec!["/srv/reader", "--reader-only", "https://reader.example"],
        ] {
            assert!(parse_multi_args(args.into_iter().map(str::to_string).collect()).is_err());
        }
    }
}
