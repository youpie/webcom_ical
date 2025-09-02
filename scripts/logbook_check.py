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

def main(include_hidden: bool, only_failed: bool, single_user: bool, condensed: bool):
    console = Console()
    table = Table(box=None, show_lines=True)
    table.add_column("User", style="bold")
    table.add_column("Up?", justify="center")
    table.add_column("State")
    table.add_column("Since")
    table.add_column("Runs", justify="right")
    table.add_column("Exec (s)", justify="right")
    table.add_column("Shifts", justify="right")
    table.add_column("Broken", justify="right")
    table.add_column("BrokenF", justify="right")
    table.add_column("Failed", justify="right")
    table.add_column("Old", justify="right")
    table.add_column("Minute", justify="right")
    table.add_column("CalVer")
    table.add_column("Last Run")
    table.add_column("Folder Name")

    failures = []

    root = Path(".")
    if not single_user: 
        for d in sorted(root.iterdir()):
            if not d.is_dir() or (d.name.startswith("_") and not include_hidden or not (d/"docker-compose.yml").exists()):
                continue
            get_user(d,table,failures,only_failed,False if not condensed else True)
    else:
        get_user(Path().resolve(),table,failures,only_failed,False)
    if not condensed:
        console.print(table)
        console.print("\n")

    if not single_user:
        if failures:
            console.print("[bold red]Failed Users:[/]")
            for f in failures:
                console.print(f" • {f}")
        else:
            console.print("[bold green]All Users are OK![/]")

def get_user(path, table, failures, only_failed, skip_docker):
    compose = path / "docker-compose.yml"
    kuma = path / "kuma"
    logbook = kuma / "logbook.json"
    previous_execution_date = kuma / "previous_execution_date"
    envfile = path / ".env"

    # user’s display name
    uname = (kuma / "name").read_text().strip() if (kuma / "name").exists() else path.name
    execution_minute = (kuma / "starting_minute").read_text().strip() if (kuma / "starting_minute").exists() else "-"
    # container status
    if not skip_docker:
        up = check_container_up(compose) if compose.exists() else False
        up_str = "[green]✔[/]" if up else "[red]✖[/]"
    else:
        up_str = ""
    # defaults if no logbook
    state = "–"
    rc = exec_s = shifts = broken = 0
    calver = "–"
    window = "–"
    last_run = "–"

    # last run from file mtime
    ts = 0.0
    long_offline = False
    last_run = ""
    if logbook.exists():
        ts = logbook.stat().st_mtime
    elif previous_execution_date.exists():
        ts = previous_execution_date.stat().st_mtime
        last_run += "[orange1]"
    if ts != 0.0:
        last_run_time = datetime.fromtimestamp(ts)
        if last_run_time + timedelta(days=2) < datetime.now():
            long_offline = True
        last_run += "[blink][red]" if long_offline else ""
        last_run += last_run_time.strftime("%Y-%m-%d %H:%M")
    

    if logbook.exists() and envfile.exists():
        
        data = json.loads(logbook.read_text())
        raw_state = data.get("state", "Unknown")
        state = normalize_state(raw_state)
        rc = data.get("repeat_count", 0)
        app = data.get("application_state", {})
        exec_s = round(app.get("execution_time_ms", 0)/1000,1)
        shifts = app.get("shifts", 0)
        broken = app.get("broken_shifts", 0)
        failed_broken = app.get("failed_broken_shifts", 0)
        failed_shifts = app.get("failed_shifts", 0)
        non_relevant_shifts = app.get("non_relevant_shifts", 0)
        calver = app.get("calendar_version", "–")
        env = dotenv_values(envfile)
        parse_int = int(env.get("KUMA_HEARTBEAT_INTERVAL", 4001))
        window = ""
        if parse_int == 4001:
            window += "[orange1]"
        window += human_duration(parse_int * rc)
    failed = False
    # colorize state
    if state.upper() == "OK":
        state_text = Text(state, style="green")
    elif state != "–" or long_offline:
        failed = True
        state_text = Text(state, style="bold red")
        text = "[red][blink]" if long_offline else ""
        text += f"{uname}"
        text += f" - {path.name}" if path.name != uname else ""
        text += f"  ~ {state_text}" if skip_docker and logbook.exists() else ""
        failures.append(text)
    else:
        state_text = Text(state, style="dim")
    if failed or not only_failed:
        table.add_row(
            uname,
            up_str,
            state_text,
            window,
            str(rc) if logbook.exists() else "–",
            str(exec_s) if logbook.exists() else "–",
            str(shifts) if logbook.exists() else "–",
            str(broken) if logbook.exists() else "–",
            str(failed_broken) if logbook.exists() else "–",
            str(failed_shifts) if logbook.exists() else "–",
            str(non_relevant_shifts) if logbook.exists() else "–",
            execution_minute,
            calver,
            last_run,
            path.name
        )

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
    parser.add_argument(
        "-s", "--single-user",
        action="store_true",
        help="only execute for the current directory"
    )
    parser.add_argument(
        "-c", "--condensed",
        action="store_true",
        help="Only show failing applications"
    )
    args = parser.parse_args()
    main(include_hidden=args.include_hidden, only_failed=args.only_failed, single_user=args.single_user, condensed=args.condensed)
