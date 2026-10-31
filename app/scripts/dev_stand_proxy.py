#!/usr/bin/env python3
"""The local dev stand's persona proxy (app/scripts/dev_stand.sh).

The stand's session cookie is a `__Host-` cookie, which a browser never stores
from plain http, so a browser on the stand would be signed out. This proxy
listens on loopback only, adds one persona's session cookie to every request
and forwards it to the stand (also loopback only), streaming the answer (the
canvas's `/v1/events` stream included).

  dev_stand_proxy.py <stand.json> <persona> <port> [--host 127.0.0.1] [--clear-on-reset]

With --clear-on-reset (the fresh stand, app/scripts/fresh_stand.sh), the first
page a browser loads after the stand restarts carries `Clear-Site-Data:
"storage"`, so the canvas this origin kept in the browser (local storage,
the service worker) is cleared and a reset starts from an empty canvas.

The stand file (mode 0600, written by the stand) names the origin and each
persona's session; it is re-read when it changes (the stand mints a session
again after a sign-out). Nothing here prints a session value. Scratch tooling
for a local browser, never deployed.
"""
import argparse
import http.client
import http.server
import ipaddress
import json
import os
import socket
import sys
import threading
import urllib.parse

LOOPBACK_NAMES = {"localhost"}
EPOCH_COOKIE = "forma_stand_epoch"
# Each browser gets its own session of the persona (the stand mints several):
# one live stream per session, so two browsers on one session replaced each
# other's stream. The browser is told apart by this cookie, set on its first visit.
CLIENT_COOKIE = "forma_stand_client"
HOP_BY_HOP = {
    "connection", "keep-alive", "proxy-authenticate", "proxy-authorization", "te", "trailers",
    "transfer-encoding", "upgrade", "content-length", "host",
}


def loopback(host):
    if host in LOOPBACK_NAMES:
        return True
    try:
        return ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


