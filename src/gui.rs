use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::thread;

use crate::config::AppConfig;
use crate::error::Result;
use crate::gui_assets::HTML_INDEX;

pub fn start_gui_server(port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)?;
    let session_token = Arc::new(generate_session_token());
    let url = format!("http://{}?token={}", addr, session_token);

    println!("======================================================");
    println!(" TouchpadFlow Control Center (Hardened)");
    println!(" URL: {}", url);
    println!(" Session Token: {}", session_token);
    println!(" Press Ctrl+C in this terminal to close.");
    println!("======================================================");

    // Launch default web browser
    let _ = Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .or_else(|_| Command::new("open").arg(&url).spawn());

    for mut stream in listener.incoming().flatten() {
        let tok = Arc::clone(&session_token);
        thread::spawn(move || {
            let _ = handle_connection(&mut stream, port, &tok);
        });
    }

    Ok(())
}

fn generate_session_token() -> String {
    let mut bytes = [0u8; 16];
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut bytes);
    } else {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = ((now >> (i * 8)) & 0xFF) as u8;
        }
    }
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn handle_connection(stream: &mut TcpStream, port: u16, token: &str) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let full_path = parts.next().unwrap_or("/");

    let (path, query) = match full_path.split_once('?') {
        Some((p, q)) => (p, q),
        None => (full_path, ""),
    };

    let mut content_length = 0usize;
    let mut origin_valid = true;
    let mut host_valid = false;
    let mut client_token = String::new();

    let expected_port = port.to_string();

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        let lower = line.to_lowercase();
        if let Some(val) = lower.strip_prefix("content-length:") {
            if let Ok(len) = val.trim().parse::<usize>() {
                content_length = len.min(65536); // Cap at 64KB for security
            }
        } else if let Some(val) = lower.strip_prefix("host:") {
            let trimmed = val.trim();
            if trimmed == format!("127.0.0.1:{}", expected_port)
                || trimmed == format!("localhost:{}", expected_port)
            {
                host_valid = true;
            }
        } else if let Some(val) = lower.strip_prefix("origin:") {
            let trimmed = val.trim();
            if !trimmed.starts_with(&format!("http://127.0.0.1:{}", expected_port))
                && !trimmed.starts_with(&format!("http://localhost:{}", expected_port))
            {
                origin_valid = false;
            }
        } else if let Some(val) = line.strip_prefix("X-Session-Token:") {
            client_token = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("x-session-token:") {
            client_token = val.trim().to_string();
        }
    }

    // Read full body if Content-Length > 0
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }
    let body_str = String::from_utf8_lossy(&body);

    // CSRF and Origin checks
    if !host_valid || !origin_valid {
        let resp = "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nAccess denied: invalid origin/host";
        stream.write_all(resp.as_bytes())?;
        return Ok(());
    }

    // Token check for API calls
    if path.starts_with("/api/") {
        let token_in_query = query
            .split('&')
            .any(|pair| pair == format!("token={}", token));
        if client_token != token && !token_in_query {
            let resp = "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"error\":\"unauthorized: invalid session token\"}";
            stream.write_all(resp.as_bytes())?;
            return Ok(());
        }
    }

    if method == "GET" && path == "/" {
        let token_script = format!(
            "<script>window.SESSION_TOKEN = \"{}\";</script>\n</head>",
            token
        );
        let html_with_token = HTML_INDEX.replace("</head>", &token_script);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html_with_token.len(),
            html_with_token
        );
        stream.write_all(response.as_bytes())?;
    } else if method == "GET" && path == "/api/status" {
        let resp_json = match crate::ipc::send_command("status") {
            Ok(s) => s,
            Err(_) => "{\"running\":false,\"error\":\"daemon not responding\"}".to_string(),
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            resp_json.len(),
            resp_json
        );
        stream.write_all(response.as_bytes())?;
    } else if method == "GET" && path == "/api/config" {
        let cfg = AppConfig::load_or_default(Some(&config_path()));
        let json = serde_json::to_string(&cfg).unwrap_or_else(|_| "{}".into());
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            json.len(),
            json
        );
        stream.write_all(response.as_bytes())?;
    } else if method == "POST" && path == "/api/config" {
        let result = match serde_json::from_str::<serde_json::Value>(&body_str) {
            Ok(val) => update_config_transactional(&val),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        };

        let response = match result {
            Ok(true) => {
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"ok\",\"reloaded\":true}"
            }
            Ok(false) => {
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"validation_failed\"}"
            }
            Err(e) => &format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{{\"error\":\"{}\"}}",
                e
            ),
        };
        stream.write_all(response.as_bytes())?;
    } else if method == "POST" && path.starts_with("/api/service/") {
        let action = path.trim_start_matches("/api/service/");
        let resp_json = match action {
            "restart" => {
                let _ = Command::new("systemctl")
                    .args(["--user", "restart", "touchpadflow"])
                    .status();
                "{\"status\":\"restarted\"}".to_string()
            }
            "stop" => {
                let _ = Command::new("systemctl")
                    .args(["--user", "stop", "touchpadflow"])
                    .status();
                "{\"status\":\"stopped\"}".to_string()
            }
            "start" => {
                let _ = Command::new("systemctl")
                    .args(["--user", "start", "touchpadflow"])
                    .status();
                "{\"status\":\"started\"}".to_string()
            }
            "pause" => match crate::ipc::send_command("pause") {
                Ok(r) => r,
                Err(_) => "{\"error\":\"daemon offline\"}".to_string(),
            },
            "resume" => match crate::ipc::send_command("resume") {
                Ok(r) => r,
                Err(_) => "{\"error\":\"daemon offline\"}".to_string(),
            },
            "toggle" => match crate::ipc::send_command("toggle") {
                Ok(r) => r,
                Err(_) => "{\"error\":\"daemon offline\"}".to_string(),
            },
            _ => "{\"error\":\"unknown action\"}".to_string(),
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            resp_json.len(),
            resp_json
        );
        stream.write_all(response.as_bytes())?;
    } else {
        let response = "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        stream.write_all(response.as_bytes())?;
    }

    stream.flush()?;
    Ok(())
}

