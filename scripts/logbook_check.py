#!/usr/bin/env python3
import argparse
import subprocess
import json
from pathlib import Path
from datetime import datetime, timedelta
from dotenv import dotenv_values
from rich.console import Console
from rich.table import Table
from rich.text import Text

def human_duration(seconds: int) -> str:
    td = timedelta(seconds=seconds)
    weeks, rem_days = divmod(td.days, 7)
    days = rem_days
    hours, rem_secs = divmod(td.seconds, 3600)
    minutes = rem_secs // 60
    parts = []
    if weeks:
        parts.append(f"{weeks}w")
    if days:
        parts.append(f"{days}d")
    if hours:
        parts.append(f"{hours}h")
    if minutes:
        parts.append(f"{minutes}m")
    return " ".join(parts) or "0m"

def check_container_up(compose_file: Path) -> bool:
    try:
        result = subprocess.run(
            ["docker", "compose", "-f", str(compose_file), "ps", "--status", "running", "--quiet"],
            capture_output=True, text=True, check=True
        )
        return bool(result.stdout.strip())
    except subprocess.CalledProcessError:
        return False

def normalize_state(raw_state):
    if isinstance(raw_state, str):
        return raw_state
    if isinstance(raw_state, dict):
        key, val = next(iter(raw_state.items()))
        return f"{key}: {val}"
    return str(raw_state)

def main(include_hidden: bool, only_failed: bool):
    console = Console()
    table = Table(title="Application Status Overview", box=None, show_lines=True)
    table.add_column("User", style="bold")
    table.add_column("Up?", justify="center")
    table.add_column("State")
    table.add_column("Runs", justify="right")
    table.add_column("Exec (s)", justify="right")
    table.add_column("Shifts", justify="right")
    table.add_column("Broken", justify="right")
    table.add_column("CalVer")
    table.add_column("Window")
    table.add_column("Last Run")
    table.add_column("Folder Name")

    failures = []

    root = Path(".")
    for d in sorted(root.iterdir()):
        if not d.is_dir() or (d.name.startswith("_") and not include_hidden or not (d/".env").exists()):
            continue
        compose = d / "docker-compose.yml"
        kuma = d / "kuma"
        logbook = kuma / "logbook.json"
        envfile = d / ".env"

        # user’s display name
        uname = (kuma / "name").read_text().strip() if (kuma / "name").exists() else d.name

        # container status
        up = check_container_up(compose) if compose.exists() else False
        up_str = "[green]✔[/]" if up else "[red]✖[/]"

        # defaults if no logbook
        state = "–"
        rc = exec_s = shifts = broken = 0
        calver = "–"
        window = "–"
        last_run = "–"

        if logbook.exists() and envfile.exists():
            # last run from file mtime
            ts = logbook.stat().st_mtime
            last_run = datetime.fromtimestamp(ts).strftime("%Y-%m-%d %H:%M")

            data = json.loads(logbook.read_text())
            raw_state = data.get("state", "Unknown")
            state = normalize_state(raw_state)
            rc = data.get("repeat_count", 0)
            app = data.get("application_state", {})
            exec_s = round(app.get("execution_time_ms", 0)/1000,1)
            shifts = app.get("shifts", 0)
            broken = app.get("broken_shifts", 0)
            calver = app.get("calendar_version", "–")

            env = dotenv_values(envfile)
            parse_int = int(env.get("PARSE_INTERVAL", 0))
            window = human_duration(parse_int * rc)
        failed = False
        # colorize state
        if state.upper() == "OK":
            state_text = Text(state, style="green")
        elif state != "–":
            failed = True
            state_text = Text(state, style="bold red")
            failures.append(f"{uname} - {d.name} ")
        else:
            state_text = Text(state, style="dim")
        if failed or not only_failed:
            table.add_row(
                uname,
                up_str,
                state_text,
                str(rc) if logbook.exists() else "–",
                str(exec_s) if logbook.exists() else "–",
                str(shifts) if logbook.exists() else "–",
                str(broken) if logbook.exists() else "–",
                calver,
                window,
                last_run,
                d.name
            )

    console.print(table)
    if failures:
        console.print("\n[bold red]Failing applications:[/]")
        for f in failures:
            console.print(f" • {f}")
    else:
        console.print("\n[bold green]All applications are OK![/]")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(
        description="Check all user apps: container up, logbook state, and computed window."
    )
    parser.add_argument(
        "-a", "--include-hidden",
        action="store_true",
        help="also scan directories beginning with '_'"
    )
    parser.add_argument(
        "-f", "--only-failed",
        action="store_true",
        help="Only show failed dirs"
    )
    args = parser.parse_args()
    main(include_hidden=args.include_hidden, only_failed=args.only_failed)
