#!/usr/bin/env python3
"""A local Microsoft-shaped sign-in authority for checks/realcore-check.py --slice2b: clear HTTP on 127.0.0.1, nothing leaves
the Mac. The real core's rig copy reaches it through WHOOSH_CORE_CHECK_AUTHORITY=<port> and plays the browser itself (no
window is ever opened). Every tenant it serves is multi-tenant: the sign-in stays at `common`, as with the shipped default ID.

    GET  /<tenant>/v2.0/.well-known/openid-configuration   200 for TENANT, "organizations" and "common", 400 otherwise
    GET  /<tenant>/oauth2/v2.0/authorize?...              302 to the loopback redirect_uri with a code and the state
    POST /<tenant>/oauth2/v2.0/token                       authorization_code (PKCE S256 checked) or refresh_token; answers
                                                           realcore_mailfake.ACCESS_TOKEN and a refresh token

The log keeps the method, the path and the grant only: no query value, no client ID, no code, no token.

    python3 checks/realcore_authfake.py --selftest   drive it with urllib: the tenant answers, the redirect, the exchange
"""
import base64
import hashlib
import json
import os
import sys
import threading
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import realcore_mailfake as mailfake  # noqa: E402

TENANT = mailfake.ADDRESS.split("@")[1]
CODE = "check-code"
REFRESH = "check-refresh-1"
TENANTS = (TENANT, "organizations", "common")


def s256(verifier):
    return base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip("=")


class Authority(BaseHTTPRequestHandler):
    server_version = "whoosh-check-authority"

    def log_message(self, *_):   # the server's own access log would print the query; the check keeps `log` instead
        pass

    def answer(self, status, body=b"", headers=()):
        self.send_response(status)
        for k, v in headers:
            self.send_header(k, v)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def json(self, status, obj):
        self.answer(status, json.dumps(obj).encode(), [("Content-Type", "application/json")])

    def do_GET(self):
        u = urllib.parse.urlsplit(self.path)
        tenant = u.path.strip("/").split("/")[0]
        st = self.server.state
        if u.path.endswith("/v2.0/.well-known/openid-configuration"):
            st["log"].append("GET %s" % u.path)
            if tenant in TENANTS:
                return self.json(200, {"issuer": "http://127.0.0.1/%s/v2.0" % tenant})
            return self.json(400, {"error": "invalid_tenant", "error_description": "AADSTS90002: Tenant '%s' not found." % tenant})
        if u.path.endswith("/oauth2/v2.0/authorize"):
            st["log"].append("GET %s" % u.path)
            q = dict(urllib.parse.parse_qsl(u.query))
            if tenant not in TENANTS:
                return self.answer(400, b"AADSTS90002")
            st["challenge"][q.get("state", "")] = q.get("code_challenge", "")
            st["last_challenge"] = q.get("code_challenge", "")
            back = q.get("redirect_uri", "") + "?" + urllib.parse.urlencode({"code": CODE, "state": q.get("state", "")})
            return self.answer(302, b"", [("Location", back)])
        self.answer(404)

    def do_POST(self):
        u = urllib.parse.urlsplit(self.path)
        n = int(self.headers.get("Content-Length") or 0)
        f = dict(urllib.parse.parse_qsl(self.rfile.read(n).decode()))
        st = self.server.state
        grant = f.get("grant_type", "")
        st["log"].append("POST %s %s" % (u.path, grant))
        if not u.path.endswith("/oauth2/v2.0/token"):
            return self.answer(404)
        tokens = {"access_token": mailfake.ACCESS_TOKEN, "refresh_token": REFRESH, "expires_in": 3600, "token_type": "Bearer"}
        if grant == "authorization_code":
            if f.get("code") == CODE and s256(f.get("code_verifier", "")) == st.get("last_challenge"):
                return self.json(200, tokens)
            return self.json(400, {"error": "invalid_grant", "error_description": "AADSTS70008: the code or its PKCE verifier is wrong"})
        if grant == "refresh_token" and f.get("refresh_token") == REFRESH:
            return self.json(200, tokens)
        return self.json(400, {"error": "invalid_grant", "error_description": "AADSTS70000: unknown grant"})


def start(port=0):
    """The authority on 127.0.0.1 in a background thread. Returns (state, server); state["log"] is the request log."""
    srv = ThreadingHTTPServer(("127.0.0.1", port), Authority)
    srv.daemon_threads = True
    srv.state = {"log": [], "challenge": {}, "last_challenge": None}
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv.state, srv


def selftest():
    import urllib.error
    import urllib.request

    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *a, **k):
            return None

    st, srv = start()
    base = "http://127.0.0.1:%d" % srv.server_address[1]
    op = urllib.request.build_opener(NoRedirect)
    assert op.open(base + "/%s/v2.0/.well-known/openid-configuration" % TENANT).status == 200
    assert op.open(base + "/common/v2.0/.well-known/openid-configuration").status == 200
    try:
        op.open(base + "/elsewhere.test/v2.0/.well-known/openid-configuration")
        raise AssertionError("a domain that is not a tenant answered 200")
    except urllib.error.HTTPError as e:
        assert e.code == 400, e.code
    verifier = "v" * 43
    q = urllib.parse.urlencode({"client_id": "x", "redirect_uri": "http://127.0.0.1:9/", "state": "s1", "code_challenge": s256(verifier), "code_challenge_method": "S256"})
    try:
        op.open(base + "/%s/oauth2/v2.0/authorize?%s" % (TENANT, q))
        raise AssertionError("the sign-in page did not redirect")
    except urllib.error.HTTPError as e:
        loc = e.headers.get("Location")
        assert e.code == 302 and loc.startswith("http://127.0.0.1:9/?code=%s&state=s1" % CODE), (e.code, loc)

    def post(form):
        r = urllib.request.Request(base + "/%s/oauth2/v2.0/token" % TENANT, data=urllib.parse.urlencode(form).encode())
        try:
            return json.loads(op.open(r).read())
        except urllib.error.HTTPError as e:
            return json.loads(e.read())
    assert post({"grant_type": "authorization_code", "code": CODE, "code_verifier": verifier})["access_token"] == mailfake.ACCESS_TOKEN
    assert post({"grant_type": "authorization_code", "code": CODE, "code_verifier": "w" * 43}).get("error") == "invalid_grant"
    assert post({"grant_type": "refresh_token", "refresh_token": REFRESH})["refresh_token"] == REFRESH
    assert not any("?" in l or CODE in l for l in st["log"]), st["log"]
    srv.shutdown()
    print("selftest: the local authority answers the tenant, the redirect and the PKCE exchange")
    return 0


if __name__ == "__main__":
    sys.exit(selftest() if "--selftest" in sys.argv else 2)
