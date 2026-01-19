import socket
import time

s = socket.socket()
s.connect(("127.0.0.1", 8080))

parts = [
    b"GET / HT",
    b"TP/1.1\r",
    b"\nHost: exam",
    b"ple.com\r\n",
    b"\r\n"
]

for p in parts:
    s.sendall(p)
    time.sleep(1)

print(s.recv(20))
s.close()
