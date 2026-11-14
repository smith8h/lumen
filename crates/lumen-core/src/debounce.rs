//! Runs a function on a background thread once things have been quiet for a moment.
//! Used to batch writes of history and clipboard files.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

enum Msg {
    Poke,
    Flush(Sender<()>),
}

pub struct Debouncer {
    tx: Sender<Msg>,
}

impl Debouncer {
    pub fn new(name: &str, delay: Duration, f: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel::<Msg>();
        std::thread::Builder::new()
            .name(format!("save-{name}"))
            .spawn(move || {
                let mut pending = false;
                loop {
                    let msg = if pending { rx.recv_timeout(delay) } else { rx.recv().map_err(|_| RecvTimeoutError::Disconnected) };
                    match msg {
                        Ok(Msg::Poke) => pending = true,
                        Ok(Msg::Flush(done)) => {
                            if pending {
                                f();
                                pending = false;
                            }
                            let _ = done.send(());
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            f();
                            pending = false;
                        }
                        Err(RecvTimeoutError::Disconnected) => {
                            if pending {
                                f();
                            }
                            return;
                        }
                    }
                }
            })
            .expect("spawn save thread");
        Self { tx }
    }

    pub fn poke(&self) {
        let _ = self.tx.send(Msg::Poke);
    }

    /// Run any pending save now and wait for it (on quit).
    pub fn flush(&self) {
        let (tx, rx) = mpsc::channel();
        if self.tx.send(Msg::Flush(tx)).is_ok() {
            let _ = rx.recv_timeout(Duration::from_secs(2));
        }
    }
}
