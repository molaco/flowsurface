#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "python-dotenv",
# ]
# ///

import json
import os
import sys
import subprocess
from pathlib import Path
from datetime import datetime

try:
    from dotenv import load_dotenv

    load_dotenv()
except ImportError:
    pass  # dotenv is optional


def get_session_data(session_id):
    """Get session data including agent name, prompts, and extras."""
    session_file = Path(f".claude/data/sessions/{session_id}.json")

    if not session_file.exists():
        return None, f"Session file {session_file} does not exist"

    try:
        with open(session_file, "r") as f:
            session_data = json.load(f)
            return session_data, None
    except Exception as e:
        return None, f"Error reading session file: {str(e)}"


def debug_log(message, data=None):
    """Log debug messages to a separate debug file."""
    debug_dir = Path("logs")
    debug_dir.mkdir(parents=True, exist_ok=True)
    debug_file = debug_dir / "statusline_debug.log"

    timestamp = datetime.now().isoformat()
    with open(debug_file, "a") as f:
        f.write(f"[{timestamp}] {message}\n")
        if data is not None:
            f.write(f"[{timestamp}] Data: {json.dumps(data, indent=2)}\n")
        f.write("\n")


def log_status_line(input_data, status_line_output, token_usage=None):
    """Log status line event to logs directory."""
    # Ensure logs directory exists
    log_dir = Path("logs")
    log_dir.mkdir(parents=True, exist_ok=True)
    log_file = log_dir / "status_line.json"

    # Read existing log data or initialize empty list
    if log_file.exists():
        with open(log_file, "r") as f:
            try:
                log_data = json.load(f)
            except (json.JSONDecodeError, ValueError):
                log_data = []
    else:
        log_data = []

    # Create log entry with input data and generated output
    log_entry = {
        "timestamp": datetime.now().isoformat(),
        "input_data": input_data,
        "status_line_output": status_line_output,
    }

    # Add token usage if available
    if token_usage:
        log_entry["token_usage"] = token_usage

    # Append the log entry
    log_data.append(log_entry)

    # Write back to file with formatting
    with open(log_file, "w") as f:
        json.dump(log_data, f, indent=2)


