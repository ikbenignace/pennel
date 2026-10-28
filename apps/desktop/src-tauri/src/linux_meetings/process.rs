use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

pub(super) fn command(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
}

pub(super) fn terminate(child: &mut Child) {
    #[cfg(unix)]
    if let Ok(pid) = i32::try_from(child.id()) {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        // Every process is spawned into its own group. Killing that group also closes pipes
        // inherited by a plugin scanner, so reader joins remain bounded after a timeout.
        unsafe {
            let _ = kill(-pid, 9);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

pub(super) fn output(
    program: &str,
    arguments: &[&str],
    timeout: Duration,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut child = command(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("LC_ALL", "C")
        .env("GST_DEBUG", "0")
        .spawn()
        .map_err(|_| format!("{program} is unavailable"))?;
    let stdout = child.stdout.take().ok_or("Process output is missing")?;
    let reader = match std::thread::Builder::new()
        .name("pennel-platform-output".into())
        .spawn(move || read_bounded(stdout, limit))
    {
        Ok(reader) => reader,
        Err(error) => {
            terminate(&mut child);
            return Err(error.to_string());
        }
    };
    let status = wait(&mut child, timeout);
    let bytes = reader
        .join()
        .map_err(|_| "Reading process output failed".to_owned())??;
    if !status?.success() {
        return Err(format!("{program} could not be run"));
    }
    Ok(bytes)
}

pub(super) fn read_bounded(mut reader: impl Read, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut block = [0u8; 8192];
    loop {
        let read = reader.read(&mut block).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) > limit {
            return Err("Process output is too large".into());
        }
        bytes.extend_from_slice(&block[..read]);
    }
    Ok(bytes)
}

pub(super) fn wait(child: &mut Child, timeout: Duration) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            terminate(child);
            return Err("The system process did not respond in time".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_is_bounded_without_truncating_success() {
        assert_eq!(read_bounded(&b"test"[..], 4).unwrap(), b"test");
        assert!(read_bounded(&b"tests"[..], 4).is_err());
    }
}
