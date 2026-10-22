//! Hidden WebView2 integration probe. No network request or real account is used.
#[allow(dead_code)]
#[path = "../src/cloud_auth.rs"]
mod cloud_auth;
use tauri::{WebviewUrl, WebviewWindowBuilder};
fn main() {
    let mode = std::env::args().nth(1).expect("write, read, or clear");
    let profile =
        std::path::PathBuf::from(std::env::args().nth(2).expect("dedicated probe profile"));
    assert!(profile.is_absolute());
    std::fs::create_dir_all(&profile).unwrap();
    let app = tauri::Builder::default().setup(move |app| {
        let window = WebviewWindowBuilder::new(app, "probe", WebviewUrl::External("about:blank".parse().unwrap()))
            .visible(false).data_directory(profile).build()?;
        let handle = app.handle().clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let origin: url::Url = "https://desktop-session-probe.invalid".parse().unwrap();
                let cookie = cloud_auth::session_cookie(&origin, "fixture-only-independent-device-session".into(), 3600);
                if mode == "write" { window.set_cookie(cookie.clone()).map_err(|e| e.to_string())?; }
                if mode == "clear" { window.delete_cookie(cookie).map_err(|e| e.to_string())?; }
                let cookies = window.cookies_for_url(origin).map_err(|e| e.to_string())?;
                let found = cookies.iter().find(|cookie| cookie.name() == "__Host-ub_session");
                if mode == "clear" {
                    if found.is_some() { return Err("device cookie still exists after logout clear".into()); }
                } else {
                    let found = found.ok_or("device cookie was not written or did not survive restart")?;
                    if found.value() != "fixture-only-independent-device-session"
                        || found.secure() != Some(true) || found.http_only() != Some(true) || found.expires().is_none() {
                        return Err("device cookie lost its value, security attributes, or persistent expiry".into());
                    }
                }
                Ok(())
            })();
            match result {
                Ok(()) => { println!("WebView2 {mode}: passed"); handle.exit(0); }
                Err(error) => { eprintln!("WebView2 {mode}: {error}"); handle.exit(1); }
            }
        });
        Ok(())
    }).build(tauri::generate_context!()).unwrap();
    app.run(|_, _| {});
}
