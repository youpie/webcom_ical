#!/usr/bin/env python3
import argparse
import os
import subprocess
import json
from pathlib import Path
from datetime import timedelta
from dotenv import dotenv_values
from rich.console import Console
from rich.table import Table
from rich.text import Text

def human_duration(seconds: int) -> str:
    td = timedelta(seconds=seconds)
    weeks, rem = divmod(td.days, 7)
    days = rem
    hours, rem = divmod(td.seconds, 3600)
    minutes = rem // 60
    parts = []
    if weeks:
        parts.append(f"{weeks}w")
    if days:
        parts.append(f"{days}d")
    if hours:
        parts.append(f"{hours}h")
    if minutes:
        parts.append(f"{minutes}m")
    return " ".join(parts) if parts else "0m"

def check_container_up(compose_file: Path) -> bool:
    """Returns True if any container from this compose file is running."""
    try:
        result = subprocess.run(
            ["docker-compose", "-f", str(compose_file), "ps", "--status", "running", "--quiet"],
            capture_output=True,
            text=True,
            check=True,
        )
        return bool(result.stdout.strip())
    except subprocess.CalledProcessError:
        return False

def main(include_hidden: bool):
    console = Console()
    table = Table(title="Application Status Overview", box=None, show_lines=True)
    table.add_column("User", style="bold")
    table.add_column("Container Up?", justify="center")
    table.add_column("State")
    table.add_column("Runs")
    table.add_column("Exec (ms)")
    table.add_column("Shifts")
    table.add_column("Broken Shifts")
    table.add_column("Cal Ver")
    table.add_column("Window")

    failures = []

    root = Path(".")
    for d in sorted(root.iterdir()):
        if not d.is_dir():
            continue
        name = d.name
        if name.startswith("_") and not include_hidden:
            continue

        compose = d / "docker-compose.yml"
        kuma_dir = d / "kuma"
        logbook = kuma_dir / "logbook.json"
        envfile = d / ".env"

        if not (compose.exists() and logbook.exists() and envfile.exists()):
            continue

        # read username
        uname = (kuma_dir / "name").read_text().strip() if (kuma_dir / "name").exists() else name

        # parse logbook
        data = json.loads(logbook.read_text())
        state = data.get("state", "Unknown")
        rc = data.get("repeat_count", 0)
        app = data.get("application_state", {})
        exec_ms = app.get("execution_time_ms", 0)
        shifts = app.get("shifts", 0)
        broken = app.get("broken_shifts", 0)
        calver = app.get("calendar_version", "")

        # parse interval
        env = dotenv_values(envfile)
        parse_int = int(env.get("PARSE_INTERVAL", 0))
        total_seconds = parse_int * rc
        window = human_duration(total_seconds)

        # container status
        up = check_container_up(compose)
        up_str = "[green]✔[/]" if up else "[red]✖[/]"

        # color state
        if state.upper() == "OK":
            state_text = Text(state, style="green")
        else:
            state_text = Text(state, style="bold red")
            failures.append(uname)

        table.add_row(
            uname,
            up_str,
            state_text,
            str(rc),
            str(exec_ms),
            str(shifts),
            str(broken),
            calver,
            window,
        )

    console.print(table)

    if failures:
        console.print("\n[bold red]Failing applications:[/]")
        for f in failures:
            console.print(f" • {f}")
    else:
        console.print("\n[bold green]All applications are OK![/]")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Check all user apps: container up, logbook state, and computed window."
    )
    parser.add_argument(
        "-a", "--include-hidden",
        action="store_true",
        help="also scan directories beginning with '_'"
    )
    args = parser.parse_args()
    main(include_hidden=args.include_hidden)
