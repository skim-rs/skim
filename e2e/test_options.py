import re
import subprocess
import pytest

from e2e.utils import find_prompt
from e2e.base import TestBase, SK
from e2e.tmux import Key, Ctrl, Alt, Tmux


class TestOptions(TestBase):
    def test_read0(self):
        nfiles = subprocess.check_output(
            "find .", shell=True).decode("utf-8").strip().split("\n")
        num_of_files = len(nfiles)

        self.tmux.send_keys(f"find . | {self.sk()}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(num_of_files))
        self.tmux.send_keys(Key('Enter'))

        self.tmux.send_keys(
            f"find . -print0 | {self.sk('--read0')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(num_of_files))

    def test_print0(self):
        self.tmux.send_keys(
            f"echo -e 'a\\nb' | {self.sk('-m', '--print0')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(2))
        self.tmux.send_keys(Key('BTab'), Key('BTab'), Key('Enter'))

        lines = self.readonce().strip()
        assert lines == 'a\0b\0'

    def test_print0_filter(self):
        self.tmux.send_keys(
            f"echo -e 'a\\naa\\nb' | {self.sk('-f', 'a', '--print0')}", Key('Enter'))

        self.tmux.until(lambda lines: len(lines)> 1)

        lines = self.readonce().strip()
        print("lines", lines)
        assert lines == 'a\0aa\0'

    def test_with_nth_preview(self):
        sk_command = self.sk(
            "--delimiter ','", '--with-nth 2..', '--preview', "'echo X{1}Y'")
        self.tmux.send_keys(
            "echo -e 'field1,field2,field3,field4' |" + sk_command, Key('Enter'))
        self.tmux.until(lambda lines: lines.any_include("Xfield1Y"))
        self.tmux.send_keys(Key('Enter'))

    @pytest.mark.parametrize('field,expected', [
        ('1', 'field1,'),
        ('2', 'field2,'),
        ('3', 'field3,'),
        ('4', 'field4'),
        ('5', ''),
        ('-1', 'field4'),
        ('-2', 'field3,'),
        ('-3', 'field2,'),
        ('-4', 'field1,'),
        ('-5', ''),
        ('2..', 'field2,field3,field4'),
        ('..3', 'field1,field2,field3,'),
        ('2..3', 'field2,field3,'),
        ('3..2', ''),
    ])
    def test_with_nth(self, field, expected):
        sk_command = self.sk("--delimiter ','", f'--with-nth={field}', suffix=field)
        self.tmux.send_keys(
            "echo -e 'field1,field2,field3,field4' |" + sk_command, Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        lines = self.tmux.capture()
        self.tmux.send_keys(Key('Enter'))
        assert lines[-3] == f'> {expected}'.strip()

    @pytest.mark.parametrize('field,query,count', [
        ('1', 'field1', 1),
        ('1', 'field2', 0),
        ('-1', 'field4', 1),
        ('-1', 'field3', 0),
        ('-5', 'f', 0),
        ('2..', 'field2', 1),
        ('2..', 'field4', 1),
        ('..3', 'field1', 1),
        ('..3', 'field3,', 1),
        ('2..3', '2,3', 1),
        ('3..2', 'f', 0),
    ])
    def test_nth(self, field, query, count):
        sk_command = self.sk(f"--delimiter ',' --nth={field} -q {query}")
        self.tmux.send_keys(
            "echo -e 'field1,field2,field3,field4' |" + sk_command, Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.send_keys(Key('Enter'))

    def test_print_query(self):
        self.tmux.send_keys(
            f"seq 1 1000 | {self.sk('-q 10', '--print-query')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1000))
        self.tmux.send_keys(Key('Enter'))

        lines = self.readonce().strip()
        assert '10\n10' == lines

    def test_print_cmd(self):
        self.tmux.send_keys(
            f"seq 1 1000 | {self.sk('--cmd-query 10', '--print-cmd')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1000))
        self.tmux.send_keys(Key('Enter'))

        lines = self.readonce().strip()
        assert lines == '10\n1'

    def test_print_cmd_and_query(self):
        self.tmux.send_keys(
            f"seq 1 1000 | {self.sk('-q 10', '--cmd-query cmd', '--print-cmd', '--print-query')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1000))
        self.tmux.send_keys(Key('Enter'))

        lines = self.readonce().strip()
        assert lines == '10\ncmd\n10'

    def test_hscroll(self):
        # XXXXXXXXXXXXXXXXX..
        self.tmux.send_keys(f"cat <<EOF | {self.sk('-q b')}", Key('Enter'))
        self.tmux.send_keys(f"b{'a'*1000}", Key('Enter'))
        self.tmux.send_keys(f"EOF", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.until(lambda lines: lines[-3].endswith('..'))
        self.tmux.send_keys(Key('Enter'))

        # ..XXXXXXXXXXXXXXXXXM
        self.tmux.send_keys(f"cat <<EOF | {self.sk('-q b')}", Key('Enter'))
        self.tmux.send_keys(f"{'a'*1000}b", Key('Enter'))
        self.tmux.send_keys(f"EOF", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.until(lambda lines: lines[-3].endswith('b'))
        self.tmux.send_keys(Key('Enter'))

        # ..XXXXXXXMXXXXXXX..
        self.tmux.send_keys(f"cat <<EOF | {self.sk('-q b')}", Key('Enter'))
        self.tmux.send_keys(f"{'a'*1000}b{'a'*1000}", Key('Enter'))
        self.tmux.send_keys(f"EOF", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.until(lambda lines: lines[-3].startswith('> ..'))
        self.tmux.until(lambda lines: lines[-3].endswith('..'))
        self.tmux.send_keys(Key('Enter'))

    def test_no_hscroll(self):
        self.tmux.send_keys(
            f"cat <<EOF | {self.sk('-q b', '--no-hscroll')}", Key('Enter'))
        self.tmux.send_keys(f"{'a'*1000}b", Key('Enter'))
        self.tmux.send_keys(f"EOF", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.until(lambda lines: lines[-3].startswith('> a'))
        self.tmux.send_keys(Key('Enter'))

    @pytest.mark.parametrize('input,args,pattern', [
        ('a\\tb', '', '> a       b'),
        ('a\\tb', '--tabstop 1', '> a b'),
        ('aa\\tb', '--tabstop 2', '> aa  b'),
        ('aa\\tb', '--tabstop 3', '> aa b'),
        ('a\\tb', '--tabstop 4', '> a   b'),
    ])
    def test_tabstop(self, input, args, pattern):
        self.tmux.send_keys(f"echo -e '{input}' | {self.sk(args)}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        print(pattern)
        print(self.tmux.capture())
        self.tmux.until(lambda lines: lines[-3].startswith(pattern))

    def test_inline_info(self):
        INLINE_INFO_SEP = " <"
        # the dot  accounts for spinner
        RE = re.compile(r'[^0-9]*([0-9]+)/([0-9]+)(?: \[([0-9]+)\])?')
        self.tmux.send_keys(
            f"echo -e 'a1\\na2\\na3\\na4' | {self.sk('--inline-info')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.match_count()
                        == lines.item_count())
        self.tmux.until(lambda lines: INLINE_INFO_SEP in lines[-1])
        self.tmux.send_keys("a")
        self.tmux.until(lambda lines: 'a' in lines[-1])
        lines = self.tmux.capture()
        self.tmux.send_keys(Key('Enter'))
        query_line = lines[-1]
        bef, after = query_line.split(INLINE_INFO_SEP)
        mat = RE.match(after)
        assert mat is not None
        ret = tuple(map(lambda x: int(x) if x is not None else 0, mat.groups()))
        assert len(ret) == 3
        assert (bef, ret[0], ret[1], ret[2]) == ("> a ", 4, 4, 0)

        # test that inline info is does not overwrite query
        self.tmux.send_keys(
            f"echo -e 'a1\\nabcd2\\nabcd3\\nabcd4' | {self.sk('--inline-info')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(4))
        self.tmux.send_keys("bc", Ctrl("a"), "a")
        self.tmux.until(lambda lines: lines[-1].find(INLINE_INFO_SEP) != -1 and
                        lines[-1].split(INLINE_INFO_SEP)[0] == "> abc ")
        self.tmux.send_keys(Key('Enter'))

    @pytest.mark.parametrize('args,index', [
        (['--header', 'hello'], -3),
        (['--inline-info', '--header', 'hello'], -2),
        (['--reverse', '--inline-info', '--header', 'hello'], 1),
        (['--reverse', '--header', 'hello'], 2),
    ])
    def test_header(self, args, index):
        self.command_until(sk_options=args,
                           until_predicate=lambda lines: len(lines) > (index if index > 0 else -index-1) and 'hello' in lines[index])

    @pytest.mark.parametrize('header_lines,extra_args,index,pattern', [
        (1, [], -3, '  a1'),
        (4, [], -5, '  a3'),
        (1, ['--inline-info'], -2, '  a1'),
        (1, ['--reverse', '--inline-info'], 1, '  a1'),
        (1, ['--reverse'], 2, '  a1'),
    ])
    def test_header_lines(self, header_lines, extra_args, index, pattern):
        self.command_until(sk_options=['--header-lines', header_lines] + extra_args,
                           until_predicate=lambda lines: len(lines) > (index if index > 0 else -index-1) and pattern in lines[index])

    @pytest.mark.parametrize('opt', [
        '--extended',
        '--algo=skim_v2',
        '--literal',
        '--no-mouse',
        '--cycle',
        '--hscroll-off=1',
        '--filepath-word',
        '--jump-labels=CHARS',
        '--border',
        '--inline-info',
        '--header=STR',
        '--header-lines=1',
        '--no-bold',
        '--history-size=10',
        '--sync',
        '--no-sort',
        '--select-1',
        '-1',
        '--exit-0',
        '-0'
    ])
    def test_reserved_options(self, opt):
        self.command_until(sk_options=[opt], until_predicate=find_prompt)

    @pytest.mark.parametrize('args,pred', [
        ('--bind=ctrl-a:cancel --bind ctrl-b:cancel', find_prompt),
        ('--expect=ctrl-a --expect=ctrl-v', find_prompt),
        ('--tiebreak=length --tiebreak=score', find_prompt),
        ('--cmd asdf --cmd find', find_prompt),
        ('--query asdf -q xyz', find_prompt),
        ('--delimiter , --delimiter . -d ,', find_prompt),
        ('--nth 1,2 --nth=1,3 -n 1,3', find_prompt),
        ('--with-nth 1,2 --with-nth=1,3', find_prompt),
        ('--color base --color light', find_prompt),
        ('--margin 30% --margin 0', find_prompt),
        ('--min-height 30% --min-height 10', find_prompt),
        ('--height 30% --height 10', find_prompt),
        ('--preview "ls {}" --preview "cat {}"', find_prompt),
        ('--preview-window up --preview-window down', find_prompt),
        ('--multi -m', find_prompt),
        ('--no-multi --no-multi', find_prompt),
        ('--tac --tac', find_prompt),
        ('--ansi --ansi', find_prompt),
        ('--exact -e', find_prompt),
        ('--regex --regex', find_prompt),
        ('--literal --literal', find_prompt),
        ('--no-mouse --no-mouse', find_prompt),
        ('--cycle --cycle', find_prompt),
        ('--no-hscroll --no-hscroll', find_prompt),
        ('--filepath-word --filepath-word', find_prompt),
        ('--border --border', find_prompt),
        ('--inline-info --inline-info', find_prompt),
        ('--no-bold --no-bold', find_prompt),
        ('--print-query --print-query', find_prompt),
        ('--print-cmd --print-cmd', find_prompt),
        ('--print0 --print0', find_prompt),
        ('--sync --sync', find_prompt),
        ('--extended --extended', find_prompt),
        ('--no-sort --no-sort', find_prompt),
        ('--select-1 --select-1', find_prompt),
        ('--exit-0 --exit-0', find_prompt),
        ('--prompt a --prompt b -p c',
         lambda lines: lines[-1].startswith("c")),
        ('-i --cmd-prompt a --cmd-prompt b',
         lambda lines: lines[-1].startswith("b")),
        ('-i --cmd-query asdf --cmd-query xyz',
         lambda lines: lines[-1].startswith("c> xyz")),
        ('--interactive -i', lambda lines: find_prompt(lines, interactive=True)),
        ('--reverse --reverse', lambda lines: find_prompt(lines, reverse=True))
    ])
    def test_multiple_option_values_should_be_accepted(self, args, pred):
        # normally we'll put some default options to SKIM_DEFAULT_OPTIONS and override it in command
        # line. this test will ensure multiple values are accepted.

        self.command_until(sk_options=[args], until_predicate=pred)

    def test_multiple_option_values_should_be_accepted_read0(self):
        self.command_until(
            stdin="echo -e a\\0b", sk_options=['--read0 --read0'], until_predicate=find_prompt)

    def test_single_quote_of_preview_command(self):
        # echo "'\"ABC\"'" | sk --preview="echo X{}X" => X'"ABC"'X
        echo_command = '''echo "'\\"ABC\\"'" | '''
        sk_command = self.sk('--preview=\"echo X{}X\"')
        command = echo_command + sk_command
        self.tmux.send_keys(command, Key('Enter'))
        self.tmux.until(lambda lines: lines.any_include('''X'"ABC"'X'''))

        # echo "'\"ABC\"'" | sk --preview="echo X\{}X" => X{}X
        echo_command = '''echo "'\\"ABC\\"'" | '''
        sk_command = self.sk('--preview=\"echo X\\{}X\"')
        command = echo_command + sk_command
        self.tmux.send_keys(command, Key('Enter'))
        self.tmux.until(lambda lines: lines.any_include('''X{}X'''))

    def test_ansi_and_read0(self):
        """should keep the NULL character, see #142"""
        self.tmux.send_keys(
            f"echo -e 'a\\0b' | {self.sk('--ansi')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.send_keys(Key('Enter'))
        output = ":".join("{:02x}".format(ord(c)) for c in self.readonce())
        assert output.find("61:00:62:0a") >= 0

    def test_smart_case_fuzzy(self):
        """should behave correctly on case, #219"""

        # smart case
        self.tmux.send_keys(f"echo -e 'aBcXyZ' | {self.sk('')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.send_keys(Key('abc'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.send_keys(Ctrl('u'), Key('aBc'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.send_keys(Ctrl('u'), Key('ABc'))
        self.tmux.until(lambda lines: lines.ready_with_matches(0))

    def test_smart_case_exact(self):
        """should behave correctly on case, #219"""

        # smart case
        self.tmux.send_keys(f"echo -e 'aBcXyZ' | {self.sk('')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.send_keys(Key("'abc"))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.send_keys(Ctrl('u'), Key("'aBc"))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.send_keys(Ctrl('u'), Key("'ABc"))
        self.tmux.until(lambda lines: lines.ready_with_matches(0))

    def test_ignore_case_fuzzy(self):
        """should behave correctly on case, #219"""

        # ignore case
        self.tmux.send_keys(
            f"echo -e 'aBcXyZ' | {self.sk('--case ignore')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.send_keys(Key('abc'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.send_keys(Ctrl('u'), Key('aBc'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.send_keys(Ctrl('u'), Key('ABc'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))

    def test_ignore_case_exact(self):
        """should behave correctly on case, #219"""

        # ignore case
        self.tmux.send_keys(
            f"echo -e 'aBcXyZ' | {self.sk('--case ignore')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.send_keys(Key("'abc"))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.send_keys(Ctrl('u'), Key("'aBc"))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.send_keys(Ctrl('u'), Key("'ABc"))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))

    def test_respect_case_fuzzy(self):
        """should behave correctly on case, #219"""

        # respect case
        self.tmux.send_keys(
            f"echo -e 'aBcXyZ' | {self.sk('--case respect')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.send_keys(Key('abc'))
        self.tmux.until(lambda lines: lines.ready_with_matches(0))

    def test_respect_case_exact(self):
        """should behave correctly on case, #219"""

        # respect case
        self.tmux.send_keys(
            f"echo -e 'aBcXyZ' | {self.sk('--case respect')}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.send_keys(Key("'abc"))
        self.tmux.until(lambda lines: lines.ready_with_matches(0))

    def test_query_history(self):
        """query history should work"""

        history_file = f'{self.tempname()}.history'

        self.tmux.send_keys(
            f"echo -e 'a\nb\nc' > {history_file}", Key('Enter'))

        self.tmux.send_keys(
            f"echo -e 'a\nb\nc' | {self.sk('--history', history_file)}", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(3))
        self.tmux.send_keys(Ctrl('p'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.until(lambda lines: lines[-3].startswith('> c'))
        self.tmux.send_keys(Ctrl('p'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.until(lambda lines: lines[-3].startswith('> b'))
        self.tmux.send_keys('b')
        self.tmux.until(lambda lines: lines.ready_with_matches(0))
        self.tmux.send_keys(Ctrl('p'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.until(lambda lines: lines[-3].startswith('> a'))

        self.tmux.send_keys(Ctrl('n'))
        self.tmux.until(lambda lines: lines.ready_with_matches(0))
        self.tmux.until(lambda lines: lines[-1].startswith('> bb'))
        self.tmux.send_keys(Ctrl('n'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.until(lambda lines: lines[-1].startswith('> c'))

        self.tmux.send_keys('d')
        self.tmux.until(lambda lines: lines[-1].startswith('> cd'))
        open_lines = self.tmux.capture()
        self.tmux.send_keys(Key('Enter'))

        self.tmux.until(lambda lines: lines != open_lines)

        with open(history_file, 'r') as f:
            lines = f.readlines()
            assert len(lines) == 4
            assert lines[0].strip() == "a"
            assert lines[1].strip() == "b"
            assert lines[2].strip() == "c"
            assert lines[3].strip() == "cd"

    def test_cmd_history(self):
        """cmd history should work"""

        history_file = f'{self.tempname()}.cmd-history'

        self.tmux.send_keys(
            f"echo -e 'a\nb\nc' > {history_file}", Key('Enter'))
        self.tmux.send_keys(
            f"""{self.sk("-i -c 'echo {}'", '--cmd-history', history_file)}""", Key('Enter'))
        self.tmux.until(lambda lines: lines.ready_with_lines(1))
        self.tmux.send_keys(Ctrl('p'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.until(lambda lines: lines[-1].startswith('c> c'))
        self.tmux.send_keys(Ctrl('p'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.until(lambda lines: lines[-1].startswith('c> b'))
        self.tmux.send_keys('b')
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.send_keys(Ctrl('p'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.until(lambda lines: lines[-1].startswith('c> a'))

        self.tmux.send_keys(Ctrl('n'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.until(lambda lines: lines[-1].startswith('c> bb'))
        self.tmux.send_keys(Ctrl('n'))
        self.tmux.until(lambda lines: lines.ready_with_matches(1))
        self.tmux.until(lambda lines: lines[-1].startswith('c> c'))

        self.tmux.send_keys('d')
        self.tmux.until(lambda lines: lines[-1].startswith('c> cd'))
        open_lines = self.tmux.capture()
        self.tmux.send_keys(Key('Enter'))

        self.tmux.until(lambda lines: lines != open_lines)

        with open(history_file, 'r') as f:
            lines = f.readlines()
            assert len(lines) == 4
            assert lines[0].strip() == "a"
            assert lines[1].strip() == "b"
            assert lines[2].strip() == "c"
            assert lines[3].strip() == "cd"
