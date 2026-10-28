#!/usr/bin/env python3
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


MODEL = "pennel-qa"


class Handler(BaseHTTPRequestHandler):
    requests = 0

    def log_message(self, format, *args):
        pass

    def respond(self, status, value):
        body = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def record(self, operation):
        type(self).requests += 1
        print(json.dumps({"request": type(self).requests, "operation": operation}), flush=True)

    def do_GET(self):
        if self.path != "/v1/models":
            self.respond(404, {"error": "unsupported test route"})
            return
        self.record("models")
        self.respond(200, {"data": [{"id": MODEL, "object": "model", "owned_by": "local-test"}]})

    def do_POST(self):
        if self.path != "/v1/chat/completions":
            self.respond(404, {"error": "unsupported test route"})
            return
        size = int(self.headers.get("Content-Length", "0"))
        if size > 1_048_576:
            self.respond(413, {"error": "test input limit"})
            return
        try:
            request = json.loads(self.rfile.read(size))
        except (ValueError, UnicodeError):
            self.respond(400, {"error": "invalid test input"})
            return
        if request.get("model") != MODEL:
            self.respond(400, {"error": "use the controlled test model"})
            return
        system = " ".join(str(item.get("content", "")) for item in request.get("messages", []) if item.get("role") == "system")
        ask = "numbered sources" in system
        self.record("ask" if ask else "summary-or-verification")
        content = "See the controlled synthetic source. [1]" if ask else "## Summary\n\nA controlled interface test completed.\n\n## Next step\n\n- Keep the test data local."
        self.respond(200, {"id": "qa-controlled-response", "object": "chat.completion", "model": MODEL, "choices": [{"index": 0, "message": {"role": "assistant", "content": content}, "finish_reason": "stop"}], "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}})


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
print(json.dumps({"origin": f"http://127.0.0.1:{server.server_port}/v1", "model": MODEL}), flush=True)
try:
    server.serve_forever()
except KeyboardInterrupt:
    pass
finally:
    server.server_close()
