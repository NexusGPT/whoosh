#!/usr/bin/env python3
"""A local network for the account detector: a DNS server (UDP and TCP, wire format written by hand) and an HTTP server that answer, for a few
reserved `.test` domains, what Microsoft 365, Google, a provider database, an RFC 6186 domain, a domain's own Autodiscover and a plain IMAP host answer.
Nothing here is a live capture: the documents are the repository's own hand-written and recorded fixtures (fixtures/autoconfig/, README there), the DNS
answers are written from the public record formats (MX, SRV, SOA), and the Microsoft answers from Microsoft's published Autodiscover documents.

WHY (lane perf-realcore-r1, round 3): checks/realcore-check.py slice 1 detected assem@nexus-example.com through PUBLIC DNS and HTTPS from a hidden test copy. That broke
the owner's rule "nothing leaves this Mac except the relay". The process-wide guard (core/crates/netguard) now refuses it, rightly. The detector's LOGIC is
judged here on recorded answers, on this Mac only.

THE SEAM THE BUILD MUST CARRY (named, not built here; owner: lane engine-act-r6, which owns netguard and the autoconfig adapters, with the rust_core.rs wiring line):
    WHOOSH_CORE_CHECK_DETECT=<dns port>:<http port>      rig only (the guard closed), read once at open, beside WHOOSH_CORE_CHECK_SERVERS
  - DnsPort: `HickoryDns::with_nameserver(127.0.0.1:<dns port>)` (it exists, adapters/dns_hickory.rs) instead of the system resolver.
  - HttpPort: every request `https://<host>/<path>?<query>` is sent as `http://127.0.0.1:<http port>/<path>?<query>` with `Host: <host>`, method and body kept
    (ReqwestHttp without `https_only`, a rig-only constructor); a redirect still stops (netguard).
  - ProbePort (step e, the guess): for a host under `.test`, a probe of imap.<d> connects in clear text to the IMAP port of WHOOSH_CORE_CHECK_SERVERS and a
    probe of smtp.<d> to its SMTP port, whatever the port and security asked; mail.<d> and every other host refuse. (plain-check.test below needs this.)
  The core's own netguard stays closed: the loopback is the only place any of these goes.

    python3 checks/realcore_netfake.py --selftest        drive both servers (dns by `dig`, an independent decoder; http by urllib)
    python3 checks/realcore_netfake.py --serve           serve until interrupted, printing the two ports
"""
import http.server
import json
import os
import re
import socket
import socketserver
import struct
import subprocess
import sys
import threading

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIX = os.path.join(ROOT, "fixtures", "autoconfig")

# the domains the check types (reserved: RFC 6761 `.test`), and what each one is
M365 = "m365-check.test"            # a Microsoft 365 tenant: Autodiscover's shared front says Exchange Online (step d), and its MX is a tenant's
M365_MX = "m365-mx-check.test"      # a tenant whose Autodiscover front does not answer: the mail record says Microsoft 365 (step e)
GOOGLE = "google-check.test"        # MX at Google: the mail record names Google's servers
ISPDB = "ispdb-check.test"          # the provider database knows it (a recorded Google answer)
SRV = "srv-check.test"              # RFC 6186 SRV records and nothing else (fixtures/autoconfig/srv-domain.txt, hand-written)
POX = "pox-check.test"              # its own Autodiscover server answers a POX document (fixtures/autoconfig/autodiscover-pox.xml)
PLAIN = "plain-check.test"          # nothing answers but the guess: imap. and smtp. probes (needs the seam's probe mapping)
MISSING = "nx-check.test"           # a name that does not exist at all: NXDOMAIN
TENANT_MX = "m365checktest.mail.protection.outlook.com."
ZONES = {
    M365: {"mx": [(0, TENANT_MX)]},
    M365_MX: {"mx": [(0, "m365mxchecktest.mail.protection.outlook.com.")]},
    GOOGLE: {"mx": [(1, "aspmx.l.google.com."), (5, "alt1.aspmx.l.google.com.")]},
    ISPDB: {}, POX: {}, PLAIN: {},
    SRV: {"srv": {"_imaps._tcp.%s" % SRV: [(10, 0, 993, "backup-imap.%s." % SRV), (0, 1, 993, "imap.%s." % SRV)],
                  "_submission._tcp.%s" % SRV: [(0, 1, 587, "smtp.%s." % SRV)]}},
}

