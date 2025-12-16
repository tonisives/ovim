use std::env;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// IPC command from CLI to main app
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum IpcCommand {
    GetMode,
    SetMode(String),
    Toggle,
    Insert,
    Normal,
    Visual,
}

/// IPC response from main app to CLI
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum IpcResponse {
    Mode(String),
    Ok,
    Error(String),
}

fn socket_path() -> PathBuf {
    let runtime_dir = dirs::runtime_dir()
        .or_else(dirs::cache_dir)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    runtime_dir.join("ovim.sock")
}

async fn send_command(cmd: IpcCommand) -> Result<IpcResponse, String> {
    let path = socket_path();

    let stream = UnixStream::connect(&path)
        .await
        .map_err(|e| format!("Failed to connect to ovim (is it running?): {}", e))?;

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    let cmd_str = serde_json::to_string(&cmd).map_err(|e| e.to_string())?;
    writer
        .write_all(cmd_str.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    writer.write_all(b"\n").await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;

    let mut line = String::new();
    reader.read_line(&mut line).await.map_err(|e| e.to_string())?;

    let response: IpcResponse = serde_json::from_str(line.trim())
        .map_err(|e| format!("Invalid response: {}", e))?;

    Ok(response)
}

fn print_usage() {
    eprintln!("ovim - System-wide Vim mode control");
    eprintln!();
    eprintln!("Usage: ovim <command>");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  mode          Get current mode");
    eprintln!("  toggle        Toggle between insert and normal mode");
    eprintln!("  insert, i     Switch to insert mode");
    eprintln!("  normal, n     Switch to normal mode");
    eprintln!("  visual, v     Switch to visual mode");
    eprintln!("  set <mode>    Set mode to insert/normal/visual");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  ovim toggle     # Toggle mode (useful for Karabiner)");
    eprintln!("  ovim normal     # Enter normal mode");
    eprintln!("  ovim insert     # Enter insert mode");
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let command = args[1].as_str();

    let ipc_cmd = match command {
        "mode" | "get" | "status" => IpcCommand::GetMode,
        "toggle" | "t" => IpcCommand::Toggle,
        "insert" | "i" => IpcCommand::Insert,
        "normal" | "n" => IpcCommand::Normal,
        "visual" | "v" => IpcCommand::Visual,
        "set" => {
            if args.len() < 3 {
                eprintln!("Error: 'set' requires a mode argument (insert/normal/visual)");
                std::process::exit(1);
            }
            IpcCommand::SetMode(args[2].clone())
        }
        "help" | "-h" | "--help" => {
            print_usage();
            std::process::exit(0);
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage();
            std::process::exit(1);
        }
    };

    match send_command(ipc_cmd).await {
        Ok(response) => match response {
            IpcResponse::Mode(mode) => {
                println!("{}", mode);
            }
            IpcResponse::Ok => {
                // Success, no output needed
            }
            IpcResponse::Error(msg) => {
                eprintln!("Error: {}", msg);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
