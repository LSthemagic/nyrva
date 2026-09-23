//! A cached terminal dashboard; redirected output is one plain frame or JSON.
use crate::{cli, everywhere::Flags, observatory};
use std::{
    io::{self, IsTerminal, Write},
    path::Path,
    sync::mpsc,
    time::Duration,
};
pub(crate) fn run(args: &[String], root: &Path, out: &mut dyn Write) -> Result<(), String> {
    let flags = Flags::parse(
        args,
        &["--interval", "--iterations"],
        &["--once", "--json", "--no-ansi"],
    )?;
    let interval = flags.number("--interval", 2, 1, 60)?;
    let iterations = flags.number("--iterations", 3600, 1, 3600)?;
    let live = !flags.has("--once")
        && !flags.has("--json")
        && io::stdout().is_terminal()
        && io::stdin().is_terminal();
    if flags.has("--json") {
        return cli::write_json(out, &observatory::cockpit(root, cli::now_ms())?);
    }
    let (tx, rx) = mpsc::channel();
    if live {
        // Canonical input leaves terminal modes untouched even on a hard interruption.
        std::thread::spawn(move || loop {
            let mut line = String::new();
            if io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
                let _ = tx.send("q".into());
                break;
            }
            if tx.send(line.trim().to_lowercase()).is_err() {
                break;
            }
        });
    }
    let mut view = String::from("quotas");
    for frame in 0..if live { iterations } else { 1 } {
        if live && !flags.has("--no-ansi") {
            write!(out, "\x1b[2J\x1b[H").map_err(|_| "terminal output closed")?;
        }
        writeln!(out, "Nyrva local — {view} — frame {}", frame + 1)
            .map_err(|_| "terminal output closed")?;
        let now = cli::now_ms();
        let result = if view == "sessions" {
            observatory::sessions(root, now).map(|rows| {
                rows.into_iter()
                    .take(50)
                    .map(|s| {
                        format!(
                            "{} | {}",
                            cli::format_snapshot(&s, now),
                            s.session
                                .as_ref()
                                .and_then(|s| s.id.as_deref())
                                .unwrap_or("session unavailable")
                        )
                    })
                    .collect::<Vec<_>>()
            })
        } else {
            cli::read_current(root, now).map(|rows| {
                rows.iter()
                    .take(50)
                    .map(|s| cli::format_snapshot(s, now))
                    .collect::<Vec<_>>()
            })
        };
        match result {
            Ok(rows) if rows.is_empty() => {
                writeln!(out, "No local observations. Missing data is not zero.")
            }
            Ok(rows) => writeln!(out, "{}", rows.join("\n")),
            Err(_) => writeln!(
                out,
                "Local cache unavailable; no quota or reset was inferred."
            ),
        }
        .map_err(|_| "terminal output closed")?;
        if !live {
            return Ok(());
        }
        writeln!(out, "\nCached data only. Enter: q = quit; s = sessions; p = quotas. Ctrl-C exits without changing terminal modes.").map_err(|_| "terminal output closed")?;
        out.flush().map_err(|_| "terminal output closed")?;
        match rx.recv_timeout(Duration::from_secs(interval)) {
            Ok(key) if key == "q" => break,
            Ok(key) if key == "s" => view = "sessions".into(),
            Ok(key) if key == "p" => view = "quotas".into(),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            _ => (),
        }
    }
    Ok(())
}
