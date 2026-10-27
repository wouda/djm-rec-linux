import os
import json
import urllib.request

token = os.environ.get("GH_TOKEN", "")
repo = os.environ.get("GITHUB_REPOSITORY", "wouda/djm-rec-linux")
sha = os.environ.get("GITHUB_SHA", "")

log_path = "/tmp/flutter_build.log"
if os.path.exists(log_path):
    with open(log_path) as f:
        lines = f.readlines()
    log_tail = "".join(lines[-150:])
else:
    log_tail = "No /tmp/flutter_build.log found"

body = f"### Flutter Build Failure Log\n```\n{log_tail}\n```"
data = json.dumps({"body": body}).encode("utf-8")
url = f"https://api.github.com/repos/{repo}/commits/{sha}/comments"

req = urllib.request.Request(
    url,
    data=data,
    headers={
        "Authorization": f"token {token}",
        "User-Agent": "CI-Logger",
        "Content-Type": "application/json",
    },
)

try:
    with urllib.request.urlopen(req) as resp:
        print(f"Successfully posted failure log as commit comment (status {resp.status})")
except Exception as e:
    print(f"Failed to post commit comment: {e}")
