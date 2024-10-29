from e2e.utils import find_prompt
from e2e.base import TestBase, SK
from e2e.tmux import Key, Ctrl, Alt, Tmux


class TestActions(TestBase):
    def test_reload(self):
        sk_command = self.sk("--bind", "\"ctrl-b:reload(echo -e 'a\\nb\\nc')\"")
        self.tmux.send_keys(
            "echo -e 'field1' |" + sk_command, Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.send_keys(Ctrl("b"))
        self.tmux.until(lambda lines: lines.ready_with_lines(3))
        assert True

    def test_reload_cmd_error(self):
        sk_command = self.sk("--show-cmd-error", "--bind", "\"ctrl-b:reload(foobarbaz)\"")
        self.tmux.send_keys(
            "echo -e 'field1' |" + sk_command, Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.send_keys(Ctrl("b"))
        # Check for error
        self.tmux.until(lambda lines: "foobarbaz" in lines[-3])
        assert True
