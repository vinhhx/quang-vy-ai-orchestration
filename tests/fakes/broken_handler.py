# Test fixture: a handler module whose own import is broken (see tests/test_robustness.py).
import brain_nonexistent_dependency_for_tests  # noqa: F401


def cmd_broken(args):
    return 0
