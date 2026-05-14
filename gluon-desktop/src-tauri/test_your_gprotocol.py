#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Quick parser test to analyze the indentation issue
"""
import sys
sys.stdout.reconfigure(encoding='utf-8')

# Let's analyze the G-PROTOCOL you provided
gprotocol = '''<gluon_patch>
  <file path="gluon-desktop/src-tauri/src/simulation_test.py">
    <change>
      <search>
    """
    logger.info(f"[RecalculateQueueThread] Rozpoczęto zadanie przeliczania cen w tle. Queue ID: {queue_id}")
    try:
        from documents.models import TransportOffer
      </search>
      <replace>
    """
    thread_name = threading.current_thread().name
    logger.info(f"[RecalculateQueueThread] Rozpoczęto zadanie przeliczania cen w tle ({thread_name}). Queue ID: {queue_id}")
    try:
        from documents.models import TransportOffer
      </replace>
    </change>
  </file>
</gluon_patch>'''

print("=" * 80)
print("ANALYZING INDENTATION IN YOUR G-PROTOCOL")
print("=" * 80)

# Extract search block
import re
search_match = re.search(r'<search>\s*(.*?)\s*</search>', gprotocol, re.DOTALL)
replace_match = re.search(r'<replace>\s*(.*?)\s*</replace>', gprotocol, re.DOTALL)

if search_match and replace_match:
    search_code = search_match.group(1)
    replace_code = replace_match.group(1)

    print("\n📋 SEARCH BLOCK (as AI generated it):")
    print("-" * 80)
    for i, line in enumerate(search_code.split('\n'), 1):
        spaces = len(line) - len(line.lstrip())
        print(f"Line {i:2d} (indent={spaces:2d}): |{line}|")

    print("\n📋 REPLACE BLOCK (as AI generated it):")
    print("-" * 80)
    for i, line in enumerate(replace_code.split('\n'), 1):
        spaces = len(line) - len(line.lstrip())
        print(f"Line {i:2d} (indent={spaces:2d}): |{line}|")

    print("\n" + "=" * 80)
    print("ANALYSIS:")
    print("=" * 80)

    # Check if AI indented correctly
    search_lines = search_code.split('\n')
    replace_lines = replace_code.split('\n')

    # Find minimum indentation in search
    min_search = min((len(line) - len(line.lstrip()) for line in search_lines if line.strip()), default=0)
    min_replace = min((len(line) - len(line.lstrip()) for line in replace_lines if line.strip()), default=0)

    print(f"✅ AI's <search> block minimum indent: {min_search} spaces")
    print(f"✅ AI's <replace> block minimum indent: {min_replace} spaces")

    if min_search == min_replace == 4:
        print("✅ AI generated CORRECT indentation (4 spaces = inside function)")
    else:
        print(f"⚠️  Indentation mismatch: search={min_search}, replace={min_replace}")

    print("\n🔍 WHAT PARSER DOES:")
    print("-" * 80)

    # Simulate parser's normalize_leading_whitespace
    def normalize_leading_whitespace(code):
        lines = code.split('\n')

        if not lines:
            return code

        # Find minimum non-zero indentation
        min_indent = float('inf')
        for line in lines:
            if line.strip():  # Skip empty lines
                indent = len(line) - len(line.lstrip())
                if indent > 0 and indent < min_indent:
                    min_indent = indent

        # If no indentation found, return as-is
        if min_indent == float('inf') or min_indent == 0:
            return code

        # Remove common leading indentation (dedent)
        dedented_lines = []
        for line in lines:
            if not line.strip():
                dedented_lines.append("")
            else:
                indent = len(line) - len(line.lstrip())
                if indent >= min_indent:
                    dedented_lines.append(line[min_indent:])
                else:
                    dedented_lines.append(line)

        return '\n'.join(dedented_lines)

    normalized_search = normalize_leading_whitespace(search_code)
    normalized_replace = normalize_leading_whitespace(replace_code)

    print("❌ AFTER normalize_leading_whitespace (SEARCH):")
    for i, line in enumerate(normalized_search.split('\n'), 1):
        spaces = len(line) - len(line.lstrip())
        print(f"Line {i:2d} (indent={spaces:2d}): |{line}|")

    print("\n❌ AFTER normalize_leading_whitespace (REPLACE):")
    for i, line in enumerate(normalized_replace.split('\n'), 1):
        spaces = len(line) - len(line.lstrip())
        print(f"Line {i:2d} (indent={spaces:2d}): |{line}|")

    print("\n" + "=" * 80)
    print("🔴 PROBLEM DETECTED!")
    print("=" * 80)
    print("The parser's normalize_leading_whitespace() is REMOVING the base indentation!")
    print("This function should NOT dedent code blocks - it should preserve them.")
    print("")
    print("BEFORE parser:  4 spaces indent (correct - inside function)")
    print("AFTER parser:   0 spaces indent (BROKEN - now at module level)")
    print("")
    print("This causes the matcher to fail because:")
    print("  - File has code at indent=4 (inside function)")
    print("  - Parser gives matcher code at indent=0 (module level)")
    print("  - Matcher can't find the code!")
