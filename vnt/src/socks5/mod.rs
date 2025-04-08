use anyhow::Context;
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

use crate::util::StopManager;

mod protocol;
pub use protocol::*;

#[cfg(test)]
mod tests;

/// Start a SOCKS5 server
pub fn start_socks5_server(
    stop_manager: StopManager,
    port: u16,
    enable_logging: bool,
) -> anyhow::Result<()> {
    // Check if we're already in a Tokio runtime
    if tokio::runtime::Handle::try_current().is_ok() {
        // We're already in a Tokio runtime, so we can just spawn a task
        let (sender, receiver) = oneshot::channel::<()>();
        let worker = stop_manager.add_listener("socks5Server".into(), move || {
            let _ = sender.send(());
        })?;

        let enable_logging = enable_logging.clone();
        tokio::spawn(async move {
            let bind_addr = format!("0.0.0.0:{}", port);
            let listener = match TcpListener::bind(&bind_addr).await {
                Ok(listener) => {
                    if enable_logging {
                        log::info!("SOCKS5 server listening on {}", bind_addr);
                    }
                    listener
                },
                Err(e) => {
                    log::error!("Failed to bind SOCKS5 server to {}: {:?}", bind_addr, e);
                    return;
                }
            };

            if let Err(e) = run_socks5_server_with_listener(listener, enable_logging, receiver).await {
                log::error!("SOCKS5 server error: {:?}", e);
            }
            drop(worker);
        });
    } else {
        // We're not in a Tokio runtime, so we need to create one
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("socks5Server")
            .build()?;

        let (sender, receiver) = oneshot::channel::<()>();
        let worker = stop_manager.add_listener("socks5Server".into(), move || {
            let _ = sender.send(());
        })?;

        let enable_logging = enable_logging.clone();
        thread::Builder::new()
            .name("socks5Server".into())
            .spawn(move || {
                runtime.block_on(async {
                    let bind_addr = format!("0.0.0.0:{}", port);
                    let listener = match TcpListener::bind(&bind_addr).await {
                        Ok(listener) => {
                            if enable_logging {
                                log::info!("SOCKS5 server listening on {}", bind_addr);
                            }
                            listener
                        },
                        Err(e) => {
                            log::error!("Failed to bind SOCKS5 server to {}: {:?}", bind_addr, e);
                            return;
                        }
                    };

                    if let Err(e) = run_socks5_server_with_listener(listener, enable_logging, receiver).await {
                        log::error!("SOCKS5 server error: {:?}", e);
                    }
                });
                runtime.shutdown_background();
                drop(worker);
            })?;
    }

    Ok(())
}

async fn run_socks5_server_with_listener(
    listener: TcpListener,
    enable_logging: bool,
    mut shutdown_signal: oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, addr)) => {
                        if enable_logging {
                            log::info!("SOCKS5: New connection from {}", addr);
                        }

                        let enable_logging = enable_logging.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_client(stream, addr, enable_logging).await {
                                if enable_logging {
                                    log::warn!("SOCKS5: Error handling client {}: {:?}", addr, e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        log::error!("SOCKS5: Accept error: {:?}", e);
                    }
                }
            }
            _ = &mut shutdown_signal => {
                if enable_logging {
                    log::info!("SOCKS5: Server shutting down");
                }
                break;
            }
        }
    }

    Ok(())
}

async fn run_socks5_server(
    port: u16,
    enable_logging: bool,
    shutdown_signal: oneshot::Receiver<()>,
) -> anyhow::Result<()> {
    let bind_addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&bind_addr).await?;

    run_socks5_server_with_listener(listener, enable_logging, shutdown_signal).await
}

