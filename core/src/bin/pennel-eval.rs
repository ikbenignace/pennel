use std::io::{BufRead, Read, Write};

fn run() -> std::io::Result<()> {
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    loop {
        let mut line = String::new();
        let count = input
            .by_ref()
            .take(pennel_core::evaluation::MAX_REQUEST_BYTES as u64 + 1)
            .read_line(&mut line)?;
        if count == 0 {
            return Ok(());
        }
        if line.len() > pennel_core::evaluation::MAX_REQUEST_BYTES {
            writeln!(output, "{{\"error\":\"request_too_large\"}}")?;
            return Ok(());
        }
        let result = serde_json::from_str(&line)
            .map_err(|_| "invalid_json")
            .and_then(pennel_core::evaluation::evaluate)
            .unwrap_or_else(|code| serde_json::json!({"error":code}));
        serde_json::to_writer(&mut output, &result)?;
        writeln!(output)?;
        output.flush()?;
    }
}

fn main() {
    if run().is_err() {
        eprintln!("Evaluation bridge input/output failure.");
        std::process::exit(1);
    }
}