fn update_config_transactional(incoming: &serde_json::Value) -> std::io::Result<bool> {
    let p = config_path();
    let mut cfg = AppConfig::load_or_default(Some(&p));

    if let Some(l) = incoming.get("left_edge_ratio").and_then(|v| v.as_f64()) {
        cfg.left_edge_ratio = (l as f32).clamp(0.05, 0.40);
    }
    if let Some(r) = incoming.get("right_edge_ratio").and_then(|v| v.as_f64()) {
        cfg.right_edge_ratio = (r as f32).clamp(0.05, 0.40);
    }
    if let Some(t) = incoming
        .get("edge_trigger_threshold")
        .and_then(|v| v.as_f64())
    {
        cfg.edge_trigger_threshold = t.clamp(5.0, 200.0);
    }
    if let Some(w) = incoming.get("settle_window_ms").and_then(|v| v.as_u64()) {
        cfg.settle_window_ms = w.clamp(10, 200);
    }
    if let Some(v) = incoming.get("volume_step").and_then(|v| v.as_f64()) {
        cfg.volume_step = v.clamp(0.005, 0.15);
    }
    if let Some(b) = incoming.get("brightness_step").and_then(|v| v.as_f64()) {
        cfg.brightness_step = b.clamp(0.005, 0.15);
    }
    if let Some(shortcuts) = incoming.get("shortcuts").and_then(|v| v.as_object()) {
        for (k, val) in shortcuts {
            if let Some(s) = val.as_str() {
                cfg.shortcuts.insert(k.clone(), s.to_string());
            }
        }
    }

    if let Some(parent) = p.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let tmp_path = p.with_extension("json.tmp");
    let json_bytes = serde_json::to_string_pretty(&cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(&tmp_path, json_bytes)?;
    fs::rename(&tmp_path, &p)?;

    // Signal running daemon to reload immediately via IPC
    let _ = crate::ipc::send_command("reload");
    Ok(true)
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/touchpadflow.json")
}
