import re
import subprocess
import time
import os
from e2e import utils

INPUT_RECORD_SEPARATOR = '\n'

class Shell(object):
    """The shell configurations for tmux tests"""

    def __init__(self):
        super(Shell, self).__init__()

    @staticmethod
    def unsets():
        return 'unset SKIM_DEFAULT_COMMAND SKIM_DEFAULT_OPTIONS;'

    @staticmethod
    def bash():
        return 'PS1= PROMPT_COMMAND= bash --rcfile None'

    @staticmethod
    def zsh():
        return 'PS1= PROMPT_COMMAND= HISTSIZE=100 zsh -f'


class Key(object):
    """Represent a key to send to tmux"""

    def __init__(self, key):
        super(Key, self).__init__()
        self.key = key

    def __repr__(self):
        return self.key


class Ctrl(Key):
    """Represent a control key"""

    def __init__(self, key):
        super(Ctrl, self).__init__(key)

    def __repr__(self):
        return f'C-{self.key.upper()}'


class Alt(Key):
    """Represent an alt key"""

    def __init__(self, key):
        super(Alt, self).__init__(key)

    def __repr__(self):
        return f'M-{self.key}'


class TmuxOutput(list):
    """A list that contains the output of tmux"""
    # match the status line
    # normal:  `| 10/219 [2]               8/0.`
    # inline:  `> query < 10/219 [2]       8/0.`
    # preview: `> query < 10/219 [2]       8/0.│...`
    RE = re.compile(
        r'(?:^|[^<-]*). ([0-9]+)/([0-9]+)(?:/[A-Z]*)?(?: \[([0-9]+)\])? *([0-9]+)/(-?[0-9]+)(\.)?(?: │)? *$')

    def __init__(self, iteratable=[]):
        super(TmuxOutput, self).__init__(iteratable)
        self._counts = None

    def counts(self):
        if self._counts is not None:
            return self._counts

        # match_count item_count select_count item_cursor matcher_stopped
        ret = (0, 0, 0, 0, 0, '.')
        for line in self:
            mat = TmuxOutput.RE.match(line)
            if mat is not None:
                ret = mat.groups()
                break
        self._counts = ret
        return ret

    def match_count(self):
        count = self.counts()[0]
        return int(count) if count is not None else None

    def item_count(self):
        count = self.counts()[1]
        return int(count) if count is not None else None

    def select_count(self):
        count = self.counts()[2]
        return int(count) if count is not None else None

    def item_index(self):
        count = self.counts()[3]
        return int(count) if count is not None else None

    def hscroll(self):
        count = self.counts()[4]
        return int(count) if count is not None else None

    def matcher_stopped(self):
        return self.counts()[5] != '.'

    def ready_with_lines(self, lines):
        return self.item_count() == lines and self.matcher_stopped()

    def ready_with_matches(self, matches):
        return self.match_count() == matches and self.matcher_stopped()

    def any_include(self, val):
        if hasattr(re, '_pattern_type') and isinstance(val, re._pattern_type):
            def method(l): return val.match(l)
        if hasattr(re, 'Pattern') and isinstance(val, re.Pattern):
            def method(l): return val.match(l)
        else:
            def method(l): return l.find(val) >= 0
        for line in self:
            if method(line):
                return True
        return False


class Tmux(object):
    TEMPNAME = '/tmp/skim-test.txt'

    """Object to manipulate tmux and get result"""

    def __init__(self, shell='bash'):
        super(Tmux, self).__init__()

        if shell == 'bash':
            shell_cmd = Shell.unsets() + Shell.bash()
        elif shell == 'zsh':
            shell_cmd = Shell.unsets() + Shell.zsh()
        else:
            raise BaseException('unknown shell')

        self.win = self._go("new-window", "-d", "-P",
                            "-F", "#I", f"{shell_cmd}")[0]
        self._go("set-window-option", "-t",
                 f"{self.win}", "pane-base-index", "0")
        self.lines = int(subprocess.check_output(
            'tput lines', shell=True).decode('utf8').strip())

    def _go(self, *args, **kwargs):
        """Run tmux command and return result in list of strings (lines)

        :returns: List<String>
        """
        ret = subprocess.check_output(["tmux"] + list(args))
        return ret.decode('utf8').split(INPUT_RECORD_SEPARATOR)

    def kill(self):
        self._go("kill-window", "-t", f"{self.win}", stderr=subprocess.DEVNULL)

    def send_keys(self, *args, pane=None):
        if pane is not None:
            self._go('select-window', '-t', f'{self.win}')
            target = '{self.win}.{pane}'
        else:
            target = self.win

        for key in args:
            if key is None:
                continue
            else:
                self._go('send-keys', '-t', f'{target}', f'{key}')
            time.sleep(0.01)

    def paste(self, content):
        subprocess.run(["tmux", "setb", f"{content}", ";",
                        "pasteb", "-t", f"{self.win}", ";",
                        "send-keys", "-t", f"{self.win}", "Enter"])

    def capture(self, pane=0):
        def save_capture():
            try:
                self._go('capture-pane', '-t',
                         f'{self.win}.{pane}', stderr=subprocess.DEVNULL)
                self._go("save-buffer",
                         f"{Tmux.TEMPNAME}", stderr=subprocess.DEVNULL)
                return True
            except subprocess.CalledProcessError:
                return False

        if os.path.exists(Tmux.TEMPNAME):
            os.remove(Tmux.TEMPNAME)

        utils.wait(save_capture)
        with open(Tmux.TEMPNAME) as fp:
            content = fp.read()
            return TmuxOutput(content.rstrip().split(INPUT_RECORD_SEPARATOR))

    def until(self, predicate, refresh=False, pane=0, debug_info=None):
        def wait_callback():
            lines = self.capture()
            pred = predicate(lines)
            if pred:
                self.send_keys(Ctrl('l') if refresh else None)
                assert True
            return pred

        def timeout_handler():
            lines = self.capture()
            print("Timeout", lines)
            if debug_info:
                print("Timeout debug", debug_info)
            assert predicate(lines)
        utils.wait(wait_callback, timeout_handler)

    def prepare(self):
        try:
            self.send_keys(Ctrl('u'), Key('hello'))
            self.until(lambda lines: lines[-1].endswith('hello'))
        except Exception as e:
            raise e
        self.send_keys(Ctrl('u'))

