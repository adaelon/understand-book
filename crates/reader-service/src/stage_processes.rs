//! Format readers belong to the local host that starts them.
use std::{
    io,
    path::PathBuf,
    process::Child,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Default)]
pub struct LocalStageProcesses {
    children: Mutex<Vec<StageProcess>>,
}

struct StageProcess {
    child: Child,
    book_dir: PathBuf,
    job_id: String,
    run_id: String,
}

impl LocalStageProcesses {
    pub(crate) fn track(&self, child: Child, book_dir: PathBuf, job_id: String, run_id: String) {
        let mut children = self
            .children
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // Reap completed stages while the host stays open; only running processes
        // need to be retained for shutdown.
        children.retain_mut(|process| !matches!(process.child.try_wait(), Ok(Some(_))));
        children.push(StageProcess {
            child,
            book_dir,
            job_id,
            run_id,
        });
    }

    pub(crate) fn stop_all(&self) {
        let mut children = self
            .children
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for mut process in children.drain(..) {
            if let Err(error) = process.stop() {
                eprintln!(
                    "Could not stop local format reader {}: {error}",
                    process.child.id()
                );
            }
        }
    }
}

impl Drop for LocalStageProcesses {
    fn drop(&mut self) {
        self.stop_all();
    }
}

impl StageProcess {
    fn stop(&mut self) -> Result<(), String> {
        if self
            .child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_none()
        {
            stop_process_tree(&mut self.child).map_err(|error| error.to_string())?;
        }
        self.child.wait().map_err(|error| error.to_string())?;
        let mut job = crate::read_build_job_by_id(&self.book_dir, &self.job_id)
            .map_err(|error| error.message)?;
        if job.get("status").and_then(|value| value.as_str()) != Some("running")
            || job
                .pointer("/active_run/run_id")
                .and_then(|value| value.as_str())
                != Some(&self.run_id)
        {
            return Ok(());
        }
        let stage = job
            .pointer("/active_run/stage")
            .and_then(|value| value.as_str())
            .unwrap_or("source_reconciliation")
            .to_owned();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();
        let message = "Local host closed before the format reader completed";
        job["status"] = serde_json::json!("interrupted");
        job["failure_summary"] = serde_json::json!({
            "stage": stage, "message": message, "failed_at": now, "recoverable": true,
        });
        job = crate::append_job_event(
            job,
            &now,
            "run_interrupted",
            Some(&stage),
            Some(message),
            None,
        );
        crate::write_build_job_atomic(&self.book_dir, &job).map_err(|error| error.message)
    }
}

pub(crate) fn stop_process_tree(child: &mut Child) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .creation_flags(0x08000000)
            .output();
    }
    #[cfg(unix)]
    unsafe {
        // Format readers start their own process group, including extraction helpers.
        if libc::kill(-(child.id() as i32), libc::SIGKILL) != 0
            && io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
        {
            return Err(io::Error::last_os_error());
        }
    }
    if child.try_wait()?.is_none() {
        child.kill()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    #[test]
    #[ignore = "subprocess fixture for shutdown_stops_format_reader_and_preserves_job_recovery"]
    fn format_reader_child() {
        let Some(ready) = std::env::var_os("UNDERSTAND_BOOK_STAGE_TEST_READY") else {
            return;
        };
        std::fs::write(ready, b"ready").unwrap();
        thread::sleep(Duration::from_secs(30));
    }

    #[test]
    fn shutdown_stops_format_reader_and_preserves_job_recovery() {
        for (status, active_run, expected) in [
            ("running", "run-1", "interrupted"),
            ("completed", "run-1", "completed"),
            ("running", "newer-run", "running"),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let ready = directory.path().join("ready");
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--ignored",
                    "--exact",
                    "stage_processes::tests::format_reader_child",
                ])
                .env("UNDERSTAND_BOOK_STAGE_TEST_READY", &ready)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                command.process_group(0);
            }
            let child = command.spawn().unwrap();
            let processes = LocalStageProcesses::default();
            processes.track(
                child,
                directory.path().to_path_buf(),
                "job-1".into(),
                "run-1".into(),
            );
            let until = Instant::now() + Duration::from_secs(5);
            while !ready.exists() && Instant::now() < until {
                thread::sleep(Duration::from_millis(10));
            }
            assert!(ready.exists(), "format reader child did not start");
            crate::write_build_job_atomic(
                directory.path(),
                &serde_json::json!({
                    "job_id": "job-1", "status": status,
                    "active_run": { "run_id": active_run, "stage": "source_reconciliation" },
                    "events": [],
                }),
            )
            .unwrap();
            {
                let mut children = processes.children.lock().unwrap();
                children[0].stop().unwrap();
                assert!(
                    children[0].child.try_wait().unwrap().is_some(),
                    "owned child must be reaped before shutdown returns"
                );
            }
            let job = crate::read_build_job_by_id(directory.path(), "job-1").unwrap();
            assert_eq!(job["status"], expected);
            if expected == "interrupted" {
                assert_eq!(job["failure_summary"]["recoverable"], true);
                assert_eq!(job["events"][0]["type"], "run_interrupted");
            } else {
                assert!(job["events"].as_array().unwrap().is_empty());
            }
            processes.stop_all();
            assert!(processes.children.lock().unwrap().is_empty());
        }
    }
}
