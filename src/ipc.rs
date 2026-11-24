use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[must_use]
pub fn socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        if !runtime_dir.is_empty() {
            return PathBuf::from(runtime_dir).join("touchpadflow.sock");
        }
    }
    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    PathBuf::from(format!("/tmp/touchpadflow-{}.sock", user))
}

pub fn send_command(cmd: &str) -> std::io::Result<String> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    writeln!(stream, "{}", cmd)?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;
    Ok(response.trim().to_string())
}

pub struct IpcServer {
    path: PathBuf,
    handle: Option<JoinHandle<()>>,
}

impl IpcServer {
    pub fn start(
        running: Arc<AtomicBool>,
        paused: Arc<AtomicBool>,
        reload_flag: Arc<AtomicBool>,
        device_desc: Arc<std::sync::Mutex<String>>,
    ) -> std::io::Result<Self> {
        let path = socket_path();
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;

        let server_path = path.clone();
        let r_clone = Arc::clone(&running);

        let handle = thread::Builder::new()
            .name("ipc-worker".into())
            .spawn(move || {
                while r_clone.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let mut reader = BufReader::new(&mut stream);
                            let mut line = String::new();
                            if reader.read_line(&mut line).is_ok() {
                                let cmd = line.trim();
                                let resp = match cmd {
                                    "status" => {
                                        let is_p = paused.load(Ordering::Relaxed);
                                        let dev = device_desc.lock().unwrap().clone();
                                        format!(
                                            "{{\"running\":true,\"paused\":{},\"device\":\"{}\"}}\n",
                                            is_p, dev
                                        )
                                    }
                                    "pause" => {
                                        paused.store(true, Ordering::SeqCst);
                                        "{\"status\":\"paused\"}\n".to_string()
                                    }
                                    "resume" => {
                                        paused.store(false, Ordering::SeqCst);
                                        "{\"status\":\"resumed\"}\n".to_string()
                                    }
                                    "toggle" => {
                                        let prev = paused.fetch_xor(true, Ordering::SeqCst);
                                        if !prev {
                                            "{\"status\":\"paused\"}\n".to_string()
                                        } else {
                                            "{\"status\":\"resumed\"}\n".to_string()
                                        }
                                    }
                                    "reload" => {
                                        reload_flag.store(true, Ordering::SeqCst);
                                        "{\"status\":\"reloaded\"}\n".to_string()
                                    }
                                    "stop" => {
                                        r_clone.store(false, Ordering::SeqCst);
                                        "{\"status\":\"stopping\"}\n".to_string()
                                    }
                                    _ => "{\"error\":\"unknown command\"}\n".to_string(),
                                };
                                let _ = stream.write_all(resp.as_bytes());
                                let _ = stream.flush();
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(50));
                        }
                        Err(_) => {
                            thread::sleep(Duration::from_millis(50));
                        }
                    }
                }
                let _ = fs::remove_file(&server_path);
            })?;

        Ok(Self {
            path,
            handle: Some(handle),
        })
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}
