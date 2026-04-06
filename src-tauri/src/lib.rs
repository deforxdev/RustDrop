use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::io::{Read, Write};
use std::fs::File;
use std::path::Path;
use serde::{Serialize, Deserialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DeviceInfo {
    pub ip: String,
    pub name: String,
    pub status: String,
}

#[derive(Serialize, Deserialize)]
struct HelloMessage {
    name: String,
}

#[tauri::command]
fn transfer_file(app: AppHandle, target_ip: String, file_path: String) -> Result<(), String> {
    let app_clone = app.clone();
    thread::spawn(move || {
        if let Err(e) = send_file(&app_clone, &target_ip, &file_path) {
            println!("Transfer failed: {}", e);
        }
    });
    Ok(())
}

fn send_file(app: &AppHandle, target_ip: &str, file_path: &str) -> std::io::Result<()> {
    let path = Path::new(file_path);
    let file_name = path.file_name().unwrap().to_string_lossy().to_string();
    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();

    let mut stream = TcpStream::connect(format!("{}:54322", target_ip))?;
    
    let name_bytes = file_name.as_bytes();
    stream.write_all(&(name_bytes.len() as u32).to_be_bytes())?;
    stream.write_all(name_bytes)?;
    stream.write_all(&file_size.to_be_bytes())?;

    let mut buffer = [0; 65536];
    let mut sent = 0;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 { break; }
        stream.write_all(&buffer[..n])?;
        sent += n as u64;
        let progress = (sent as f64 / file_size as f64 * 100.0) as u32;
        let _ = app.emit("transfer_progress", progress);
        thread::sleep(Duration::from_millis(1));
    }
    let _ = app.emit("transfer_complete", target_ip.to_string());
    Ok(())
}

fn start_tcp_server(app: AppHandle) {
    thread::spawn(move || {
        let listener = TcpListener::bind("0.0.0.0:54322").unwrap();
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let app_c = app.clone();
                let _ = receive_file(app_c, &mut stream);
            }
        }
    });
}

fn receive_file(app: AppHandle, stream: &mut TcpStream) -> std::io::Result<()> {
    let mut len_buf = [0; 4];
    stream.read_exact(&mut len_buf)?;
    let name_len = u32::from_be_bytes(len_buf) as usize;
    
    let mut name_buf = vec![0; name_len];
    stream.read_exact(&mut name_buf)?;
    let file_name = String::from_utf8_lossy(&name_buf).to_string();

    let mut size_buf = [0; 8];
    stream.read_exact(&mut size_buf)?;

    let peer_addr = stream.peer_addr().map(|a| a.ip().to_string()).unwrap_or_else(|_| "Невідомо".to_string());
    let msg = format!("Прийняти файл '{}' від пристрою {}?", file_name, peer_addr);
    let accepted = app.dialog().message(msg)
        .title("Вхідний файл (RustDrop)")
        .kind(tauri_plugin_dialog::MessageDialogKind::Info)
        .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom("Прийняти".to_string(), "Відхилити".to_string()))
        .blocking_show();

    if !accepted {
        return Ok(());
    }

    let downloads = dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let dest_path = downloads.join(&file_name);
    let mut file = File::create(dest_path)?;

    let mut buffer = [0; 65536];
    loop {
        let n = stream.read(&mut buffer)?;
        if n == 0 { break; }
        file.write_all(&buffer[..n])?;
    }
    Ok(())
}

use get_if_addrs::get_if_addrs;

fn start_udp_discovery(app: AppHandle) {
    let socket = UdpSocket::bind("0.0.0.0:54321").expect("could not bind to udp port 54321");
    socket.set_broadcast(true).expect("set_broadcast failed");
    
    let socket_clone = socket.try_clone().unwrap();
    thread::spawn(move || {
        let my_name = std::env::var("USERNAME").unwrap_or_else(|_| "PC".to_string());
        let msg = HelloMessage { name: my_name };
        let msg_bytes = serde_json::to_string(&msg).unwrap();
        loop {
            if let Ok(interfaces) = get_if_addrs() {
                for iface in interfaces {
                    if let get_if_addrs::IfAddr::V4(v4_addr) = iface.addr {
                        if !v4_addr.is_loopback() {
                            if let Some(broadcast) = v4_addr.broadcast {
                                let target = format!("{}:54321", broadcast);
                                let _ = socket_clone.send_to(msg_bytes.as_bytes(), &target);
                            }
                        }
                    }
                }
            }
            let _ = socket_clone.send_to(msg_bytes.as_bytes(), "255.255.255.255:54321");
            thread::sleep(Duration::from_secs(3));
        }
    });

    thread::spawn(move || {
        let mut buf = [0; 1024];
        let mut devices = std::collections::HashMap::new();
        loop {
            if let Ok((amt, src)) = socket.recv_from(&mut buf) {
                if let Ok(msg) = serde_json::from_slice::<HelloMessage>(&buf[..amt]) {
                    let ip = src.ip().to_string();
                    let hostname = msg.name;
                    devices.insert(ip.clone(), DeviceInfo {
                        ip: ip.clone(),
                        name: hostname,
                        status: "Active".to_string(),
                    });
                    
                    let dev_list: Vec<DeviceInfo> = devices.values().cloned().collect();
                    let _ = app.emit("devices_updated", dev_list);
                }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .setup(|app| {
      start_tcp_server(app.handle().clone());
      start_udp_discovery(app.handle().clone());
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![transfer_file])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
