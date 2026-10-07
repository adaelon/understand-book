//! Offline operator credentials. stdin, never argv, carries a password.
use server::control_store::{ControlStore, ServiceWriter};
use std::{io::Read, path::Path};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = (|| -> Result<(), String> {
        if args.len() != 3 {
            return Err("usage: manage_reader <absolute-service-root> create|password|disable|enable|revoke <user-id>; create/password read password from stdin".into());
        }
        let writer = ServiceWriter::acquire(Path::new(&args[0])).map_err(|e| e.error_code)?;
        let mut control = ControlStore::open(writer).map_err(|e| e.error_code)?;
        let result = match args[1].as_str() {
            "create" | "password" => {
                let mut password = String::new();
                std::io::stdin()
                    .take(1027)
                    .read_to_string(&mut password)
                    .map_err(|_| "Could not read password")?;
                let password = password
                    .strip_suffix("\r\n")
                    .or_else(|| password.strip_suffix('\n'))
                    .unwrap_or(&password);
                control.provision_password(&args[2], password, args[1] == "create")
            }
            "disable" => control.set_user_disabled(&args[2], true),
            "enable" => control.set_user_disabled(&args[2], false),
            "revoke" => control.revoke_user_sessions(&args[2]),
            _ => return Err("Unknown account operation".into()),
        };
        result.map_err(|e| e.error_code)
    })();
    match result {
        Ok(()) => println!("Account operation committed"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
