// Permission requests answered on the card — off unless the user turns it on (config.json
// `permFromCard`). Claude Code runs our hook when it is about to ask for permission; the hook
// (bin/hook.rs `ask_widget`) connects here, says what is asked and waits on the connection. The
// session's row gets «Allow», «Deny» and «In the terminal»; the click is written back as one word
// and becomes the hook's decision — or none, and the request goes on to Claude Code's own dialog.
//
// A connection rather than a file next to the session's state: an answer can only come from this
// process, on a click in the widget's own window (main.rs `permit` takes it from no other), so a
// session can't write itself a permission. The port is on 127.0.0.1 alone, picked by the system and
// left in perm-port in the data folder. A request lives as long as its hook does: when the hook is
// gone — answered in the terminal, timed out, the session ended — the row goes back to what it was.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::Mutex;
use std::time::Duration;

/// A request waiting for a click, as the hook described it.
#[derive(Clone)]
pub struct Waiting {
    pub sid: String,
    #[allow(dead_code)]
    pub tool: String,
    pub detail: String,
    pub more: String,
    pub since: u64,
}

struct Asked {
    what: Waiting,
    stream: TcpStream,
}

static WAITING: Mutex<Vec<Asked>> = Mutex::new(Vec::new());

/// More than this many at once is not a user's sessions asking.
const MOST: usize = 32;

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn safe(sid: &str) -> bool {
    !sid.is_empty()
        && sid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Starts listening and leaves the port for the hook. A build under work listens only when asked
/// to (MEOWNITOR_PERMS), so it doesn't take the installed widget's requests.
pub fn spawn() {
    if cfg!(debug_assertions) && std::env::var_os("MEOWNITOR_PERMS").is_none() {
        return;
    }
    let Some(dir) = crate::sessions::data_dir() else {
        return;
    };
    let Ok(listener) = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) else {
        return;
    };
    let Ok(addr) = listener.local_addr() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    if std::fs::write(dir.join("perm-port"), addr.port().to_string()).is_err() {
        return;
    }
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            // Read off this thread: a connection that says nothing mustn't hold up the next one.
            std::thread::spawn(move || take(stream));
        }
    });
}

/// One request: a line of JSON from the hook, then the connection is kept until it is answered.
fn take(stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let Ok(reader) = stream.try_clone() else {
        return;
    };
    let mut line = String::new();
    if BufReader::new(reader.take(16 * 1024))
        .read_line(&mut line)
        .is_err()
    {
        return;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
        return;
    };
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let sid = text("sid");
    if !safe(&sid) {
        return;
    }
    let asked = Asked {
        what: Waiting {
            sid,
            tool: text("tool"),
            detail: text("detail"),
            more: text("more"),
            since: now_ms(),
        },
        stream,
    };
    let mut waiting = WAITING.lock().unwrap();
    // A session asks for one thing at a time: a new request takes the place of its old one, whose
    // hook reads the end of its connection and leaves the request to the dialog.
    waiting.retain(|a| a.what.sid != asked.what.sid);
    if waiting.len() < MOST {
        waiting.push(asked);
    }
}

/// Whether the hook is still there, waiting on its end of the connection.
fn alive(stream: &TcpStream) -> bool {
    if stream.set_nonblocking(true).is_err() {
        return false;
    }
    let mut byte = [0u8; 1];
    let there = match stream.peek(&mut byte) {
        Ok(0) => false,
        Ok(_) => true,
        Err(e) => e.kind() == std::io::ErrorKind::WouldBlock,
    };
    let _ = stream.set_nonblocking(false);
    there
}

/// The requests whose hooks still wait.
pub fn waiting() -> Vec<Waiting> {
    let mut waiting = WAITING.lock().unwrap();
    waiting.retain(|a| alive(&a.stream));
    waiting.iter().map(|a| a.what.clone()).collect()
}

/// Hands the session's waiting hook its answer — "allow", "deny", or "dialog" for none. False when
/// no request of that session waits any more.
pub fn answer(sid: &str, word: &str) -> bool {
    let asked = {
        let mut waiting = WAITING.lock().unwrap();
        waiting
            .iter()
            .position(|a| a.what.sid == sid)
            .map(|i| waiting.remove(i))
    };
    let Some(mut asked) = asked else {
        return false;
    };
    let sent = asked
        .stream
        .write_all(format!("{word}\n").as_bytes())
        .is_ok();
    let _ = asked.stream.flush();
    sent
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hook's side, as bin/hook.rs has it: say what is asked, wait for the word.
    fn hook(port: u16, sid: &str) -> std::thread::JoinHandle<String> {
        let ask = serde_json::json!({ "sid": sid, "tool": "Bash", "detail": "Bash · Build", "more": "cargo build" });
        std::thread::spawn(move || {
            let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            s.write_all(format!("{ask}\n").as_bytes()).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut word = String::new();
            let _ = BufReader::new(s).read_line(&mut word);
            word.trim().to_string()
        })
    }

    fn until(what: impl Fn() -> bool) {
        for _ in 0..100 {
            if what() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("it never came");
    }

    #[test]
    fn a_request_waits_for_its_answer_and_goes_with_its_hook() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || take(stream));
            }
        });
        let asks = |sid: &str| waiting().iter().any(|a| a.sid == sid);

        let h = hook(port, "perm-a");
        until(|| asks("perm-a"));
        let asked = waiting().into_iter().find(|a| a.sid == "perm-a").unwrap();
        assert_eq!(
            (asked.tool.as_str(), asked.more.as_str()),
            ("Bash", "cargo build")
        );
        assert!(answer("perm-a", "allow"));
        assert_eq!(h.join().unwrap(), "allow");
        assert!(!asks("perm-a"));
        assert!(!answer("perm-a", "deny"), "answered once");

        // A hook that is gone takes its request with it.
        let gone = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        (&gone)
            .write_all(b"{\"sid\":\"perm-b\",\"tool\":\"Write\"}\n")
            .unwrap();
        until(|| asks("perm-b"));
        drop(gone);
        until(|| !asks("perm-b"));

        // A session's new request takes the place of its old one.
        let old = hook(port, "perm-c");
        until(|| asks("perm-c"));
        let new = hook(port, "perm-c");
        assert_eq!(
            old.join().unwrap(),
            "",
            "the old hook is let go with no word"
        );
        until(|| asks("perm-c"));
        assert!(answer("perm-c", "deny"));
        assert_eq!(new.join().unwrap(), "deny");
    }
}