def bundle_has(bundle, needle):
    """True when `needle` is a string literal in the bundle's executables: how the build's seams are told without launching it."""
    needle = needle.encode() if isinstance(needle, str) else needle
    for sub in ("Contents/MacOS", "Contents/Frameworks"):
        for root, _d, files in os.walk(os.path.join(bundle, sub)):
            for f in files:
                try:
                    with open(os.path.join(root, f), "rb") as fh:
                        tail = b""
                        while True:
                            chunk = fh.read(8 << 20)
                            if not chunk:
                                break
                            if needle in tail + chunk:
                                return True
                            tail = chunk[-len(needle):]
                except OSError:
                    continue
    return False


# ------------------------------------------------------------------------------------------------ DNS
T_A, T_SOA, T_MX, T_SRV, T_ANY = 1, 6, 15, 33, 255


def enc_name(name):
    out = b""
    for label in name.rstrip(".").split("."):
        if label:
            out += bytes([len(label)]) + label.encode("ascii")
    return out + b"\0"


def dec_name(data, i):
    labels, jumped, end = [], False, 0
    while True:
        n = data[i]
        if n == 0:
            i += 1
            break
        if n & 0xC0 == 0xC0:
            ptr = ((n & 0x3F) << 8) | data[i + 1]
            if not jumped:
                end = i + 2
            i, jumped = ptr, True
            continue
        labels.append(data[i + 1:i + 1 + n].decode("ascii").lower())
        i += 1 + n
    return ".".join(labels), (end if jumped else i)


def rr(rtype, rdata, ttl=60):
    return b"\xc0\x0c" + struct.pack(">HHIH", rtype, 1, ttl, len(rdata)) + rdata


class Dns:
    """The zone table and the answers. `log` lists (type, name) of every question, for the check to read."""
    def __init__(self, zones=None):
        self.zones, self.log, self.lock = zones if zones is not None else ZONES, [], threading.Lock()

    def zone_of(self, name):
        for z in self.zones:
            if name == z or name.endswith("." + z):
                return z
        return None

    def answer(self, query):
        """(response bytes) for one query message, or None when it is not a plain query."""
        if len(query) < 12:
            return None
        qid, flags, qd = struct.unpack(">HHH", query[:6])
        if flags & 0x8000 or qd != 1:
            return None
        name, i = dec_name(query, 12)
        qtype, _qclass = struct.unpack(">HH", query[i:i + 4])
        question = query[12:i + 4]
        with self.lock:
            self.log.append((qtype, name))
        zone = self.zone_of(name)
        rcode, answers = 0, []
        if zone is None:
            rcode = 3                                           # NXDOMAIN: the name is in no zone this fake holds
        else:
            z = self.zones[zone]
            if qtype == T_SOA and name == zone:
                answers = [rr(T_SOA, enc_name("ns.%s" % zone) + enc_name("hostmaster.%s" % zone) + struct.pack(">IIIII", 1, 3600, 600, 86400, 60))]
            elif qtype == T_MX and name == zone:
                answers = [rr(T_MX, struct.pack(">H", pref) + enc_name(ex)) for pref, ex in z.get("mx", [])]
            elif qtype == T_SRV:
                answers = [rr(T_SRV, struct.pack(">HHH", pr, w, port) + enc_name(t)) for pr, w, port, t in z.get("srv", {}).get(name, [])]
            elif qtype == T_A and name in (zone, "imap." + zone, "smtp." + zone, "mail." + zone):
                answers = [rr(T_A, socket.inet_aton("127.0.0.1"))]
        rflags = 0x8400 | (flags & 0x0100) | 0x0080 | rcode      # QR, AA, RD as asked, RA, the rcode
        return struct.pack(">HHHHHH", qid, rflags, 1, len(answers), 0, 0) + question + b"".join(answers)


class UdpHandler(socketserver.BaseRequestHandler):
    def handle(self):
        data, sock = self.request
        reply = self.server.dns.answer(data)
        if reply is not None:
            sock.sendto(reply, self.client_address)


class TcpHandler(socketserver.BaseRequestHandler):
    def handle(self):
        while True:
            head = self.request.recv(2)
            if len(head) < 2:
                return
            n = struct.unpack(">H", head)[0]
            data = b""
            while len(data) < n:
                chunk = self.request.recv(n - len(data))
                if not chunk:
                    return
                data += chunk
            reply = self.server.dns.answer(data)
            if reply is None:
                return
            self.request.sendall(struct.pack(">H", len(reply)) + reply)


class UdpServer(socketserver.ThreadingUDPServer):
    allow_reuse_address = True
    daemon_threads = True


class TcpServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


# ------------------------------------------------------------------------------------------------ HTTP
def read_fixture(name):
    with open(os.path.join(FIX, name), encoding="utf-8") as f:
        return f.read()


