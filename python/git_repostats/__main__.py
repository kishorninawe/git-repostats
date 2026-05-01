import os
import subprocess
import sys

EXECUTABLE_NAME = "git-repostats"


def _build_command_args() -> list[str]:
    """Build the command-line arguments for the executable.

    Returns:
        List containing executable name followed by CLI arguments.
    """
    return [EXECUTABLE_NAME, *sys.argv[1:]]


def _run_on_windows(command: list[str]) -> int:
    """Execute the command on Windows using subprocess.

    Handles KeyboardInterrupt gracefully to avoid traceback.

    Args:
        command: Command and arguments to execute.

    Returns:
        Exit code from the executed process.
    """
    try:
        completed_process = subprocess.run(command)
        return completed_process.returncode
    except KeyboardInterrupt:
        # Exit with standard interrupt code
        return 2


def _run_on_unix(command: list[str]) -> None:
    """Execute the command on Unix-like systems using execvp.

    This replaces the current process, so no return occurs.

    Args:
        command: Command and arguments to execute.
    """
    os.execvp(command[0], command)


def main() -> None:
    """Entry point for executing the CLI wrapper.

    Delegates execution based on the operating system:
    - Windows: uses subprocess
    - Unix-like: uses os.execvp for process replacement
    """
    command = _build_command_args()

    if sys.platform == "win32":
        exit_code = _run_on_windows(command)
        sys.exit(exit_code)
    else:
        _run_on_unix(command)


if __name__ == "__main__":
    main()
