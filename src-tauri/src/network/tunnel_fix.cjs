const fs = require('fs');
let code = fs.readFileSync('G:/oskarlolpo project/minecraftjava/01_Active/p2p/src-tauri/src/network/tunnel.rs', 'utf8');

if (!code.includes('use tauri::Emitter;')) {
    code = code.replace(/use tauri::/g, 'use tauri::Emitter;\nuse tauri::');
}
if (!code.includes('use tauri::AppHandle;')) {
    code = code.replace(/use std::/g, 'use tauri::AppHandle;\nuse std::');
}

// Update bridge_tcp_to_remote
code = code.replace(/pub async fn bridge_tcp_to_remote\(\r?\n\s*mut local_stream: TcpStream,/, 'pub async fn bridge_tcp_to_remote(\n    app: AppHandle,\n    mut local_stream: TcpStream,');
code = code.replace(/copy_bidirectional_tolerant\(&mut local_stream, &mut remote_stream\)/, 'copy_bidirectional_tolerant(&app, &mut local_stream, &mut remote_stream)');

// Update run_reverse_tunnel_control_loop
code = code.replace(/async fn run_reverse_tunnel_control_loop\(\r?\n\s*mut control: Delimited<TcpStream>,\r?\n\s*config: ReverseTunnelConfig,\r?\n\s*cancel: CancellationToken,\r?\n\s*\) -> Result<\(\)> \{/, 'async fn run_reverse_tunnel_control_loop(\n    app: AppHandle,\n    mut control: Delimited<TcpStream>,\n    config: ReverseTunnelConfig,\n    cancel: CancellationToken,\n) -> Result<()> {');
code = code.replace(/let task = tokio::spawn\(run_reverse_tunnel_control_loop\(control, config, cancel\)\);/, 'let task = tokio::spawn(run_reverse_tunnel_control_loop(app, control, config, cancel));');
code = code.replace(/pub async fn connect_reverse_tunnel\(\r?\n\s*config: ReverseTunnelConfig,\r?\n\s*cancel: CancellationToken,\r?\n\s*\) -> Result<ReverseTunnelHandle> \{/, 'pub async fn connect_reverse_tunnel(\n    app: AppHandle,\n    config: ReverseTunnelConfig,\n    cancel: CancellationToken,\n) -> Result<ReverseTunnelHandle> {');
code = code.replace(/copy_bidirectional_tolerant\(&mut local, &mut parts\.io\)/, 'copy_bidirectional_tolerant(&app, &mut local, &mut parts.io)');

// Update copy_bidirectional_tolerant
const newCopyFunc = `async fn copy_bidirectional_tolerant<A, B>(app: &AppHandle, left: &mut A, right: &mut B) -> Result<()>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let mut left_buf = vec![0u8; 16384];
    let mut right_buf = vec![0u8; 16384];
    
    let (mut left_read, mut left_write) = tokio::io::split(left);
    let (mut right_read, mut right_write) = tokio::io::split(right);
    
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tauri::Emitter;
    
    let app_clone1 = app.clone();
    let left_to_right = async move {
        loop {
            match left_read.read(&mut left_buf).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let _ = app_clone1.emit("network_stats", serde_json::json!({ "bytesOut": n }));
                    if let Err(e) = right_write.write_all(&left_buf[..n]).await {
                        if !is_connection_close(&e) {
                            return Err(e);
                        }
                        break;
                    }
                }
                Err(e) => {
                    if !is_connection_close(&e) {
                        return Err(e);
                    }
                    break;
                }
            }
        }
        let _ = right_write.shutdown().await;
        Ok::<_, std::io::Error>(())
    };

    let app_clone2 = app.clone();
    let right_to_left = async move {
        loop {
            match right_read.read(&mut right_buf).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let _ = app_clone2.emit("network_stats", serde_json::json!({ "bytesIn": n }));
                    if let Err(e) = left_write.write_all(&right_buf[..n]).await {
                        if !is_connection_close(&e) {
                            return Err(e);
                        }
                        break;
                    }
                }
                Err(e) => {
                    if !is_connection_close(&e) {
                        return Err(e);
                    }
                    break;
                }
            }
        }
        let _ = left_write.shutdown().await;
        Ok::<_, std::io::Error>(())
    };

    match tokio::try_join!(left_to_right, right_to_left) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}`;

code = code.replace(/async fn copy_bidirectional_tolerant<A, B>\(left: &mut A, right: &mut B\) -> Result<\(\)>[\s\S]*?Err\(error\) => Err\(error\)\.context\(\"tokio::io::copy_bidirectional вернул ошибку\"\),\r?\n\s*\}\r?\n\}/, newCopyFunc);

fs.writeFileSync('G:/oskarlolpo project/minecraftjava/01_Active/p2p/src-tauri/src/network/tunnel.rs', code);
console.log('tunnel.rs updated.');
