#!/usr/bin/env python3
import subprocess
import sys

REPO = "ildar-safarov/gutenindex"
CORPUS_TAG = "corpus-v1"
INDEX_TAG = "index-v1"
USER = "root"
BASE = "/opt/gutenindex"
BINARY = "/usr/local/bin/gutenindex-web"
SUPERVISOR_CONF = "/etc/supervisor/conf.d/gutenindex-web.conf"
PORT = 3000

CORPUS_FILES = [f"{i}.zip" for i in range(10)]
INDEX_FILES = ["vocab"] + [str(i) for i in range(8)] + ["doclen", "meta"]
SSH_OPTS = ["-o", "StrictHostKeyChecking=accept-new"]


def sh(cmd):
    print(f"  $ {' '.join(cmd)}")
    subprocess.run(cmd, check=True)


def ssh(host, cmd):
    print(f"  remote: {cmd}")
    subprocess.run(["ssh"] + SSH_OPTS + [f"{USER}@{host}", cmd], check=True)


def scp(host, local, remote):
    subprocess.run(["scp"] + SSH_OPTS + [local, f"{USER}@{host}:{remote}"], check=True)


def pipe_to(host, path, content):
    subprocess.run(
        ["ssh"] + SSH_OPTS + [f"{USER}@{host}", f"tee {path} > /dev/null"],
        input=content.encode(),
        check=True,
    )


def gh_url(tag, filename):
    return f"https://github.com/{REPO}/releases/download/{tag}/{filename}"


def main():
    if len(sys.argv) != 2:
        sys.exit(f"usage: {sys.argv[0]} <host-ip>")
    host = sys.argv[1]

    print("==> 1/6  build x86_64 Linux binary")
    if subprocess.run(["which", "zig"], capture_output=True).returncode != 0:
        sys.exit("zig not found — run: brew install zig")
    if subprocess.run(["which", "cargo-zigbuild"], capture_output=True).returncode != 0:
        sh(["cargo", "install", "cargo-zigbuild"])
    sh(["rustup", "target", "add", "x86_64-unknown-linux-gnu"])
    sh(["cargo", "zigbuild", "--target", "x86_64-unknown-linux-gnu.2.17", "--release", "-p", "web"])

    print("==> 2/6  upload binary")
    scp(host, "target/x86_64-unknown-linux-gnu/release/web", BINARY)
    ssh(host, f"chmod +x {BINARY}")

    print("==> 3/6  install packages")
    ssh(host, "apt-get update -qq && apt-get install -y -q supervisor curl")

    print("==> 4/6  download corpus")
    ssh(host, f"mkdir -p {BASE}/corpus {BASE}/index")
    for f in CORPUS_FILES:
        ssh(host, f"[ -f {BASE}/corpus/{f} ] || curl -fsSL -o {BASE}/corpus/{f} '{gh_url(CORPUS_TAG, f)}'")

    print("==> 5/6  download index")
    for f in INDEX_FILES:
        ssh(host, f"[ -f {BASE}/index/{f} ] || curl -fsSL -o {BASE}/index/{f} '{gh_url(INDEX_TAG, f)}'")

    print("==> 6/6  configure supervisor")
    pipe_to(host, SUPERVISOR_CONF, (
        f"[program:gutenindex-web]\n"
        f"command={BINARY} --index {BASE}/index --corpus {BASE}/corpus --port {PORT}\n"
        f"autostart=true\n"
        f"autorestart=true\n"
        f"stdout_logfile=/var/log/gutenindex-web.log\n"
        f"stderr_logfile=/var/log/gutenindex-web.log\n"
    ))
    ssh(host, "service supervisor start 2>/dev/null || true")
    ssh(host, "supervisorctl reread && supervisorctl update")
    ssh(host, "supervisorctl restart gutenindex-web 2>/dev/null || supervisorctl start gutenindex-web")

    print(f"\nhttp://{host}:{PORT}")


if __name__ == "__main__":
    main()
