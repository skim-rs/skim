import pytest

from e2e.base import TestBase, SK
from e2e.tmux import Key, Ctrl, Alt

common_binds = [
    ([Ctrl('a')], '{cur}foo bar foo-bar'),
    ([Ctrl('a'), Alt('f')], 'foo{cur} bar foo-bar'),
    ([Ctrl('b')], 'foo bar foo-ba{cur}r'),
    ([Key('Left'), Key('Left'), Ctrl('d')], 'foo bar foo-b{cur}r'),
    ([Alt('BSpace')], 'foo bar foo-{cur}'),
    ([Alt('BSpace'), Ctrl('y'), Ctrl('y')], 'foo bar foo-barbar{cur}'),
    ([Alt('b')], 'foo bar foo-{cur}bar'),
    ([Ctrl('a'), Ctrl('f'), Key('Right')], 'fo{cur}o bar foo-bar'),
    ([Ctrl('h')], 'foo bar foo-ba{cur}'),
    ([Key('BSpace')], 'foo bar foo-ba{cur}'),
    ([Ctrl('a'), Ctrl('e')], 'foo bar foo-bar{cur}'),
    ([Ctrl('u')], '{cur}'),
    ([Ctrl('w')], 'foo bar {cur}'),
    ([Alt('b'), Alt('b'), Alt('d'), Ctrl('a'),
      Ctrl('y')], 'foo{cur}foo bar -bar')
]


class TestBinds(TestBase):
    cursor = '|'

    def send_keys(self, *keys):
        self.tmux.send_keys(*keys, Key(self.cursor))

    def check_query_line(self, query, prompt='>'):
        self.tmux.until(
            lambda lines: lines[-1] == prompt + ' ' + query.format(cur=self.cursor))
        assert True

    @pytest.mark.parametrize('keys,line', common_binds)
    def test_common_keys(self, keys, line):
        self.tmux.send_keys(f"{SK} -q 'foo bar foo-bar'", Key('Enter'))
        self.tmux.until(lambda lines: lines[-1].startswith('>'))
        print("Keys:", keys)
        self.send_keys(*keys)
        self.check_query_line(line)

    @pytest.mark.parametrize('keys,line', common_binds)
    def test_common_interactive_keys(self, keys, line):
        self.tmux.send_keys(
            f"{SK} -i --cmd-query 'foo bar foo-bar'", Key('Enter'))
        self.tmux.until(lambda lines: lines[-1].startswith('c>'))
        print("Keys:", keys)
        self.send_keys(*keys)
        self.check_query_line(line, prompt='c>')

    def test_ctrl_r(self):
        self.tmux.send_keys(f"{SK} -q 'foo bar foo-bar'", Key('Enter'))
        self.tmux.until(
            lambda lines: lines[-1].startswith('>') and not 'RE' in lines[-2])
        self.send_keys(Ctrl('r'))
        self.tmux.until(lambda lines: 'RE' in lines[-2])
        assert True
