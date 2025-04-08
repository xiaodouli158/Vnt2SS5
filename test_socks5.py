import socket
import socks
import requests

def test_socks5_proxy():
    # Configure SOCKS5 proxy
    socks.set_default_proxy(socks.SOCKS5, "127.0.0.1", 10080)
    socket.socket = socks.socksocket
    
    try:
        # Make a request through the proxy
        response = requests.get("http://httpbin.org/ip")
        print("Response status code:", response.status_code)
        print("Response body:", response.text)
        print("SOCKS5 proxy test successful!")
    except Exception as e:
        print("Error:", e)
        print("SOCKS5 proxy test failed!")

if __name__ == "__main__":
    test_socks5_proxy()
