import os


DEFAULT_TIMEOUT = 500

SCRIPT_PATH = os.path.realpath(__file__)
BASE = os.path.expanduser(os.path.join(os.path.dirname(SCRIPT_PATH), '..'))
SK = f"SKIM_DEFAULT_OPTIONS= SKIM_DEFAULT_COMMAND= cargo run --release --"
