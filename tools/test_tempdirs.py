"""Scratch directories which inherit the writable parent's Windows permissions.

Python 3.12's mode-0700 TemporaryDirectory creates a DACL a restricted Windows token
cannot re-enter. These test artifacts need inherited permissions, not a private DACL.
"""
import shutil
import tempfile
import uuid
from pathlib import Path


class TemporaryDirectory:
    def __init__(self, *, dir=None):
        self.root = Path(dir or tempfile.gettempdir()).resolve()
        self.path = self.root / ('jac-test-' + uuid.uuid4().hex)
        self.path.mkdir(mode=0o777)
        self.name = str(self.path)

    def __enter__(self):
        return self.name

    def cleanup(self):
        target = self.path.resolve()
        if target.parent != self.root or not target.name.startswith('jac-test-'):
            raise ValueError('Test cleanup escaped its scratch root')
        if target.exists():
            shutil.rmtree(target)

    def __exit__(self, *args):
        self.cleanup()
