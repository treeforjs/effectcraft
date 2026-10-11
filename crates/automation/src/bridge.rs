//! Blocking client for the desktop app's JSON-lines control channel (`docs/control-protocol.md`).
//! One persistent loopback connection; failed calls are never automatically replayed.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde_json::{Value, json};

use crate::{Error, Result};

/// Slightly longer than the app's own 60 s per-request timeout, so its error reply wins.
const READ_TIMEOUT: Duration = Duration::from_secs(65);

pub struct BridgeClient {
    addr: String,
    conn: Option<(BufReader<TcpStream>, TcpStream)>,
    next_id: u64,
}

impl BridgeClient {
    /// `addr`: a port (`9877`) or `host:port` on loopback (`127.0.0.1`, `localhost`, `[::1]`).
    pub fn new(addr: &str) -> Result<Self> {
        let addr = if addr.parse::<u16>().is_ok() { format!("127.0.0.1:{addr}") } else { addr.to_string() };
        let host = addr.rsplit_once(':').map(|(h, _)| h).unwrap_or(&addr);
        if !matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1") {
            return Err(Error::BadArgs(format!("bridge address must be loopback (PORT or 127.0.0.1:PORT), got `{addr}`")));
        }
        Ok(Self { addr, conn: None, next_id: 1 })
    }

    pub fn addr(&self) -> &str {
        &self.addr
    }

    fn connect(&self) -> Result<(BufReader<TcpStream>, TcpStream)> {
        let sa = self
            .addr
            .to_socket_addrs()
            .map_err(|e| Error::Bridge(format!("{}: {e}", self.addr)))?
            .next()
            .ok_or_else(|| Error::Bridge(format!("cannot resolve {}", self.addr)))?;
        let s = TcpStream::connect_timeout(&sa, Duration::from_secs(5))
            .map_err(|e| Error::Bridge(format!("cannot connect to {} ({e}); start the app with `effectcraft --control <port>`", self.addr)))?;
        s.set_read_timeout(Some(READ_TIMEOUT)).ok();
        s.set_nodelay(true).ok();
        let r = s.try_clone().map_err(|e| Error::Bridge(e.to_string()))?;
        Ok((BufReader::new(r), s))
    }

    /// Call a control method; returns the reply's `result`, or the app's `error` as [`Error::App`].
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        if self.conn.is_none() {
            self.conn = Some(self.connect()?);
        }
        let id = self.next_id;
        self.next_id += 1;
        let Some((r, w)) = self.conn.as_mut() else {
            return Err(Error::Bridge(format!("not connected to {}", self.addr)));
        };
        let line = format!("{}\n", json!({"id": id, "method": method, "params": params}));
        let res: Result<Value> = (|| {
            w.write_all(line.as_bytes()).map_err(|e| Error::Bridge(e.to_string()))?;
            let mut buf = String::new();
            match r.read_line(&mut buf) {
                Ok(0) => return Err(Error::Bridge("connection closed by the app".into())),
                Ok(_) => {}
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                    return Err(Error::Bridge(format!("no reply to `{method}` within {}s (is the app window running?)", READ_TIMEOUT.as_secs())));
                }
                Err(e) => return Err(Error::Bridge(e.to_string())),
            }
            serde_json::from_str(&buf).map_err(|e| Error::Bridge(format!("bad reply: {e}")))
        })();
        match res {
            Ok(v) if v.get("ok").and_then(Value::as_bool) == Some(true) => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
            Ok(v) => Err(Error::App(v.get("error").and_then(Value::as_str).unwrap_or("error").to_string())),
            Err(e) => {
                // EOF or a write/read failure does not prove that the app skipped the command.
                // Reconnect only for the caller's next explicit request, never replay this one.
                self.conn = None;
                Err(Error::Bridge(format!("{e}; request outcome is unknown, inspect app state before retrying")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn a_lost_reply_never_replays_a_mutation() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = BridgeClient::new(&listener.local_addr().unwrap().to_string()).unwrap();
        let applied = Arc::new(AtomicUsize::new(0));
        let counter = applied.clone();
        let server = std::thread::spawn(move || {
            for index in 0..2 {
                let (stream, _) = listener.accept().unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                if request["method"] == "engine.execute" {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
                if index == 1 {
                    let reply = json!({"id": request["id"], "ok": true, "result": {"applied": counter.load(Ordering::SeqCst)}});
                    writeln!(reader.get_mut(), "{reply}").unwrap();
                }
                // The first mutation committed, but its reply was lost as the socket closed.
            }
        });
        let error = client.call("engine.execute", json!({"command": "layer.newSolid", "params": {}})).unwrap_err();
        assert!(error.to_string().contains("outcome is unknown"), "{error}");
        assert_eq!(applied.load(Ordering::SeqCst), 1);
        assert_eq!(client.call("ui.inspect", json!({})).unwrap(), json!({"applied": 1}));
        server.join().unwrap();
        assert_eq!(applied.load(Ordering::SeqCst), 1);
    }
}