MS_FRONT = "autodiscover-s.outlook.com"
ISPDB_HOST = "autoconfig.thunderbird.net"


class Http(http.server.BaseHTTPRequestHandler):
    """Routes by the Host header the core keeps (the seam sends https://<host>/<path> as http://127.0.0.1:<port>/<path>)."""
    def log_message(self, *a):
        pass

    def reply(self, code, body="", ctype="application/json", extra=()):
        raw = body.encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(raw)))
        for k, v in extra:
            self.send_header(k, v)
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(raw)

    def note(self, method, body=b""):
        host = (self.headers.get("Host") or "").split(":")[0].lower()
        with self.server.lock:
            self.server.log.append((method, host, self.path))
        return host

    def do_GET(self):
        host = self.note("GET")
        path, _, query = self.path.partition("?")
        if host == ISPDB_HOST and path.startswith("/v1.1/"):
            d = path[len("/v1.1/"):]
            if d == ISPDB:      # recorded Google answer (fixtures/autoconfig/ispdb-gmail.com.xml), served for the check domain
                return self.reply(200, read_fixture("ispdb-gmail.com.xml"), "text/xml")
            return self.reply(404, "<html><body>404 Not Found</body></html>", "text/html")
        if host == MS_FRONT and path == "/autodiscover/autodiscover.json":
            m = re.search(r"Email=[^&@]*(?:@|%40)([^&]+)", query)
            d = m.group(1).lower() if m else ""
            if d == M365:    # a tenant: Exchange Online (fixtures/autoconfig/autodiscover-m365.json, hand-written from Microsoft's Autodiscover v2 document)
                return self.reply(200, read_fixture("autodiscover-m365.json"))
            # not a tenant of this front: an error document (my reading of Microsoft's published error shape, never a live capture)
            return self.reply(400, json.dumps({"ErrorCode": "InvalidRequest", "ErrorMessage": "The email address is not a tenant of this service."}))
        return self.reply(404, "not found", "text/plain")       # autoconfig.<d>, <d>/.well-known: nothing published

    def do_POST(self):
        host = self.note("POST")
        n = int(self.headers.get("Content-Length") or 0)
        if n:
            self.rfile.read(n)
        if host == "autodiscover." + POX and self.path.startswith("/autodiscover/autodiscover.xml"):
            return self.reply(200, read_fixture("autodiscover-pox.xml"), "text/xml")
        if host == "autodiscover." + M365 or host == "autodiscover." + M365_MX:
            # a tenant's own name is a CNAME of Exchange Online's front, which wants credentials for a POX request
            return self.reply(401, "", "text/plain", [("WWW-Authenticate", "Basic realm=\"\"")])
        return self.reply(404, "not found", "text/plain")


