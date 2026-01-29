import sys
from server import start_server
from looper import start_loop

debug = "--debug" in sys.argv or "-d" in sys.argv

start_loop()
start_server(debug=debug)
