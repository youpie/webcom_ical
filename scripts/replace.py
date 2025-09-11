#!/usr/bin/env python3
######
#
# Geschreven met AI!
#
######
import os
import shutil
import argparse

def main():
    parser = argparse.ArgumentParser(description="Replace a file in subfolders (depth=1)")
    parser.add_argument("filename", help="Name of the file to replace (e.g., docker-compose.yml)")
    parser.add_argument("source", help="Path to the source file to copy from")
    parser.add_argument(
        "-e", "--exclude", nargs="*", default=[], 
        help="Folders to exclude (space-separated)"
    )
    parser.add_argument(
        "-o", "--only", nargs="*", default=[],
        help="Only process these folders (space-separated)"
    )

    args = parser.parse_args()

    cwd = os.getcwd()

    if not os.path.isfile(args.source):
        print(f"❌ Source file {args.source} does not exist.")
        return

    for entry in os.listdir(cwd):
        folder = os.path.join(cwd, entry)

        if not os.path.isdir(folder):
            continue  # only subfolders

        # Apply exclusions/inclusions
        if args.only and entry not in args.only:
            continue
        if entry in args.exclude:
            continue

        target_file = os.path.join(folder, args.filename)

        if os.path.isfile(target_file):
            print(f"📄 Replacing {target_file}")
            shutil.copy2(args.source, target_file)
        else:
            print(f"⚠️ Skipping {folder}, no {args.filename}")

if __name__ == "__main__":
    main()