class HttpServer(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


def start(dns_port=0, http_port=0):
    """Both servers on 127.0.0.1 (DNS on UDP and TCP at the same port) in background threads. Returns (dns, http, stop): dns.log and http.log record
    every question; `stop()` shuts everything. Raises OSError when a port cannot be bound."""
    dns = Dns()
    udp = UdpServer(("127.0.0.1", dns_port), UdpHandler)
    port = udp.server_address[1]
    tcp = TcpServer(("127.0.0.1", port), TcpHandler)
    udp.dns = tcp.dns = dns
    web = HttpServer(("127.0.0.1", http_port), Http)
    web.log, web.lock = [], threading.Lock()
    for s in (udp, tcp, web):
        threading.Thread(target=s.serve_forever, daemon=True).start()

    def stop():
        for s in (udp, tcp, web):
            s.shutdown(); s.server_close()
    dns.port, web.dns_port = port, port
    return dns, web, stop


# ------------------------------------------------------------------------------------------------ selftest
def dig(port, name, rtype, tcp=False):
    p = subprocess.run(["dig", "@127.0.0.1", "-p", str(port), name, rtype, "+short", "+tries=1", "+time=3"] + (["+tcp"] if tcp else []), capture_output=True, text=True)
    return p.stdout.split("\n") if p.returncode == 0 else ["dig exit %d: %s" % (p.returncode, p.stderr[:80])]


def dig_status(port, name, rtype):
    p = subprocess.run(["dig", "@127.0.0.1", "-p", str(port), name, rtype, "+tries=1", "+time=3"], capture_output=True, text=True)
    m = re.search(r"status: (\w+)", p.stdout)
    return m.group(1) if m else "?"


def selftest():
    import urllib.request
    import urllib.error
    dns, web, stop = start()
    port, hp = dns.port, web.server_address[1]
    res = []

    def get(host, path, method="GET", body=None):
        req = urllib.request.Request("http://127.0.0.1:%d%s" % (hp, path), data=body, method=method, headers={"Host": host})
        try:
            with urllib.request.urlopen(req, timeout=5) as r:
                return r.status, r.read().decode()
        except urllib.error.HTTPError as e:
            return e.code, e.read().decode()

    try:
        for label, got, want in [
            ("MX of the Microsoft 365 tenant, over UDP", [x for x in dig(port, M365, "MX") if x], ["0 %s" % TENANT_MX]),
            ("MX of the Microsoft 365 tenant, over TCP", [x for x in dig(port, M365, "MX", tcp=True) if x], ["0 %s" % TENANT_MX]),
            ("MX of Google, both exchangers", sorted(x for x in dig(port, GOOGLE, "MX") if x), ["1 aspmx.l.google.com.", "5 alt1.aspmx.l.google.com."]),
            ("SRV _imaps._tcp, both records", sorted(x for x in dig(port, "_imaps._tcp." + SRV, "SRV") if x), ["0 1 993 imap.%s." % SRV, "10 0 993 backup-imap.%s." % SRV]),
            ("SRV _submission._tcp", [x for x in dig(port, "_submission._tcp." + SRV, "SRV") if x], ["0 1 587 smtp.%s." % SRV]),
            ("A of imap.<plain>", [x for x in dig(port, "imap." + PLAIN, "A") if x], ["127.0.0.1"]),
            ("SOA of an existing domain exists (NOERROR)", dig_status(port, M365, "SOA"), "NOERROR"),
            ("SOA of an unknown name is NXDOMAIN", dig_status(port, MISSING, "SOA"), "NXDOMAIN"),
            ("SRV of a domain with none is NOERROR with no record", (dig_status(port, "_imaps._tcp." + GOOGLE, "SRV"), [x for x in dig(port, "_imaps._tcp." + GOOGLE, "SRV") if x]), ("NOERROR", [])),
            ("MX of a name outside every zone is NXDOMAIN (control: the zone's own MX answered above)", dig_status(port, "example.org", "MX"), "NXDOMAIN"),
        ]:
            res.append((label, got == want, "got %r want %r" % (got, want)))
        for label, got, want in [
            ("ISPDB answers the check domain with the recorded Google file", get(ISPDB_HOST, "/v1.1/" + ISPDB)[0], 200),
            ("ISPDB has no entry for another domain (404)", get(ISPDB_HOST, "/v1.1/" + GOOGLE)[0], 404),
            ("the Microsoft front says Exchange Online for the tenant", json.loads(get(MS_FRONT, "/autodiscover/autodiscover.json?Email=a@%s&Protocol=AutodiscoverV1" % M365)[1]).get("Url"), "https://outlook.office365.com/autodiscover/autodiscover.xml"),
            ("the Microsoft front refuses a domain that is no tenant (400)", get(MS_FRONT, "/autodiscover/autodiscover.json?Email=a@%s&Protocol=AutodiscoverV1" % GOOGLE)[0], 400),
            ("the tenant's own Autodiscover wants credentials (401)", get("autodiscover." + M365, "/autodiscover/autodiscover.xml", "POST", b"<x/>")[0], 401),
            ("a domain's own Autodiscover answers a POX document", "mail.onprem.example" in get("autodiscover." + POX, "/autodiscover/autodiscover.xml", "POST", b"<x/>")[1], True),
            ("autoconfig.<d> publishes nothing (404)", get("autoconfig." + GOOGLE, "/mail/config-v1.1.xml?emailaddress=a@%s" % GOOGLE)[0], 404),
        ]:
            res.append((label, got == want, "got %r want %r" % (got, want)))
        res.append(("the DNS log records what was asked (MX and SOA seen)", (T_MX, M365) in dns.log and (T_SOA, M365) in dns.log, str(dns.log[:3])))
        res.append(("the HTTP log records the Host the core keeps", ("GET", ISPDB_HOST, "/v1.1/" + ISPDB) in web.log, str(web.log[:2])))
    finally:
        stop()
    bad = [r for r in res if not r[1]]
    for label, ok, detail in res:
        print("%-4s %s%s" % ("ok" if ok else "BAD", label, "" if ok else "  [%s]" % detail))
    print("%d checks, %d failed, 0 not judged" % (len(res), len(bad)))
    return 1 if bad else 0


def serve():
    dns, web, _stop = start()
    print("dns %d http %d" % (dns.port, web.server_address[1]), flush=True)
    try:
        threading.Event().wait()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else serve() if "--serve" in sys.argv else 2)
