import socket

UDP_IP = "10.226.227.18"
UDP_PORT = 4210
OPS = ["a", "b", "c"]
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

def send_op(btn_id):
    if btn_id in OPS:
        sock.sendto(bytearray([99, ord(btn_id[0])]), (UDP_IP, UDP_PORT))
        return True
    return False

