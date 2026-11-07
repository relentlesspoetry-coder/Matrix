//! Preserve the installer's diagnosis even when it closes its request early.
//! This module never changes validation, terminates an installer, or retries it.
use matrix_update_core as core;
use std::{
    io::{self, Read},
    os::{fd::OwnedFd, unix::net::UnixStream},
    process::{Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

// Outcome recovery accepts at most 16 KiB of JSON. Bound both raw and decoded
// text to 4 KiB, leaving room for worst-case JSON escaping and the error/remedy.
const DIAGNOSTIC_LIMIT: usize = 4096;
const READ_WAIT: Duration = Duration::from_millis(50);
const FINAL_DRAIN: Duration = Duration::from_millis(100);
const MANUAL_REMEDY: &str = "Close all Matrix Play windows, then install the matching official DEB manually. If installation began, check your system package manager before retrying.";

pub(super) fn run(mut command: Command, download: &core::Downloaded) -> Result<(), String> {
    // A socket timeout lets capture finish even if a descendant inherits stderr.
    // Create/configure the capture before starting any installer. Read it while
    // writing stdin, otherwise a verbose rejection could fill stderr and deadlock.
    let (reader, writer) = UnixStream::pair()
        .map_err(|e| format!("Cannot prepare system installer diagnostics: {e}"))?;
    reader
        .set_read_timeout(Some(READ_WAIT))
        .map_err(|e| format!("Cannot prepare system installer diagnostics: {e}"))?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::from(OwnedFd::from(writer)));
    let finished = Arc::new(AtomicBool::new(false));
    let reader_finished = finished.clone();
    let capture = std::thread::Builder::new()
        .name("matrix-installer-diagnostics".into())
        .spawn(move || collect_diagnostics(reader, &reader_finished))
        .map_err(|e| format!("Cannot capture system installer diagnostics: {e}"))?;
    let spawned = command.spawn();
    // Command may retain the configured stderr descriptor for another spawn.
    drop(command);
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            finished.store(true, Ordering::Release);
            let _ = capture.join();
            return Err(format!(
                "Could not open the secure system installer: {error}. {MANUAL_REMEDY}"
            ));
        }
    };
    let sent = match child.stdin.take() {
        Some(mut input) => core::deb::write_request(download, &mut input),
        None => Err("Installer input unavailable".into()),
    };
    // Closing stdin is essential even after a failed/truncated request. Always
    // reap the actual helper before releasing the lifecycle gate. Never kill it.
    let status = child.wait();
    finished.store(true, Ordering::Release);
    let diagnostics = capture
        .join()
        .unwrap_or_else(|_| "Installer diagnostics could not be read.".into());
    installation_result(sent, status, &diagnostics)
}

