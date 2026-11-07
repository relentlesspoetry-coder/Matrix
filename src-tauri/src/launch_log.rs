//! Standalone sink mode, dispatched before desktop/runtime initialization.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    os::fd::AsFd,
};

pub(crate) const LIMIT: usize = 65536;

pub(crate) fn run() -> io::Result<()> {
    use std::os::fd::FromRawFd;
    // Keep the update blocked while any descendant still writes game output.
    // This descriptor is set only in the sink child, never the game command.
    let _activity = if std::env::var("MATRIX_LOG_ACTIVITY_FD").as_deref() == Ok("3") {
        Some(unsafe { File::from_raw_fd(3) })
    } else {
        None
    };
    // stdout is the private, already-open log file supplied by the launcher.
    let mut file = File::from(io::stdout().as_fd().try_clone_to_owned()?);
    drain(io::stdin().lock(), &mut file)
}

fn drain(mut input: impl Read, file: &mut File) -> io::Result<()> {
    let mut tail = Vec::with_capacity(LIMIT + 4096);
    let mut buffer = [0; 4096];
    loop {
        let n = match input.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        tail.extend_from_slice(&buffer[..n]);
        if tail.len() > LIMIT {
            tail.drain(..tail.len() - LIMIT);
        }
        // Regular-file writes have no userspace buffering. Keep consuming input
        // even on ENOSPC or other log errors, so logging cannot block the game.
        let _ = (|| -> io::Result<()> {
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&tail)?;
            file.set_len(tail.len() as u64)
        })();
    }
}

#[cfg(test)]
pub(crate) fn fixture_name() -> String {
    let (_, module) = module_path!().split_once("::").expect("sink module path");
    format!("{module}::tests::sink_fixture")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test executables use the harness entry point instead of production main.
    #[test]
    fn sink_fixture() {
        if std::env::var_os("MATRIX_TEST_LOG_SINK").is_some() {
            std::process::exit(if run().is_ok() { 0 } else { 1 });
        }
    }

    #[test]
    fn keeps_recent_output_after_limit() {
        let mut file = tempfile::tempfile().unwrap();
        let mut data = vec![b'x'; LIMIT * 3];
        data.extend_from_slice(b"late failure diagnostic\n");
        drain(io::Cursor::new(&data), &mut file).unwrap();
        file.rewind().unwrap();
        let mut saved = Vec::new();
        file.read_to_end(&mut saved).unwrap();
        assert_eq!(saved, data[data.len() - LIMIT..]);
    }

    #[test]
    fn write_failure_does_not_stop_draining() {
        let mut full = File::options().write(true).open("/dev/full").unwrap();
        let data = vec![b'x'; LIMIT * 3];
        let mut input = io::Cursor::new(&data);
        drain(&mut input, &mut full).unwrap();
        assert_eq!(input.position(), data.len() as u64);
    }
}
