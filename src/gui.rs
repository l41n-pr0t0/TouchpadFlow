use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::thread;

use crate::config::AppConfig;
use crate::error::Result;
use crate::gui_assets::HTML_INDEX;

pub fn start_gui_server(port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)?;
    let url = format!("http://{}", addr);

    println!("======================================================");
    println!(" TouchpadFlow Control Center active at: {}", url);
    println!(" Press Ctrl+C in this terminal to close.");
    println!("======================================================");

    // Launch default web browser
    let _ = Command::new("xdg-open").arg(&url).spawn().or_else(|_| {
        Command::new("open").arg(&url).spawn()
    });

    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            thread::spawn(move || {
                let _ = handle_connection(&mut stream);
            });
        }
    }

    Ok(())
}

fn handle_connection(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut buffer = [0u8; 4096];
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let first_line = request.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");

    if method == "GET" && path == "/" {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            HTML_INDEX.len(),
            HTML_INDEX
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
        if let Some(body_start) = request.find("\r\n\r\n") {
            let body = &request[body_start + 4..];
            if let Ok(new_cfg) = serde_json::from_str::<serde_json::Value>(body) {
                let p = config_path();
                if let Some(parent) = p.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(p, serde_json::to_string_pretty(&new_cfg).unwrap_or_default());
            }
        }
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
        stream.write_all(response.as_bytes())?;
    } else if method == "POST" && path.starts_with("/api/service/") {
        let action = path.trim_start_matches("/api/service/");
        let _ = match action {
            "restart" => Command::new("systemctl").args(["--user", "restart", "touchpadflow"]).status(),
            "stop" => Command::new("systemctl").args(["--user", "stop", "touchpadflow"]).status(),
            "start" => Command::new("systemctl").args(["--user", "start", "touchpadflow"]).status(),
            _ => Ok(std::process::ExitStatus::default()),
        };
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
        stream.write_all(response.as_bytes())?;
    } else {
        let response = "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        stream.write_all(response.as_bytes())?;
    }

    stream.flush()?;
    Ok(())
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config/touchpadflow.json")
}
