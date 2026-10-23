#!/usr/bin/env python3
"""POSIX pseudo-terminal regressions; production console uses no Python or new crates."""
import codecs
import fcntl
import json
import os
from pathlib import Path
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
import unicodedata

BINARY = Path(sys.argv[1] if len(sys.argv) > 1 else 'target/debug/i').resolve()
class Screen:
    def __init__(self, columns=32, rows=10):
        self.columns, self.rows = columns, rows
        self.lines = [[' '] * columns for _ in range(rows)]
        self.x = self.y = 0
        self.escape = ''
        self.decoder = codecs.getincrementaldecoder('utf8')('strict')
        self.full_clears = 0
    def down(self):
        self.y += 1
        if self.y == self.rows:
            self.lines.pop(0); self.lines.append([' '] * self.columns); self.y -= 1
    def put(self, c):
        width = 2 if unicodedata.east_asian_width(c) in ('W','F') else 1
        if self.x + width > self.columns:
            self.x = 0; self.down()
        self.lines[self.y][self.x] = c
        if width == 2: self.lines[self.y][self.x+1] = ''
        self.x += width
    def feed(self, data):
        for c in self.decoder.decode(data):
            if self.escape:
                self.escape += c
                if self.escape == '\x1b[': continue
                if len(self.escape) > 2 and '@' <= c <= '~':
                    value = self.escape[2:-1]
                    if c == 'K': self.lines[self.y] = [' '] * self.columns
                    elif c == 'J':
                        if value == '2': self.full_clears += 1
                        self.lines[self.y][min(self.x,self.columns):] = [' '] * (self.columns-min(self.x,self.columns))
                        for row in range(self.y+1,self.rows): self.lines[row] = [' '] * self.columns
                    elif c == 'G': self.x = int(value or '1')-1
                    elif c == 'A': self.y = max(0,self.y-int(value or '1'))
                    self.escape = ''
                continue
            if c == '\x1b': self.escape = c
            elif c == '\r': self.x = 0
            elif c == '\n': self.down()
            else: self.put(c)
    def line(self): return ''.join(self.lines[self.y]).rstrip()
    def resize(self, columns):
        # Simulate the reflow of the current editable row, including wide-cell gaps.
        text = ''.join(self.lines[self.y]).rstrip(); cursor = self.x; old_y = self.y
        self.columns = columns
        self.lines = [(line + [' '] * columns)[:columns] for line in self.lines]
        self.lines[old_y] = [' '] * columns; self.x = 0
        cells = 0; location = None
        for c in text:
            if cells == cursor: location = (self.x,self.y)
            self.put(c); cells += 2 if unicodedata.east_asian_width(c) in ('W','F') else 1
        if location: self.x,self.y = location

class Console:
    def __init__(self, root, backend):
        self.root = root; self.screen = Screen(); self.data = bytearray()
        self.master,self.slave = os.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack('HHHH',10,32,0,0))
        self.original = termios.tcgetattr(self.slave)
        env = os.environ.copy()
        for key in ('API_KEY','DEEPSEEK_API_KEY','DEEPSEEK_TEST_API_KEY','OPENAI_API_KEY'):
            env.pop(key,None)
        env.update(TERM='xterm-256color', UI_TEST_ROOT=str(root), PYTHONIOENCODING='utf-8')
        for name in ('CONFIG','DATA','CACHE','RUNTIME'): env[f'I_{name}_DIR'] = str(root/name.lower())
        self.child = subprocess.Popen([str(BINARY),'--runtime','console','--no-logo',f'--bin={backend}',
            f'--dir={root}','--session-name=pty'], stdin=self.slave,stdout=self.slave,stderr=self.slave,
            env=env,start_new_session=True)
        self.wait(lambda: b'\x1b[?2004h' in self.data and 'lninl>' in self.screen.line())
        assert not termios.tcgetattr(self.slave)[3] & (termios.ECHO | termios.ICANON)
    def pump(self, seconds=.1):
        until = time.monotonic()+seconds
        while time.monotonic() < until:
            if select.select([self.master],[],[],max(0,until-time.monotonic()))[0]:
                data = os.read(self.master,65536); self.data.extend(data); self.screen.feed(data)
    def wait(self, condition, timeout=8):
        until=time.monotonic()+timeout
        while not condition():
            if time.monotonic()>until: raise AssertionError(f'console timeout; screen={self.screen.line()!r}, tail={bytes(self.data[-1500:])!r}')
            self.pump(.03)
    def send(self, text):
        os.write(self.master,text.encode() if isinstance(text,str) else text); self.pump(.12)
    def records(self):
        path=self.root/'calls.jsonl'
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
    def finish(self, code):
        self.wait(lambda:self.child.poll() is not None); self.pump()
        assert self.child.returncode==code,(self.child.returncode,bytes(self.data[-1000:]))
        assert termios.tcgetattr(self.slave)==self.original, 'terminal attributes not restored'
        assert b'\x1b[?2004l' in self.data, 'bracketed paste not disabled'
    def close(self):
        if self.child.poll() is None: self.child.kill(); self.child.wait()
        termios.tcsetattr(self.slave,termios.TCSANOW,self.original)
        os.close(self.master);os.close(self.slave)

