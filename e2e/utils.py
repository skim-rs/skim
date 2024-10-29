import time

from e2e.config import DEFAULT_TIMEOUT

def now_mills():
    return int(round(time.time() * 1000))


def wait(func, timeout_handler=None, timeout=DEFAULT_TIMEOUT):
    since = now_mills()
    while now_mills() - since < timeout:
        time.sleep(0.02)
        ret = func()
        if ret is not None and ret:
            return
    if timeout_handler is not None:
        timeout_handler()
    raise TimeoutError



def find_prompt(lines, interactive=False, reverse=False):
    linen = -1
    prompt = ">"
    if interactive:
        prompt = "c>"
    if reverse:
        linen = 0
    return lines[linen].startswith(prompt)
