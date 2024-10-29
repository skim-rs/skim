import re

from e2e.base import TestBase, SK
from e2e.tmux import Key, Ctrl, Alt, Tmux


class TestVanilla(TestBase):
    def test_vanilla(self):
        self.tmux.send_keys(Key(f'seq 1 100000 | {self.sk()}'), Key('Enter'))
        self.tmux.until(lambda lines: re.match(
            r'^>', lines[-1]) and re.match(r'^  100000', lines[-2]))
        lines = self.tmux.capture()
        assert lines[-4] == '  2'
        assert lines[-3] == '> 1'
        assert re.match('^  100000/100000 *0', lines[-2])
        assert re.match('^  100000/100000 *0', lines[-2])
        assert lines[-1] == '>'

        # testing basic key binding
        self.tmux.send_keys(Key('99'))
        self.tmux.until(lambda ls: ls[-2].startswith('  8146/100000'))
        self.tmux.until(lambda ls: ls[-1].startswith('> 99'))

        self.tmux.send_keys(Ctrl('a'), Key('1'))
        self.tmux.until(lambda ls: ls[-2].startswith('  856/100000'))
        self.tmux.until(lambda ls: ls[-1].startswith('> 199'))

        self.tmux.send_keys(Ctrl('f'), Key('3'))
        self.tmux.until(lambda ls: ls[-2].startswith('  46/100000'))
        self.tmux.until(lambda ls: ls[-1].startswith('> 1939'))

        self.tmux.send_keys(Ctrl('b'), Ctrl('h'))
        self.tmux.until(lambda ls: ls[-2].startswith('  856/100000'))
        self.tmux.until(lambda ls: ls[-1].startswith('> 139'))

        self.tmux.send_keys(Ctrl('e'), Ctrl('b'))
        self.tmux.send_keys(Ctrl('k'))
        self.tmux.until(lambda ls: ls[-4].startswith('> 1390'))
        self.tmux.until(lambda ls: ls[-3].startswith('  139'))

        self.tmux.send_keys(Key('Tab'))
        self.tmux.until(lambda ls: ls[-4].startswith('  1390'))
        self.tmux.until(lambda ls: ls[-3].startswith('> 139'))

        self.tmux.send_keys(Key('BTab'))
        self.tmux.until(lambda ls: ls[-4].startswith('> 1390'))
        self.tmux.until(lambda ls: ls[-3].startswith('  139'))

        lines = self.tmux.capture()
        assert lines[-4] == '> 1390'
        assert lines[-3] == '  139'
        assert lines[-2].startswith('  856/100000')
        assert lines[-1] == '> 139'

        self.tmux.send_keys(Key('Enter'))
        self.tmux.send_keys(Key('Enter'))
        self.tmux.until(lambda l: l != lines)
        assert self.readonce().strip() == '1390'

    def test_default_command(self):
        self.tmux.send_keys(self.sk().replace(
            'SKIM_DEFAULT_COMMAND=', "SKIM_DEFAULT_COMMAND='echo hello'"))
        self.tmux.send_keys(Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.send_keys(Key('Enter'))
        assert self.readonce().strip() == 'hello'
