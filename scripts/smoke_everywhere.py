#!/usr/bin/env python3
"""Black-box Everywhere acceptance; every write is inside an isolated synthetic home."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import time


def read_http_response(connection: socket.socket) -> bytes:
    """Read one bounded Content-Length response, not the TCP connection lifetime.

    Windows can abort the transport after a complete response. Stop at the HTTP
    message boundary; never suppress a socket error or accept a truncated body.
    SSE is intentionally read separately because it has no Content-Length.
    """
    result = bytearray()
    while b'\r\n\r\n' not in result:
        part = connection.recv(4096)
        assert part, 'local API ended before response headers'
        result.extend(part)
        boundary = result.find(b'\r\n\r\n')
        assert (boundary + 4 if boundary >= 0 else len(result)) <= 8192, 'API response headers exceed limit'
    header_end = result.index(b'\r\n\r\n') + 4
    headers: dict[bytes, bytes] = {}
    for line in bytes(result[:header_end - 4]).split(b'\r\n')[1:]:
        name, separator, value = line.partition(b':')
        name = name.lower()
        assert separator and name and name not in headers, 'invalid or duplicate API response header'
        headers[name] = value.strip()
    length = headers.get(b'content-length', b'')
    assert length.isdigit() and len(length) <= 7, 'API response needs an explicit valid length'
    assert b'transfer-encoding' not in headers, 'ambiguous API response framing'
    size = int(length)
    assert size <= 2 * 1024 * 1024, 'API response body exceeds limit'
    message_end = header_end + size
    assert len(result) <= message_end, 'unexpected bytes after API response'
    while len(result) < message_end:
        part = connection.recv(min(65536, message_end - len(result)))
        assert part, 'local API ended before the complete response body'
        result.extend(part)
    return bytes(result)


def run(binary: Path, output: Path | None) -> None:
    checks: list[str] = []
    with tempfile.TemporaryDirectory(prefix='nyrva-everywhere-') as temporary:
        base = Path(temporary)
        root, home = base / 'data with spaces', base / 'home with spaces'
        home.mkdir()
        env = dict(os.environ, NYRVA_DATA_DIR=str(root), HOME=str(home), USERPROFILE=str(home),
                   APPDATA=str(base / 'config'), XDG_CONFIG_HOME=str(base / 'config'))
        def call(*args: str, payload: str | None = None, expected: int = 0) -> str:
            p = subprocess.run([str(binary), *args], input=payload, text=True, encoding='utf-8',
                               capture_output=True, env=env, timeout=20)
            assert p.returncode == expected, f'{args[0]} exited {p.returncode}: {p.stderr[:300]}'
            return p.stdout
        for args in [('doctor','--json'), ('export','--json'), ('top','--once','--json'),
                     ('privacy','status','--json'), ('settings','export','--json'),
                     ('migrate','status','--json'), ('update','status','--json')]:
            assert json.loads(call(*args))['schema_version'] == 1
        assert not root.exists()
        assert '\x1b' not in call('top')
        checks.append('all read entrypoints are versioned, headless and non-mutating')

        settings = json.loads(call('settings','export','--json'))['settings']
        settings['retention_days'] = 30
        call('settings','import','--apply', payload=json.dumps(settings))
        settings['retention_days'] = 7
        call('settings','import','--apply', payload=json.dumps(settings))
        call('settings','rollback','--apply')
        assert json.loads(call('settings','export'))['settings']['retention_days'] == 30
        call('settings','import','--apply', payload='{"secret":"do not import"}', expected=1)
        checks.append('portable preferences are validated and rollback restores the previous version')

        target = home / '.claude' / 'settings.json'
        target.parent.mkdir()
        original = {'theme':'unchanged', 'statusLine':{'type':'command','command':'DO_NOT_EXECUTE_OLD_COMMAND'},
                    'fixtureAuthentication':'SYNTHETIC_PROVIDER_SECRET'}
        target.write_text(json.dumps(original), encoding='utf-8')
        assert json.loads(call('integrations','plan','claude','--home',str(home)))['replacement_required']
        call('integrations','install','claude','--home',str(home),'--executable',str(binary),'--apply',expected=1)
        install = ('integrations','install','claude','--home',str(home),'--executable',str(binary),'--apply','--replace')
        call(*install)
        installed = json.loads(target.read_text(encoding='utf-8'))
        assert installed['fixtureAuthentication'] == original['fixtureAuthentication']
        assert installed['theme'] == 'unchanged'
        assert not json.loads(call(*install))['changed']
        command = installed['statusLine']['command']
        payload = json.dumps({'session_id':'smoke-session', 'model':{'id':'fixture-model'},
                              'rate_limits':{'five_hour':{'used_percentage':100,'resets_at':int(time.time())+3600}},
                              'prompt':'SYNTHETIC_PROMPT_SECRET'})
        shell_args = ['powershell.exe','-NoProfile','-NonInteractive','-Command',command] if os.name == 'nt' else ['sh','-c',command]
        p = subprocess.run(shell_args, input=payload, text=True, encoding='utf-8', capture_output=True, env=env, timeout=20)
        assert p.returncode == 0, p.stderr[:300]
        assert '0%' in p.stdout
        current = json.loads(call('status','--json'))
        assert current['providers'][0]['provider'] == 'claude'
        assert current['providers'][0]['buckets'][0]['remaining_fraction'] == 0
        installed['theme'] = 'user-edited-during-install'
        installed['statusLine'] = {'type':'command','command':'USER_CHANGED_COMMAND'}
        target.write_text(json.dumps(installed), encoding='utf-8')
        call('integrations','remove','claude','--home',str(home),'--apply',expected=1)
        installed['statusLine']['command'] = command
        target.write_text(json.dumps(installed), encoding='utf-8')
        call('integrations','remove','claude','--home',str(home),'--apply')
        restored = json.loads(target.read_text(encoding='utf-8'))
        assert restored['statusLine'] == original['statusLine']
        assert restored['theme'] == 'user-edited-during-install'
        assert restored['fixtureAuthentication'] == original['fixtureAuthentication']
        checks.append('real generated shell command ingests in the chosen root; ownership-safe removal preserves unrelated settings')

        exported = call('export','--json')
        assert 'SYNTHETIC_PROVIDER_SECRET' not in exported and 'SYNTHETIC_PROMPT_SECRET' not in exported
        assert str(home) not in exported and command not in exported
        for path in root.rglob('*'):
            if path.is_file():
                b = path.read_bytes()
                assert b'SYNTHETIC_PROVIDER_SECRET' not in b and b'SYNTHETIC_PROMPT_SECRET' not in b
        checks.append('diagnostics and Nyrva storage exclude provider authentication and prompts')

        server = subprocess.Popen([str(binary),'serve','--port','0','--duration','5'], env=env,
                                  stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        try:
            session_file = root / 'telemetry/api-session.json'
            deadline = time.monotonic()+10
            while not session_file.exists():
                assert time.monotonic() < deadline and server.poll() is None, 'local API did not start'
                time.sleep(.03)
            session = json.loads(session_file.read_text(encoding='utf-8'))
            assert re.fullmatch(r'127\.0\.0\.1:\d{1,5}',session['address'])
            assert re.fullmatch(r'[0-9a-f]{64}',session['token'])
            if os.name != 'nt':
                assert session_file.stat().st_mode & 0o077 == 0
            else:
                # Inspect the actual file ACL, without printing the token or changing permissions.
                q = str(session_file).replace("'", "''")
                script = f"$a=Get-Acl -LiteralPath '{q}'; if(-not $a.AreAccessRulesProtected -or @($a.Access).Count -ne 1){{exit 1}}"
                assert subprocess.run(['powershell','-NoProfile','-NonInteractive','-Command',script],capture_output=True,timeout=10).returncode == 0
            host, port = session['address'].split(':')
            def request(path: str, *, token: bool = True, origin: bool = False, method: str = 'GET') -> bytes:
                with socket.create_connection((host,int(port)),timeout=3) as c:
                    header = f'{method} {path} HTTP/1.1\r\nHost: {session["address"]}\r\n'
                    if token: header += f'Authorization: Bearer {session["token"]}\r\n'
                    if origin: header += 'Origin: https://untrusted.example\r\n'
                    c.sendall((header+'\r\n').encode())
                    return read_http_response(c)
            assert request('/v1/status',token=False).startswith(b'HTTP/1.1 401')
            assert request('/v1/status',origin=True).startswith(b'HTTP/1.1 403')
            assert request('/v1/status',method='POST').startswith(b'HTTP/1.1 405')
            response=request('/v1/status')
            assert response.startswith(b'HTTP/1.1 200') and b'access-control-allow-origin' not in response.lower()
            assert json.loads(response.split(b'\r\n\r\n',1)[1])['schema_version'] == 1
            with socket.create_connection((host,int(port)),timeout=3) as c:
                c.sendall(f'GET /v1/events HTTP/1.1\r\nHost: {session["address"]}\r\nAuthorization: Bearer {session["token"]}\r\n\r\n'.encode())
                result=b''
                while b'\n\n' not in result.split(b'\r\n\r\n',1)[-1]:
                    chunk=c.recv(65536)
                    assert chunk, 'event stream ended without an event'
                    result+=chunk
                assert b'text/event-stream' in result and b'data: ' in result
                assert session['token'].encode() not in result
            assert server.wait(timeout=10) == 0
            assert not session_file.exists()
            checks.append('real loopback HTTP/SSE enforces authentication, host/privacy boundaries and session cleanup')
        finally:
            if server.poll() is None: server.kill()
            server.wait()
            if server.stderr: server.stderr.close()

        assert not json.loads(call('update','status'))['configured']
        call('update','verify',expected=1)
        call('migrate','rollback','--apply')
        assert json.loads(call('migrate','status'))['database_version'] == 1
        call('migrate','--apply')
        assert json.loads(call('migrate','status'))['database_version'] == 2
        call('privacy','clear','--confirm')
        assert json.loads(call('status','--json'))['providers'] == []
        assert json.loads(target.read_text(encoding='utf-8')) == restored
        checks.append('migration round trip preserves data; clear is Nyrva-only; unsigned updates fail closed')

        if os.name == 'posix':
            import pty, select, termios
            master, slave = pty.openpty()
            before = termios.tcgetattr(slave)
            p = subprocess.Popen([str(binary),'top','--interval','1'], stdin=slave,stdout=slave,stderr=slave,
                                 env=dict(env,TERM='xterm'),close_fds=True)
            try:
                deadline=time.monotonic()+8; data=b''
                while b'q = quit' not in data and time.monotonic()<deadline:
                    if select.select([master],[],[],.2)[0]: data+=os.read(master,65536)
                assert b'q = quit' in data, 'interactive terminal did not render'
                os.write(master,b'q\n')
                assert p.wait(timeout=3)==0, 'terminal could not exit through its input worker'
                assert termios.tcgetattr(slave)==before, 'terminal settings changed'
                checks.append('real PTY accepts q+Enter and leaves terminal modes unchanged')
            finally:
                if p.poll() is None: p.kill()
                p.wait(); os.close(master); os.close(slave)
    report={'binary':binary.name,'status':'passed','checks':checks,'real_provider_authentication':False}
    text=json.dumps(report,indent=2,ensure_ascii=False)
    if output: output.parent.mkdir(parents=True,exist_ok=True); output.write_text(text+'\n',encoding='utf-8')
    print(text)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--output',type=Path)
    args=parser.parse_args();run(args.binary.resolve(strict=True),args.output)
