from __future__ import annotations

EXIT_OK = 0
EXIT_ERROR = 1
EXIT_ROUTING = 2
EXIT_PROTOCOL = 3
EXIT_BLOCKED = 4
EXIT_INTERRUPTED = 130


class BrainError(Exception):
    def __init__(self, message: str, code: int = EXIT_ERROR):
        super().__init__(message)
        self.message = message
        self.code = code