with tempfile.TemporaryDirectory(prefix='i-console-check-') as temporary:
    root=Path(temporary)
    backend=root/'backend'
    backend.write_text('''#!/usr/bin/env python3
import json,os,subprocess,sys,time
from pathlib import Path
root=Path(os.environ['UI_TEST_ROOT']);task=sys.argv[-1]
with (root/'calls.jsonl').open('a') as file: file.write(json.dumps(task,ensure_ascii=False)+'\\n')
if task=='慢任务':
    time.sleep(.3)
    encoded='后台你好'.encode();sys.stdout.buffer.write(encoded[:1]);sys.stdout.buffer.flush()
    time.sleep(.2);sys.stderr.write('\\x1b[2J');sys.stderr.flush()
    sys.stdout.buffer.write(encoded[1:]+b'\\n');sys.stdout.buffer.flush()
    time.sleep(.4);print('后台完成',flush=True)
elif task=='审批':
    sys.stderr.write('\\x1e'+os.environ['I_APPROVAL_TOKEN']+'\\x1eAllow? [y/N] ');sys.stderr.flush()
    (root/'approval.txt').write_text(sys.stdin.readline());print('APPROVED',flush=True)
elif task=='模式':
    manifest=json.loads(Path(sys.argv[sys.argv.index('--plugins')+1]).read_text())
    hook=next(h for h in manifest['hooks'] if h.get('module')=='approval')
    mode=Path(hook['exec'][hook['exec'].index('--mode-file')+1])
    worker=subprocess.Popen(hook['exec'],cwd=hook['cwd'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
    try:
        for ident,expected in enumerate(('auto','ask','auto'),1):
            deadline=time.monotonic()+8
            while json.loads(mode.read_text())['mode']!=expected:
                assert time.monotonic()<deadline,'mode update timed out'
                time.sleep(.01)
            frame={'id':ident,'event':'tool_approval','workspace':str(root),
                   'data':{'name':'write','arguments':{},'external':False,'decision':'ask'}}
            worker.stdin.write(json.dumps(frame)+'\\n');worker.stdin.flush()
            reply=json.loads(worker.stdout.readline())
            assert reply['id']==ident and 'error' not in reply,reply
            (root/('mode-'+str(ident)+'.json')).write_text(json.dumps(reply['data']))
    finally:
        worker.stdin.close();worker.wait(timeout=3)
    print('MODE DONE',flush=True)
elif task=='等待':
    print('WAITING',flush=True);time.sleep(60)
else: print('任务完成',flush=True)
''')
    backend.chmod(0o755)
    console=Console(root,backend)
    try:
        for _ in range(2):
            console.send('你好');assert console.screen.line()=='lninl> 你好';assert console.screen.x==11
            console.send(b'\x7f');assert console.screen.line()=='lninl> 你';assert console.screen.x==9
            console.send(b'\x7f');assert console.screen.line()=='lninl>';assert console.screen.x==7
        console.send('甲丙');console.send(b'\x1b[D');console.send('乙');console.send(b'\x1b[3~\x1b[H\x1b[3~\x1b[F')
        console.send('好\r');console.wait(lambda:console.records()==['乙好'])
        console.wait(lambda:console.data.count(b'[task finished]')==1)
        console.send('\x1b[200~中文\n/exit\n代码\x1b[201~')
        assert console.records()==['乙好'];assert console.child.poll() is None
        # The 32-column editor scrolls the start of the folded paste marker.
        assert console.screen.line().endswith('Content #1 11 chars]'), console.screen.line()
        console.send('\r');console.wait(lambda:len(console.records())==2)
        assert console.records()[1]=='中文\n/exit\n代码'
        console.wait(lambda:console.data.count(b'[task finished]')==2)
        console.send('慢任务\r');console.wait(lambda:len(console.records())==3)
        console.send('草稿中文');console.wait(lambda:'后台完成'.encode() in console.data)
        console.wait(lambda:console.data.count(b'[task finished]')==3)
        assert console.screen.line()=='lninl> 草稿中文',console.screen.line()
        assert '\ufffd'.encode() not in console.data
        assert console.screen.full_clears==0,'backend escape sequence reached terminal'
        console.send('\r');console.wait(lambda:len(console.records())==4);assert console.records()[3]=='草稿中文'
        console.wait(lambda:console.data.count(b'[task finished]')==4)
        console.send('审批\r');console.wait(lambda:b'[y/N]' in console.data)
        console.send('/mode auto\r');assert not (root/'approval.txt').exists(), 'mode bypassed a pending ask decision'
        console.send('/mode ask\r');assert not (root/'approval.txt').exists()
        console.send('y\r');console.wait(lambda:(root/'approval.txt').exists())
        assert (root/'approval.txt').read_text()=='y\n'
        console.wait(lambda:console.data.count(b'[task finished]')==5)
        console.send('未提交');console.send(b'\x1b[A');assert '审批' in console.screen.line()
        console.send(b'\x1b[B');assert '未提交' in console.screen.line();console.send(b'\x15')
        previous_line=''.join(console.screen.lines[console.screen.y-1]).rstrip()
        console.send('很长中文'*40);assert console.screen.x<32
        console.screen.resize(20);fcntl.ioctl(console.slave,termios.TIOCSWINSZ,struct.pack('HHHH',10,20,0,0))
        console.pump(.3);assert console.screen.x<20
        console.send(b'\x15');assert console.screen.line()=='lninl>'
        assert ''.join(console.screen.lines[console.screen.y-1]).rstrip()==previous_line
        console.send('/mode auto\r');console.send('模式\r')
        console.wait(lambda:(root/'mode-1.json').exists())
        assert json.loads((root/'mode-1.json').read_text())['decision']=='allow'
        console.send('/mode ask\r');console.wait(lambda:(root/'mode-2.json').exists())
        assert json.loads((root/'mode-2.json').read_text())['decision']=='ask'
        console.send('/mode auto\r');console.wait(lambda:(root/'mode-3.json').exists())
        assert json.loads((root/'mode-3.json').read_text())['decision']=='allow'
        console.wait(lambda:console.data.count(b'[task finished]')==6)
        console.send('/mode\r');console.send('/mode status\r')
        assert console.records()[-1]=='模式', 'mode commands leaked into task input'
        console.send('/exit\r');console.finish(0)
    finally: console.close()
    for action in ('ctrl-c','sigterm','error'):
        console=Console(root,root/'does-not-exist' if action=='error' else backend)
        try:
            if action=='error': console.send('测试失败\r');console.finish(1)
            else:
                console.send('等待\r');console.wait(lambda:b'WAITING' in console.data)
                if action=='ctrl-c':console.send(b'\x03')
                else: console.child.send_signal(signal.SIGTERM)
                console.finish(130)
        finally:console.close()
print('PASS: PTY Chinese editing/cursor, paste, history, resize, concurrent UTF-8 output, approvals, live plugin modes, Ctrl-C/SIGTERM/error restoration')
