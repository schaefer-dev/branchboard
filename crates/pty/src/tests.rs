use std::sync::mpsc::{Receiver, Sender, channel};

use super::*;

enum Event {
    Output(Vec<u8>),
    Exited(Option<u32>),
}

struct Collect(Mutex<Sender<Event>>);

impl PtySink for Collect {
    fn output(&self, bytes: Vec<u8>) {
        let _ = lock(&self.0).send(Event::Output(bytes));
    }

    fn exited(&self, code: Option<u32>) {
        let _ = lock(&self.0).send(Event::Exited(code));
    }
}

fn spawn(command: &PtyCommand, size: Size) -> (Pty, Receiver<Event>) {
    let (tx, rx) = channel();
    let pty = Pty::spawn(command, size, Arc::new(Collect(Mutex::new(tx)))).unwrap();
    (pty, rx)
}

/// Whether the program had ended when `output_until` returned.
#[derive(Debug, PartialEq, Eq)]
enum Ended {
    Running,
    Exited(Option<u32>),
}

const SIZE: Size = Size { cols: 80, rows: 24 };

/// Collects output until `done` holds for it or the program exits.
fn output_until(events: &Receiver<Event>, done: impl Fn(&str) -> bool) -> (String, Ended) {
    let mut text = String::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        match events.recv_timeout(Duration::from_millis(100)) {
            Ok(Event::Output(bytes)) => {
                text.push_str(&String::from_utf8_lossy(&bytes));
                if done(&text) {
                    return (text, Ended::Running);
                }
            }
            Ok(Event::Exited(code)) => return (text, Ended::Exited(code)),
            Err(_) => {}
        }
    }
    panic!("timed out; output so far: {text:?}");
}

fn shell(script: &str) -> PtyCommand {
    if cfg!(windows) {
        PtyCommand::new("cmd.exe").arg("/c").arg(script)
    } else {
        PtyCommand::new("/bin/sh").arg("-c").arg(script)
    }
}

#[test]
fn runs_a_program_and_reports_its_exit() {
    let script = if cfg!(windows) {
        "echo hello& exit 3"
    } else {
        "printf hello; exit 3"
    };
    let (_pty, events) = spawn(&shell(script), SIZE);
    let (text, exit) = output_until(&events, |_| false);
    assert!(text.contains("hello"), "{text:?}");
    assert_eq!(exit, Ended::Exited(Some(3)));
}

#[test]
fn passes_the_environment() {
    let script = if cfg!(windows) {
        "echo %BRANCHBOARD_TEST%"
    } else {
        "printf %s \"$BRANCHBOARD_TEST\""
    };
    let (_pty, events) = spawn(&shell(script).env("BRANCHBOARD_TEST", "from-env"), SIZE);
    let (text, _) = output_until(&events, |_| false);
    assert!(text.contains("from-env"), "{text:?}");
}

#[cfg(unix)]
#[test]
fn input_reaches_the_program() {
    let (pty, events) = spawn(&shell("read line; printf 'got:%s' \"$line\""), SIZE);
    pty.write(b"ping\r").unwrap();
    let (text, _) = output_until(&events, |text| text.contains("got:ping"));
    assert!(text.contains("got:ping"), "{text:?}");
}

#[cfg(unix)]
#[test]
fn resizing_reaches_the_program() {
    let (pty, events) = spawn(&shell("read go; stty size"), SIZE);
    pty.resize(Size {
        cols: 100,
        rows: 30,
    })
    .unwrap();
    pty.write(b"\r").unwrap();
    let (text, _) = output_until(&events, |text| text.contains("30 100"));
    assert!(text.contains("30 100"), "{text:?}");
}

#[test]
fn kill_ends_the_program() {
    let script = if cfg!(windows) {
        "ping -n 30 127.0.0.1"
    } else {
        "sleep 30"
    };
    let (pty, events) = spawn(&shell(script), SIZE);
    std::thread::sleep(Duration::from_millis(200));
    pty.kill();
    let (_, exit) = output_until(&events, |_| false);
    assert!(matches!(exit, Ended::Exited(_)));
    assert!(matches!(pty.write(b"x"), Err(PtyError::Closed)));
}

#[test]
fn a_missing_program_fails_to_spawn() {
    let (tx, _rx) = channel();
    let result = Pty::spawn(
        &PtyCommand::new("/definitely/not/here"),
        SIZE,
        Arc::new(Collect(Mutex::new(tx))),
    );
    assert!(matches!(result, Err(PtyError::Spawn(_))));
}

#[test]
fn coalesces_output_bursts() {
    let (tx, rx) = channel();
    for part in ["a", "b", "c"] {
        tx.send(part.as_bytes().to_vec()).unwrap();
    }
    drop(tx);
    let (events_tx, events) = channel();
    forward(&rx, &Collect(Mutex::new(events_tx)));
    let Ok(Event::Output(bytes)) = events.try_recv() else {
        panic!("no output");
    };
    assert_eq!(bytes, b"abc");
    assert!(events.try_recv().is_err());
}
