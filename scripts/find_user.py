#!/usr/bin/env python3
import os
import sys
import tempfile
from pathlib import Path

BASE_DIR = Path.home() / "Services" / "Webcom"
TMP_FILE = Path(tempfile.gettempdir()) / "kuma-find.tmp"

def parse_env(path):
    vars = {}
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k,v = line.split("=",1)
        vars[k.strip()] = v.strip().strip('"').strip("'")
    return vars

def collect_env_maps(base):
    email_map = {}
    user_map = {}
    for folder in base.iterdir():
        if not folder.is_dir(): continue
        env_file = folder / ".env"
        if not env_file.exists(): continue
        env = parse_env(env_file)
        if "MAIL_TO" in env:
            email_map[str(folder)] = [e.strip() for e in env["MAIL_TO"].split(",")]
        if "USERNAME" in env:
            user_map[str(folder)] = env["USERNAME"]
    return email_map, user_map

def collect_name_map(base):
    nm = {}
    for dirpath, dirnames, filenames in os.walk(base, topdown=True):
        # prune any paths deeper than 3 levels under BASE_DIR
        rel = Path(dirpath).relative_to(base)
        if len(rel.parts) > 3:
            dirnames[:] = []
            continue
        if "kuma" in dirnames:
            kuma_dir = Path(dirpath) / "kuma"
            name_file = kuma_dir / "name"
            if name_file.exists():
                folder = Path(dirpath)
                nm[str(folder)] = name_file.read_text().strip()
    return nm

def write_tmp(result):
    if result:
        TMP_FILE.write_text(result)
    else:
        # remove if exists
        try: TMP_FILE.unlink()
        except: pass

def main():
    if len(sys.argv) < 2:
        print("Usage: kuma_find.py \"search\"")
        sys.exit(1)

    q = sys.argv[1]
    lc = q.lower()

    email_map, user_map = collect_env_maps(BASE_DIR)
    name_map = collect_name_map(BASE_DIR)

    # pick which map to search
    matches = []
    if "@" in q:
        # email search
        for folder, emails in email_map.items():
            if any(e.lower() == lc for e in emails):
                matches.append(folder)
    elif q and q[0].isdigit():
        # username search
        for folder, user in user_map.items():
            if user.lower() == lc:
                matches.append(folder)
    else:
        # title‐case name search
        for folder, name in name_map.items():
            if name.lower() == lc:
                matches.append(folder)

    # helper to auto-ignore underscore
    def pick_two(ms):
        us = [d for d in ms if Path(d).name.startswith("_")]
        non = [d for d in ms if not Path(d).name.startswith("_")]
        if len(ms)==2 and len(us)==1 and len(non)==1:
            return non[0]
        return None

    folder = None
    cnt = len(matches)
    if cnt == 1:
        folder = matches[0]
        print(f"Found: {folder}")
    elif cnt == 2:
        auto = pick_two(matches)
        if auto:
            print(f"Ignored (leading underscore): { [d for d in matches if d!=auto][0] }")
            print(f"Auto‐selected: {auto}")
            folder = auto
    elif cnt > 2:
        print(f"Multiple matches for “{q}”:")
        for i,d in enumerate(matches,1):
            print(f"  {i}) {d}")
        try:
            choice = int(input(f"Choose [1-{cnt}]: "))
            if 1 <= choice <= cnt:
                folder = matches[choice-1]
                print(f"You picked: {folder}")
        except:
            print("Invalid choice – aborting.")
    else:
        print(f"No exact match for “{q}”.")
        # skip levenshtein for brevity
        # could implement fallback here

    # write to temp and exit
    if folder:
        write_tmp(folder)
        sys.exit(0)
    else:
        write_tmp("")  # clear
        sys.exit(1)

if __name__=="__main__":
    main()
