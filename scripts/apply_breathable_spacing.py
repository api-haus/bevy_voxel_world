#!/usr/bin/env python3
import re
import sys
from pathlib import Path

ROOT = Path("/Volumes/Archive2TB/_dev/bevister")


def ensure_single_blank_between_items(lines):
    """Ensure 1 blank line between top-level items and import/mod blocks.

    Conservative: does not reorder, only inserts a single blank line where an
    item starts on the next line without a separating blank line. Keeps attrs
    (#[...]) and doc comments attached to following item.
    """
    out = []

    item_start_re = re.compile(r"^(?:pub\s+)?(?:struct|enum|trait|impl|fn)\b")
    use_mod_re = re.compile(r"^(?:pub\s+)?(?:use|mod)\b")

    n = len(lines)
    for i, line in enumerate(lines):
        out.append(line)

        if i + 1 >= n:
            continue

        next_line = lines[i + 1]
        if item_start_re.match(next_line) or use_mod_re.match(next_line):
            # Skip if current line already blank
            if line.strip() == "":
                continue

            # Avoid splitting attributes/doc comments from their items
            if line.strip().startswith(("#[", "//!", "///")):
                continue

            out.append("\n")

    return out


def group_import_blocks(lines):
    """Insert a blank line around contiguous use/mod blocks to separate them
    from surrounding code. Do not reorder imports or merge blocks.
    """
    use_re = re.compile(r"^(?:pub\s+)?use\s+")
    mod_re = re.compile(r"^(?:pub\s+)?mod\s+")

    out = []
    i = 0
    n = len(lines)
    while i < n:
        # Detect a contiguous block of use/mod lines
        if use_re.match(lines[i]) or mod_re.match(lines[i]):
            if out and out[-1].strip() != "":
                out.append("\n")

            while i < n and (use_re.match(lines[i]) or mod_re.match(lines[i])):
                out.append(lines[i])
                i += 1

            # Add a single blank line after the block if following content
            if i < n and lines[i].strip() != "":
                out.append("\n")
            continue

        out.append(lines[i])
        i += 1

    return out


def inside_fn_blank_lines(lines):
    """Add blank lines between logical paragraphs inside blocks.

    Heuristics:
    - Add a blank line before a control-flow starter (if/match/for/while/loop/return)
      when previous line is non-blank code.
    - Add a blank line after a closing brace '}' when more statements follow.
    """
    out = []
    brace_depth = 0

    for i, line in enumerate(lines):
        stripped = line.strip()
        prev_depth = brace_depth
        # Update depth after using prev_depth for decisions
        brace_depth += line.count("{") - line.count("}")

        out.append(line)

        if i + 1 < len(lines):
            nxt = lines[i + 1]
            nxt_s = nxt.strip()

            if prev_depth > 0:
                starts_control = nxt_s.startswith(
                    ("if ", "if(", "match ", "for ", "while ", "loop", "return ")
                )
                if starts_control and stripped != "" and not stripped.startswith("//"):
                    if out and out[-1].strip() != "":
                        out.append("\n")

        # After closing a block, if more work follows on next non-blank line, add space
        if stripped.endswith("}") and prev_depth > 0:
            j = i + 1
            while j < len(lines) and lines[j].strip() == "":
                j += 1
            if j < len(lines):
                nxt_s = lines[j].strip()
                if nxt_s and not nxt_s.startswith(("}", ")", ",", "//")):
                    if out and out[-1].strip() != "":
                        out.append("\n")

    return out


def plugin_chain_spacing(lines):
    """Insert blank lines between conceptual phases in Bevy Plugin::build chains.

    Boundaries we try to separate (heuristic):
    - resources/events (init/insert/add_event) -> configure_sets
    - configure_sets -> add_systems(Startup,
    - add_systems(Startup, ...) -> add_systems(Update, ...)
    """

    def is_resource_line(s: str) -> bool:
        return any(
            k in s
            for k in (
                ".init_resource(",
                ".insert_resource(",
                ".add_event<",
                ".add_event(",
            )
        )

    def is_configure_sets_line(s: str) -> bool:
        return ".configure_sets(" in s

    def is_add_startup_line(s: str) -> bool:
        return ".add_systems(Startup" in s

    def is_add_update_line(s: str) -> bool:
        return ".add_systems(Update" in s

    out = []
    prev_nonblank = ""
    for i, line in enumerate(lines):
        s = line.strip()
        if s:
            # Insert blank between phases
            if (
                is_configure_sets_line(s)
                and prev_nonblank
                and is_resource_line(prev_nonblank)
            ):
                if out and out[-1].strip() != "":
                    out.append("\n")
            elif (
                is_add_startup_line(s)
                and prev_nonblank
                and (
                    is_resource_line(prev_nonblank)
                    or is_configure_sets_line(prev_nonblank)
                )
            ):
                if out and out[-1].strip() != "":
                    out.append("\n")
            elif (
                is_add_update_line(s)
                and prev_nonblank
                and (
                    is_add_startup_line(prev_nonblank)
                    or is_configure_sets_line(prev_nonblank)
                    or is_resource_line(prev_nonblank)
                )
            ):
                if out and out[-1].strip() != "":
                    out.append("\n")

            prev_nonblank = s

        out.append(line)

    return out


def cap_consecutive_blank_lines(lines, max_blanks=2):
    out = []
    blank_run = 0
    for line in lines:
        if line.strip() == "":
            blank_run += 1
            if blank_run <= max_blanks:
                out.append("\n")
        else:
            blank_run = 0
            out.append(line)
    return out


def process_file(path: Path):
    txt = path.read_text(encoding="utf-8")
    lines = txt.splitlines(keepends=True)

    lines = group_import_blocks(lines)
    lines = ensure_single_blank_between_items(lines)
    lines = plugin_chain_spacing(lines)
    lines = inside_fn_blank_lines(lines)
    lines = cap_consecutive_blank_lines(lines, 2)

    new_txt = "".join(lines)
    if new_txt != txt:
        path.write_text(new_txt, encoding="utf-8")
        return True
    return False


def main():
    changed = 0
    for p in ROOT.glob("crates/**/*.rs"):
        try:
            if process_file(p):
                changed += 1
        except Exception as e:
            print(f"Warning: failed to process {p}: {e}", file=sys.stderr)
    print(f"Files changed: {changed}")


if __name__ == "__main__":
    main()
