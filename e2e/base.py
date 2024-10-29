import inspect
import os
import subprocess
import time

from e2e.tmux import Key, Tmux
from e2e.utils import wait
from e2e.config import SK, BASE

class TestBase:
    TEMPNAME = '/tmp/output'

    def setup_method(self, _method):
        self._temp_suffix = 0
        os.chdir(BASE)
        self.tmux = Tmux()
        subprocess.run(["cargo", "build", "--release"])

    def teardown_method(self, _method):
        self.tmux.kill()


    def tempname(self, suffix=None):
        if suffix is None:
            suffix = self._temp_suffix
        curframe = inspect.currentframe()
        frames = inspect.getouterframes(curframe)

        names = [f.function for f in frames if f.function.startswith('test_')]
        fun_name = names[0] if len(names) > 0 else 'test'

        tempname = '-'.join((TestBase.TEMPNAME, fun_name, str(suffix)))
        return tempname

    def writelines(self, path, lines):
        if os.path.exists(path):
            os.remove(path)

        with open(path, 'w') as fp:
            fp.writelines(lines)

    def readonce(self, suffix=None):
        path = self.tempname(suffix)
        try:
            wait(lambda: os.path.exists(path))
            with open(path) as fp:
                return fp.read()
        except TimeoutError as e:
            print(f"{path} does not exist, aborting")
            raise e
        finally:
            if os.path.exists(path):
                os.remove(path)
            self._temp_suffix += 1
            self.tmux.prepare()

    def sk(self, *opts, suffix=None):
        tmp = self.tempname(suffix)
        return f'{SK} {" ".join(map(str, opts))} > {tmp}.tmp; mv {tmp}.tmp {tmp}'

    def command_until(self, until_predicate, sk_options, stdin="echo -e 'a1\\na2\\na3'"):
        command_keys = stdin + " | " + self.sk(*sk_options)
        self.tmux.send_keys(command_keys)
        self.tmux.send_keys(Key("Enter"))
        self.tmux.until(until_predicate,
                        debug_info="SK args: {}".format(sk_options))
        self.tmux.send_keys(Key('Enter'))

    def line_at(self, index: int):
        """
        Get line at index in output
        """
        cap = self.tmux.capture()
        return cap[index]