def get_git_branch():
    """Get current git branch if in a git repository."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True,
            text=True,
            timeout=2,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except Exception:
        pass
    return None


def get_git_status():
    """Get git status indicators."""
    try:
        # Check if there are uncommitted changes
        result = subprocess.run(
            ["git", "status", "--porcelain"], capture_output=True, text=True, timeout=2
        )
        if result.returncode == 0:
            changes = result.stdout.strip()
            if changes:
                lines = changes.split("\n")
                return f"±{len(lines)}"
    except Exception:
        pass
    return ""


def get_token_usage_from_transcript(transcript_path):
    """Get cumulative usage data from Claude Code transcript file."""
    try:
        if not Path(transcript_path).exists():
            return None, f"Transcript file {transcript_path} does not exist"

        # Parse JSONL file and sum all assistant message usage
        total_usage = {
            "input_tokens": 0,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 0,
            "output_tokens": 0,
            "ephemeral_5m_input_tokens": 0,
            "ephemeral_1h_input_tokens": 0,
            "service_tier": "unknown",
            "usage_count": 0,
        }
        
        debug_log(f"Starting token summation from transcript: {transcript_path}")

        with open(transcript_path, "r") as f:
            for line in f:
                try:
                    entry = json.loads(line.strip())
                    # Look for assistant messages with usage data
                    if (
                        entry.get("type") == "assistant"
                        and "message" in entry
                        and "usage" in entry.get("message", {})
                    ):
                        usage = entry["message"]["usage"]

                        # Sum up all the usage fields
                        total_usage["input_tokens"] += usage.get("input_tokens", 0)
                        total_usage["cache_creation_input_tokens"] += usage.get(
                            "cache_creation_input_tokens", 0
                        )
                        total_usage["cache_read_input_tokens"] += usage.get(
                            "cache_read_input_tokens", 0
                        )
                        total_usage["output_tokens"] += usage.get("output_tokens", 0)

                        # Handle ephemeral cache tokens
                        cache_creation = usage.get("cache_creation", {})
                        total_usage["ephemeral_5m_input_tokens"] += cache_creation.get(
                            "ephemeral_5m_input_tokens", 0
                        )
                        total_usage["ephemeral_1h_input_tokens"] += cache_creation.get(
                            "ephemeral_1h_input_tokens", 0
                        )

                        # Keep track of service tier (use latest)
                        total_usage["service_tier"] = usage.get(
                            "service_tier", "unknown"
                        )
                        total_usage["usage_count"] += 1

                except json.JSONDecodeError:
                    continue

        if total_usage["usage_count"] == 0:
            return None, "No usage data found in transcript"

        debug_log(f"Found {total_usage['usage_count']} assistant messages with usage data")
        debug_log("Final token totals before returning:", {
            "input_tokens": total_usage["input_tokens"],
            "cache_creation_input_tokens": total_usage["cache_creation_input_tokens"], 
            "cache_read_input_tokens": total_usage["cache_read_input_tokens"],
            "output_tokens": total_usage["output_tokens"]
        })

        # Remove the usage_count from the final result
        del total_usage["usage_count"]
        return total_usage, None

    except Exception as e:
        return None, f"Error reading transcript file: {str(e)}"


def get_token_usage(session):
    """Get usage data for a Claude Code session."""

    usage = session.get("usage")
    if not usage:
        return None, "No usage data found in session"

    # Extract all the usage fields
    result = {
        "input_tokens": usage.get("input_tokens", 0),
        "cache_creation_input_tokens": usage.get("cache_creation_input_tokens", 0),
        "cache_read_input_tokens": usage.get("cache_read_input_tokens", 0),
        "output_tokens": usage.get("output_tokens", 0),
        "service_tier": usage.get("service_tier", "unknown"),
    }

    # Handle ephemeral cache tokens
    cache_creation = usage.get("cache_creation", {})
    result["ephemeral_5m_input_tokens"] = cache_creation.get(
        "ephemeral_5m_input_tokens", 0
    )
    result["ephemeral_1h_input_tokens"] = cache_creation.get(
        "ephemeral_1h_input_tokens", 0
    )

    return result, None


def format_token_display(token_usage):
    """Format token usage for display in status line with differentiated token types."""
    if not token_usage:
        return ""

    input_tokens = token_usage.get("input_tokens", 0)
    output_tokens = token_usage.get("output_tokens", 0)
    cache_create = token_usage.get("cache_creation_input_tokens", 0)
    cache_read = token_usage.get("cache_read_input_tokens", 0)

    # Format with thousands separators for readability
    def format_count(count):
        return f"{count:,}" if count > 0 else "0"

    # Create tabular display format similar to: 108 | 707 | 353,040 | 3,608,253
    token_display = f"🪙 In:{format_count(input_tokens)} | Out:{format_count(output_tokens)} | C+:{format_count(cache_create)} | C:{format_count(cache_read)}"

    return token_display


def generate_status_line(input_data, token_usage=None):
    """Generate the status line based on input data."""
    parts = []

    # Model display name
    model_info = input_data.get("model", {})
    model_name = model_info.get("display_name", "Claude")
    parts.append(f"\033[36m[{model_name}]\033[0m")  # Cyan color

    # Current directory
    workspace = input_data.get("workspace", {})
    current_dir = workspace.get("current_dir", "")
    if current_dir:
        dir_name = os.path.basename(current_dir)
        parts.append(f"\033[34m📁 {dir_name}\033[0m")  # Blue color

    # Git branch and status
    git_branch = get_git_branch()
    if git_branch:
        git_status = get_git_status()
        git_info = f"🌿 {git_branch}"
        if git_status:
            git_info += f" {git_status}"
        parts.append(f"\033[32m{git_info}\033[0m")  # Green color

    # Token usage (if available)
    token_display = format_token_display(token_usage)
    if token_display:
        parts.append(f"\033[33m{token_display}\033[0m")  # Yellow color

    # Version info (optional, smaller)
    version = input_data.get("version", "")
    if version:
        parts.append(f"\033[90mv{version}\033[0m")  # Gray color

    return " | ".join(parts)


def main():
    try:
        # Read JSON input from stdin
        input_data = json.loads(sys.stdin.read())
        debug_log("Starting statusline processing", input_data)

        # Extract session ID from input data
        session_id = input_data.get("session_id", "unknown")
        debug_log(f"Session ID: {session_id}")

        # Get session data and token usage
        session_data, error = get_session_data(session_id)
        debug_log(f"Session data retrieval - Error: {error}")

        if session_data:
            debug_log("Session data found", session_data)
        else:
            debug_log("No session data found")

        token_usage = None

        # Try to get token usage from transcript file first (preferred method)
        transcript_path = input_data.get("transcript_path")
        if transcript_path:
            token_usage, token_error = get_token_usage_from_transcript(transcript_path)
            debug_log(f"Token usage from transcript - Error: {token_error}")
            if token_usage:
                debug_log("Token usage found from transcript", token_usage)
            else:
                debug_log("No token usage found in transcript")

        # Fallback to session data if transcript didn't work
        if not token_usage and not error and session_data:
            token_usage, token_error = get_token_usage(session_data)
            debug_log(f"Token usage from session - Error: {token_error}")
            if token_usage:
                debug_log("Token usage found from session", token_usage)
            else:
                debug_log("No token usage found in session")

        # Generate status line with token info
        status_line = generate_status_line(input_data, token_usage)
        debug_log(f"Generated status line: {status_line}")

        # Log the status line event with token data
        log_status_line(input_data, status_line, token_usage)

        # Output the status line (first line of stdout becomes the status line)
        print(status_line)

        # Success
        sys.exit(0)

    except json.JSONDecodeError as e:
        debug_log(f"JSON decode error: {str(e)}")
        # Handle JSON decode errors gracefully - output basic status
        print("\033[31m[Claude] 📁 Unknown\033[0m")
        sys.exit(0)
    except Exception as e:
        debug_log(f"Unexpected error: {str(e)}")
        # Handle any other errors gracefully - output basic status
        print("\033[31m[Claude] 📁 Error\033[0m")
        sys.exit(0)


if __name__ == "__main__":
    main()