fn collect_diagnostics(mut reader: UnixStream, finished: &AtomicBool) -> String {
    let mut tail = Vec::with_capacity(DIAGNOSTIC_LIMIT);
    let mut truncated = false;
    let mut drain_until = None;
    let mut read_error = None;
    let mut chunk = [0; 4096];
    loop {
        if finished.load(Ordering::Acquire) {
            let deadline = drain_until.get_or_insert_with(|| Instant::now() + FINAL_DRAIN);
            if Instant::now() >= *deadline {
                break;
            }
        }
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => {
                let remove = (tail.len() + count).saturating_sub(DIAGNOSTIC_LIMIT);
                if remove != 0 {
                    tail.drain(..remove);
                    truncated = true;
                }
                tail.extend_from_slice(&chunk[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                if finished.load(Ordering::Acquire) {
                    break;
                }
            }
            Err(error) => {
                read_error = Some(error);
                break;
            }
        }
    }
    let mut text: String = String::from_utf8_lossy(&tail)
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();
    // Lossy decoding can expand every invalid byte to a three-byte replacement.
    if text.len() > DIAGNOSTIC_LIMIT {
        let mut start = text.len() - DIAGNOSTIC_LIMIT;
        while !text.is_char_boundary(start) {
            start += 1;
        }
        text.drain(..start);
        truncated = true;
    }
    let mut result = String::new();
    if truncated {
        result.push_str("[Earlier installer output omitted]\n");
    }
    result.push_str(text.trim());
    if let Some(error) = read_error {
        result.push_str(&format!("\n[Installer diagnostics interrupted: {error}]"));
    }
    result
}

fn installation_result(
    sent: Result<(), String>,
    status: io::Result<ExitStatus>,
    diagnostics: &str,
) -> Result<(), String> {
    if sent.is_ok() && status.as_ref().is_ok_and(ExitStatus::success) {
        return Ok(());
    }
    let mut message = match status {
        // pkexec reserves 126 for a dismissed authentication dialog. 127 can
        // mean denied authorization, missing agent, or failure to run the helper.
        Ok(status) if status.code() == Some(126) => {
            "System update authorization was dismissed (installer exit status 126).".into()
        }
        Ok(status) if status.code() == Some(127) => {
            "System authorization or installer startup failed (installer exit status 127).".into()
        }
        Ok(status) => {
            format!("The secure system installer did not complete the update ({status}).")
        }
        Err(error) => format!("Could not confirm the system installer's result: {error}."),
    };
    if let Err(error) = sent {
        message.push_str(&format!(
            " The installer request was not fully delivered: {error}."
        ));
    }
    if !diagnostics.trim().is_empty() {
        message.push_str("\nInstaller details:\n");
        message.push_str(diagnostics);
    }
    message.push('\n');
    message.push_str(MANUAL_REMEDY);
    Err(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, os::unix::process::ExitStatusExt};

    // These are unsigned synthetic bytes for transport-only fixtures. No test
    // invokes pkexec, APT, the privileged helper, or production verification.
    fn fixture_download(root: &std::path::Path) -> core::Downloaded {
        let path = root.join("not-an-installable-package.deb");
        std::fs::write(&path, vec![b'x'; 2 * 1024 * 1024]).unwrap();
        core::Downloaded {
            release: core::VerifiedRelease {
                manifest: core::Manifest {
                    schema: 1,
                    repository: core::REPOSITORY.into(),
                    version: "99.0.0".into(),
                    tag: "v99.0.0".into(),
                    notes: String::new(),
                    assets: Default::default(),
                },
                asset: core::Asset {
                    name: "not-an-installable-package.deb".into(),
                    size: 2 * 1024 * 1024,
                    sha256: String::new(),
                },
                kind: "deb".into(),
                signed_manifest: b"transport-only-fixture".to_vec(),
                signature: vec![0; 64],
            },
            path,
        }
    }

    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    #[test]
    fn old_write_request_exposes_only_broken_pipe_after_reader_exits() {
        let root = tempfile::tempdir().unwrap();
        let download = fixture_download(root.path());
        let mut child = Command::new("/bin/true")
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        child.wait().unwrap();
        assert_eq!(
            core::deb::write_request(&download, &mut input).unwrap_err(),
            "Broken pipe (os error 32)"
        );
    }

    #[test]
    fn early_exit_retains_authorization_and_rejection_details() {
        let root = tempfile::tempdir().unwrap();
        let download = fixture_download(root.path());
        for (code, marker, explanation) in [
            (
                126,
                "fixture dialog dismissed",
                "authorization was dismissed",
            ),
            (127, "fixture authorization denied", "startup failed"),
            (1, "fixture signed metadata rejected", "did not complete"),
        ] {
            let error = run(
                shell(&format!("exec 0<&-; printf '{marker}\\n' >&2; exit {code}")),
                &download,
            )
            .unwrap_err();
            assert!(error.contains(marker), "{error}");
            assert!(error.contains(explanation), "{error}");
            assert!(error.contains("not fully delivered"), "{error}");
            assert!(error.contains("Broken pipe"), "{error}");
            assert!(error.contains("official DEB manually"), "{error}");
        }
    }

    #[test]
    fn incomplete_transfer_cannot_report_success_even_with_zero_exit() {
        let root = tempfile::tempdir().unwrap();
        let error = run(
            shell("exec 0<&-; printf 'fixture premature success\\n' >&2; exit 0"),
            &fixture_download(root.path()),
        )
        .unwrap_err();
        assert!(error.contains("exit status: 0"), "{error}");
        assert!(error.contains("not fully delivered"), "{error}");
        assert!(error.contains("fixture premature success"), "{error}");
    }

    #[test]
    fn full_transfer_requires_successful_helper_exit() {
        let root = tempfile::tempdir().unwrap();
        let download = fixture_download(root.path());
        run(shell("cat >/dev/null; exit 0"), &download).unwrap();
        let error = run(shell("cat >/dev/null; exit 1"), &download).unwrap_err();
        assert!(error.contains("exit status: 1"), "{error}");
        assert!(!error.contains("not fully delivered"), "{error}");
    }

    #[test]
    fn verbose_helper_is_drained_while_request_is_written_and_tail_is_bounded() {
        let root = tempfile::tempdir().unwrap();
        // More stderr than a pipe/socket can hold, before reading any request.
        let script = "head -c 1048576 /dev/zero | tr '\\000' x >&2; cat >/dev/null; printf '\\nFINAL_REJECTION_FIXTURE\\n' >&2; exit 1";
        let error = run(shell(script), &fixture_download(root.path())).unwrap_err();
        assert!(error.contains("FINAL_REJECTION_FIXTURE"), "{error}");
        assert!(error.contains("Earlier installer output omitted"));
        assert!(!error.contains("not fully delivered"));
        assert!(error.len() < DIAGNOSTIC_LIMIT + 1024, "{}", error.len());
    }

    #[test]
    fn capture_does_not_wait_for_an_inherited_writer_to_close() {
        let (reader, mut inherited_writer) = UnixStream::pair().unwrap();
        reader.set_read_timeout(Some(READ_WAIT)).unwrap();
        inherited_writer
            .write_all(b"last helper diagnostic")
            .unwrap();
        let finished = AtomicBool::new(true);
        let start = Instant::now();
        let details = collect_diagnostics(reader, &finished);
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(details, "last helper diagnostic");
        // inherited_writer deliberately remained open throughout collection.
        drop(inherited_writer);
    }

    #[test]
    fn immediate_helper_exit_does_not_wait_for_descendant_stderr() {
        let root = tempfile::tempdir().unwrap();
        let finished = root.path().join("release harmless descendant");
        let descendant_done = root.path().join("descendant finished");
        // The harmless descendant retains stderr after the immediate helper
        // exits. It exits cooperatively when released, or after five seconds.
        let mut command = shell(
            "(i=0; while [ \"$i\" -lt 100 ] && [ ! -e \"$1\" ]; do sleep 0.05; i=$((i + 1)); done; : > \"$2\") & cat >/dev/null; printf 'IMMEDIATE_HELPER_EXIT_FIXTURE\\n' >&2; exit 1",
        );
        command.arg("fixture").arg(&finished).arg(&descendant_done);
        let start = Instant::now();
        let result = run(command, &fixture_download(root.path()));
        let elapsed = start.elapsed();
        std::fs::write(finished, b"exit cooperatively").unwrap();
        let cleanup = Instant::now();
        while !descendant_done.exists() && cleanup.elapsed() < Duration::from_secs(1) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(descendant_done.exists());
        assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
        assert!(result
            .unwrap_err()
            .contains("IMMEDIATE_HELPER_EXIT_FIXTURE"));
    }

    #[test]
    fn capture_deadline_applies_even_if_inherited_writer_keeps_writing() {
        let (reader, mut inherited_writer) = UnixStream::pair().unwrap();
        reader.set_read_timeout(Some(READ_WAIT)).unwrap();
        let writer = std::thread::spawn(
            move || {
                while inherited_writer.write_all(&[b'x'; 4096]).is_ok() {}
            },
        );
        let start = Instant::now();
        let details = collect_diagnostics(reader, &AtomicBool::new(true));
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(details.len() <= DIAGNOSTIC_LIMIT + 64);
        writer.join().unwrap();
    }

    #[test]
    fn capture_tolerates_invalid_utf8_and_removes_control_characters() {
        let (reader, mut writer) = UnixStream::pair().unwrap();
        reader.set_read_timeout(Some(READ_WAIT)).unwrap();
        writer
            .write_all(b"bad\xff\x1b\x00\nuseful diagnostic\n")
            .unwrap();
        drop(writer);
        let details = collect_diagnostics(reader, &AtomicBool::new(true));
        assert!(details.contains("useful diagnostic"));
        assert!(!details.contains('\x1b'));
        assert!(!details.contains('\0'));
    }

    #[test]
    fn escaped_and_invalid_diagnostics_fit_persisted_outcome_read_limit() {
        use super::super::lifecycle;
        for bytes in [
            [b'"', b'\\', b'\n', b'\t'].repeat(DIAGNOSTIC_LIMIT),
            vec![0xff; DIAGNOSTIC_LIMIT * 4],
            [0, 0x1b, 7, b'"', 0xff, b'\n'].repeat(DIAGNOSTIC_LIMIT),
        ] {
            let (reader, mut writer) = UnixStream::pair().unwrap();
            reader.set_read_timeout(Some(READ_WAIT)).unwrap();
            let writer = std::thread::spawn(move || {
                writer.write_all(&bytes).unwrap();
                writer.write_all(b"\nFINAL_DIAGNOSTIC_FIXTURE\n").unwrap();
            });
            let diagnostics = collect_diagnostics(reader, &AtomicBool::new(false));
            writer.join().unwrap();
            assert!(diagnostics.len() <= DIAGNOSTIC_LIMIT + 64);
            let message = installation_result(
                Err("Broken pipe (os error 32)".into()),
                Ok(ExitStatus::from_raw(1 << 8)),
                &diagnostics,
            )
            .unwrap_err();
            let root = tempfile::tempdir().unwrap();
            lifecycle::save_outcome(
                root.path(),
                &lifecycle::Outcome {
                    success: false,
                    message: message.clone(),
                },
            )
            .unwrap();
            // Apply outcome()'s exact bounded-read contract in a disposable
            // directory, without changing the real per-user lifecycle outcome.
            let mut encoded = Vec::new();
            std::fs::File::open(root.path().join("last-update.json"))
                .unwrap()
                .take(16385)
                .read_to_end(&mut encoded)
                .unwrap();
            assert!(encoded.len() <= 16384, "{}", encoded.len());
            let restored: lifecycle::Outcome = serde_json::from_slice(&encoded).unwrap();
            assert!(!restored.success);
            assert_eq!(restored.message, message);
            assert!(restored.message.contains("FINAL_DIAGNOSTIC_FIXTURE"));
        }
    }

    #[test]
    fn unavailable_command_returns_without_hanging_capture() {
        let root = tempfile::tempdir().unwrap();
        let start = Instant::now();
        let error = run(
            Command::new(root.path().join("missing-installer-fixture")),
            &fixture_download(root.path()),
        )
        .unwrap_err();
        assert!(error.contains("Could not open"));
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn wait_failure_and_signal_are_not_success() {
        let error = installation_result(
            Ok(()),
            Err(io::Error::other("fixture wait error")),
            "helper detail",
        )
        .unwrap_err();
        assert!(error.contains("Could not confirm"));
        assert!(error.contains("helper detail"));
        let error = installation_result(Ok(()), Ok(ExitStatus::from_raw(15)), "").unwrap_err();
        assert!(error.contains("signal"));
    }
}
