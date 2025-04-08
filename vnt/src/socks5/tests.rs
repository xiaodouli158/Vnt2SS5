#[cfg(test)]
mod tests {
    use crate::socks5::*;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn test_socks5_connect() {
        // Start a mock target server
        let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = target_listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut socket, _) = target_listener.accept().await.unwrap();
            let mut buf = [0; 1024];
            let n = socket.read(&mut buf).await.unwrap();
            assert_eq!(&buf[..n], b"hello from client");
            socket.write_all(b"hello from server").await.unwrap();
        });

        // Start a SOCKS5 server
        let socks5_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let socks5_addr = socks5_listener.local_addr().unwrap();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        tokio::spawn(async move {
            tokio::select! {
                accept_result = socks5_listener.accept() => {
                    match accept_result {
                        Ok((stream, addr)) => {
                            tokio::spawn(async move {
                                let _ = handle_client(stream, addr, true).await;
                            });
                        }
                        Err(_) => {}
                    }
                }
                _ = shutdown_rx => {}
            }
        });

        // Connect to the SOCKS5 server
        let mut client = TcpStream::connect(socks5_addr).await.unwrap();

        // SOCKS5 handshake
        // Send version and auth methods
        client.write_all(&[SOCKS_VERSION, 1, NO_AUTH]).await.unwrap();

        // Receive auth method choice
        let mut response = [0u8; 2];
        client.read_exact(&mut response).await.unwrap();
        assert_eq!(response, [SOCKS_VERSION, NO_AUTH]);

        // Send connect request to target
        let mut request = vec![
            SOCKS_VERSION, CONNECT, 0, IPV4_ADDRESS,
        ];

        // Handle IPv4 address
        if let std::net::IpAddr::V4(ipv4) = target_addr.ip() {
            request.extend_from_slice(&ipv4.octets());
        } else {
            panic!("Expected IPv4 address");
        }

        request.extend_from_slice(&target_addr.port().to_be_bytes());

        client.write_all(&request).await.unwrap();

        // Receive connect response
        let mut header = [0u8; 4];
        client.read_exact(&mut header).await.unwrap();
        assert_eq!(header[0], SOCKS_VERSION);
        assert_eq!(header[1], SUCCEEDED);

        // Skip the rest of the response
        let addr_type = header[3];
        match addr_type {
            IPV4_ADDRESS => {
                let mut addr_buf = [0u8; 6]; // 4 for IPv4 + 2 for port
                client.read_exact(&mut addr_buf).await.unwrap();
            },
            IPV6_ADDRESS => {
                let mut addr_buf = [0u8; 18]; // 16 for IPv6 + 2 for port
                client.read_exact(&mut addr_buf).await.unwrap();
            },
            _ => panic!("Unexpected address type: {}", addr_type),
        }

        // Send data to target through SOCKS5
        client.write_all(b"hello from client").await.unwrap();

        // Receive data from target through SOCKS5
        let mut buf = [0; 1024];
        let n = client.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"hello from server");

        // Cleanup
        let _ = shutdown_tx.send(());
    }
}