class Stand:
    """The stand file, re-read when its modification time changes."""

    def __init__(self, path, persona):
        self.path, self.persona = path, persona
        self.lock = threading.Lock()
        self.mtime = None
        self.load()

    def load(self):
        with self.lock:
            mtime = os.stat(self.path).st_mtime_ns
            if mtime == self.mtime:
                return self.current
            with open(self.path) as handle:
                stand = json.load(handle)
            target = urllib.parse.urlparse(stand["origin"])
            if target.scheme != "http" or not loopback(target.hostname or ""):
                raise SystemExit("the stand origin must be http on loopback")
            persona = stand["personas"][self.persona]
            # A new stand process is a new epoch (the browser's storage is cleared once per epoch).
            self.epoch = "%s-%s" % (stand.get("pid", 0), target.port)
            self.sessions = persona.get("sessions") or [persona["session"]]
            self.current = (target, stand["cookie"], persona["session"], persona["name"])
            self.mtime = mtime
            return self.current


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    stand = None  # set in main
    clear_on_reset = False

    clients = {}
    clients_lock = threading.Lock()

    def browser(self):
        """This browser's id (new if it has none) and whether it is new."""
        for c in (self.headers.get("cookie") or "").split(";"):
            c = c.strip()
            if c.startswith(CLIENT_COOKIE + "="):
                value = c[len(CLIENT_COOKIE) + 1:]
                if value.isalnum() and len(value) <= 64:
                    return value, False
        return os.urandom(12).hex(), True

    def session_for(self, client, new_client, default):
        """The browser's own session. A browser takes the next one on its first
        page load (when it gets its cookie); requests before that (or from a
        tool like curl) share the last one, so they never use up a browser's."""
        sessions = self.stand.sessions
        if not sessions:
            return default
        page = self.command == "GET" and "text/html" in (self.headers.get("accept") or "")
        with self.clients_lock:
            slot = Handler.clients.get(client)
            if slot is None and page:
                slot = Handler.clients[client] = len(Handler.clients) % max(1, len(sessions) - 1)
        return sessions[slot] if slot is not None else sessions[-1]

    def stale_page(self):
        """A page load from a browser that last saw an earlier stand."""
        if not self.clear_on_reset or self.command != "GET":
            return False
        if self.headers.get("sec-fetch-dest", "document") != "document" or "text/html" not in (self.headers.get("accept") or ""):
            return False
        seen = [c.strip()[len(EPOCH_COOKIE) + 1:] for c in (self.headers.get("cookie") or "").split(";") if c.strip().startswith(EPOCH_COOKIE + "=")]
        return seen != [self.stand.epoch]

    def forward(self):
        try:
            target, name, session, _ = self.stand.load()
        except (OSError, ValueError, KeyError):
            # The stand is restarting (its file is rewritten when it is up).
            self.send_error(503, "dev stand restarting")
            return
        length = int(self.headers.get("content-length") or 0)
        body = self.rfile.read(length) if length else None
        headers = {}
        for key, value in self.headers.items():
            lower = key.lower()
            if lower in HOP_BY_HOP or lower == "cookie":
                continue
            headers[key] = value
        # The browser's own cookies (never a session cookie), then the persona's.
        kept = [
            c.strip() for c in (self.headers.get("cookie") or "").split(";")
            if c.strip() and not c.strip().startswith(name + "=")
        ]
        client, new_client = self.browser()
        session = self.session_for(client, new_client, session)
        headers["Cookie"] = "; ".join(kept + ["%s=%s" % (name, session)])
        headers["Host"] = target.netloc
        if self.headers.get("origin"):
            headers["Origin"] = "%s://%s" % (target.scheme, target.netloc)
        if self.headers.get("referer"):
            parsed = urllib.parse.urlparse(self.headers["referer"])
            headers["Referer"] = urllib.parse.urlunparse(parsed._replace(scheme=target.scheme, netloc=target.netloc))
        stream = self.path.startswith("/v1/events")
        conn = http.client.HTTPConnection(target.hostname, target.port, timeout=None if stream else 600)
        try:
            conn.request(self.command, self.path, body=body, headers=headers)
            response = conn.getresponse()
        except OSError:
            self.send_error(502, "dev stand unreachable")
            return
        self.send_response(response.status)
        if new_client:
            self.send_header("Set-Cookie", "%s=%s; Path=/; SameSite=Lax; HttpOnly" % (CLIENT_COOKIE, client))
        if self.stale_page():
            self.send_header("Clear-Site-Data", '"storage"')
            self.send_header("Set-Cookie", "%s=%s; Path=/; SameSite=Lax" % (EPOCH_COOKIE, self.stand.epoch))
        sized = response.getheader("content-length")
        for key, value in response.getheaders():
            if key.lower() in HOP_BY_HOP:
                continue
            self.send_header(key, value)
        if self.command == "HEAD" or response.status in (204, 304):
            self.send_header("Content-Length", "0" if self.command != "HEAD" else (sized or "0"))
            self.end_headers()
            conn.close()
            return
        if sized is not None:
            data = response.read()
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            conn.close()
            return
        # No length (an event stream, a chunked answer): relay chunks as they come.
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()
        try:
            while True:
                chunk = response.read1(65536)
                if not chunk:
                    break
                self.wfile.write(b"%x\r\n%s\r\n" % (len(chunk), chunk))
                self.wfile.flush()
            self.wfile.write(b"0\r\n\r\n")
        except (BrokenPipeError, ConnectionResetError, OSError):
            self.close_connection = True
        finally:
            conn.close()

    do_GET = do_POST = do_PUT = do_PATCH = do_DELETE = do_HEAD = do_OPTIONS = forward

    def log_message(self, *args):
        pass


class Server(http.server.ThreadingHTTPServer):
    daemon_threads = True


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("stand")
    parser.add_argument("persona")
    parser.add_argument("port", type=int)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--clear-on-reset", action="store_true")
    args = parser.parse_args()
    Handler.clear_on_reset = args.clear_on_reset
    if not loopback(args.host):
        raise SystemExit("refused: the proxy binds loopback only (127.0.0.1, ::1, localhost)")
    Handler.stand = Stand(args.stand, args.persona)
    Server.address_family = socket.AF_INET6 if ":" in args.host else socket.AF_INET
    server = Server((args.host, args.port), Handler)
    name = Handler.stand.current[3]
    print("DEV_STAND_PROXY %s http://%s:%d" % (name, args.host, args.port), flush=True)
    server.serve_forever()


if __name__ == "__main__":
    sys.exit(main())