pub async fn handle_client(
    mut stream: TcpStream,
    client_addr: SocketAddr,
    enable_logging: bool,
) -> anyhow::Result<()> {
    // 1. Handle authentication negotiation
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await?;

    // Check SOCKS version
    if buf[0] != SOCKS_VERSION {
        return Err(anyhow::anyhow!("Unsupported SOCKS version: {}", buf[0]));
    }

    // Check authentication methods
    let nmethods = buf[1] as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods).await?;

    // We only support NO_AUTH for now
    if !methods.contains(&NO_AUTH) {
        // No acceptable auth methods
        stream.write_all(&[SOCKS_VERSION, AUTH_METHOD_NOT_ACCEPTABLE]).await?;
        return Err(anyhow::anyhow!("No supported authentication methods"));
    }

    // Send auth method choice (NO_AUTH)
    stream.write_all(&[SOCKS_VERSION, NO_AUTH]).await?;

    // 2. Handle client request
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;

    // Check SOCKS version again
    if header[0] != SOCKS_VERSION {
        return Err(anyhow::anyhow!("Unexpected SOCKS version: {}", header[0]));
    }

    // Check command
    let cmd = header[1];
    if cmd != CONNECT {
        // We only support CONNECT for now
        let mut response = [SOCKS_VERSION, COMMAND_NOT_SUPPORTED, 0, IPV4_ADDRESS, 0, 0, 0, 0, 0, 0];
        stream.write_all(&response).await?;
        return Err(anyhow::anyhow!("Unsupported command: {}", cmd));
    }

    // Parse address type
    let addr_type = header[3];
    let target_addr = match addr_type {
        IPV4_ADDRESS => {
            let mut addr_buf = [0u8; 4];
            stream.read_exact(&mut addr_buf).await?;
            let mut port_buf = [0u8; 2];
            stream.read_exact(&mut port_buf).await?;
            let port = u16::from_be_bytes(port_buf);

            format!("{}.{}.{}.{}:{}", addr_buf[0], addr_buf[1], addr_buf[2], addr_buf[3], port)
        },
        DOMAIN_NAME => {
            let mut len_buf = [0u8; 1];
            stream.read_exact(&mut len_buf).await?;
            let domain_len = len_buf[0] as usize;

            let mut domain_buf = vec![0u8; domain_len];
            stream.read_exact(&mut domain_buf).await?;

            let mut port_buf = [0u8; 2];
            stream.read_exact(&mut port_buf).await?;
            let port = u16::from_be_bytes(port_buf);

            let domain = String::from_utf8_lossy(&domain_buf).to_string();
            format!("{}:{}", domain, port)
        },
        IPV6_ADDRESS => {
            let mut addr_buf = [0u8; 16];
            stream.read_exact(&mut addr_buf).await?;
            let mut port_buf = [0u8; 2];
            stream.read_exact(&mut port_buf).await?;
            let port = u16::from_be_bytes(port_buf);

            // Format IPv6 address
            format!("[{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}]:{}",
                u16::from_be_bytes([addr_buf[0], addr_buf[1]]),
                u16::from_be_bytes([addr_buf[2], addr_buf[3]]),
                u16::from_be_bytes([addr_buf[4], addr_buf[5]]),
                u16::from_be_bytes([addr_buf[6], addr_buf[7]]),
                u16::from_be_bytes([addr_buf[8], addr_buf[9]]),
                u16::from_be_bytes([addr_buf[10], addr_buf[11]]),
                u16::from_be_bytes([addr_buf[12], addr_buf[13]]),
                u16::from_be_bytes([addr_buf[14], addr_buf[15]]),
                port
            )
        },
        _ => {
            // Unsupported address type
            let mut response = [SOCKS_VERSION, ADDRESS_TYPE_NOT_SUPPORTED, 0, IPV4_ADDRESS, 0, 0, 0, 0, 0, 0];
            stream.write_all(&response).await?;
            return Err(anyhow::anyhow!("Unsupported address type: {}", addr_type));
        }
    };

    if enable_logging {
        log::info!("SOCKS5: Connection request from {} to {}", client_addr, target_addr);
    }

    // 3. Connect to the target
    match TcpStream::connect(&target_addr).await {
        Ok(target_stream) => {
            // Send success response
            let bind_addr = target_stream.local_addr()?;
            let response = create_response(SUCCEEDED, &bind_addr);
            stream.write_all(&response).await?;

            // Start proxying data
            proxy_data(stream, target_stream).await?;

            if enable_logging {
                log::info!("SOCKS5: Connection closed: {} -> {}", client_addr, target_addr);
            }

            Ok(())
        },
        Err(e) => {
            // Send failure response
            let response = [SOCKS_VERSION, HOST_UNREACHABLE, 0, IPV4_ADDRESS, 0, 0, 0, 0, 0, 0];
            stream.write_all(&response).await?;

            Err(anyhow::anyhow!("Failed to connect to target: {}", e))
        }
    }
}

fn create_response(status: u8, addr: &SocketAddr) -> Vec<u8> {
    let mut response = vec![SOCKS_VERSION, status, 0];

    match addr {
        SocketAddr::V4(addr) => {
            response.push(IPV4_ADDRESS);
            response.extend_from_slice(&addr.ip().octets());
            response.extend_from_slice(&addr.port().to_be_bytes());
        },
        SocketAddr::V6(addr) => {
            response.push(IPV6_ADDRESS);
            response.extend_from_slice(&addr.ip().octets());
            response.extend_from_slice(&addr.port().to_be_bytes());
        }
    }

    response
}

async fn proxy_data(mut client: TcpStream, mut target: TcpStream) -> anyhow::Result<()> {
    let (mut client_read, mut client_write) = client.split();
    let (mut target_read, mut target_write) = target.split();

    let client_to_target = tokio::io::copy(&mut client_read, &mut target_write);
    let target_to_client = tokio::io::copy(&mut target_read, &mut client_write);

    tokio::select! {
        result = client_to_target => {
            if let Err(e) = result {
                return Err(anyhow::anyhow!("Error copying client to target: {}", e));
            }
        }
        result = target_to_client => {
            if let Err(e) = result {
                return Err(anyhow::anyhow!("Error copying target to client: {}", e));
            }
        }
    }

    Ok(())
}
