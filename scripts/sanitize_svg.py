#!/usr/bin/env python3
"""
scripts/sanitize_svg.py
Sanitizes Mermaid SVG files for strict compatibility with resvg and simplecss.
- Strips @keyframes rules (unsupported by simplecss)
- Strips :root selectors (unsupported by simplecss)
- Strips [data-look=...] attribute selectors (unsupported by simplecss)
"""

import sys
import re

def sanitize_svg_content(content: str) -> str:
    # 1. Strip @keyframes rules (handles single-level nested braces)
    content = re.sub(r'@keyframes\s+[^{]+\{(?:[^{}]*\{[^{}]*\})*[^{}]*\}', '', content)

    # 2. Strip :root selector rules
    content = re.sub(r'[^;{}]*:root\s*\{[^}]*\}', '', content)

    # 3. Strip rules inside <style> blocks that use unsupported attribute selectors like [data-look="neo"]
    def clean_style_block(m):
        css = m.group(1)
        cleaned_rules = []
        # Match each rule: selector { decls }
        for rule in re.finditer(r'([^{}]+)\{([^{}]*)\}', css):
            selector, decls = rule.group(1), rule.group(2)
            if '[data-look' not in selector and ':root' not in selector:
                cleaned_rules.append(f'{selector}{{{decls}}}')
        return '<style>' + ''.join(cleaned_rules) + '</style>'

    content = re.sub(r'<style>(.*?)</style>', clean_style_block, content, flags=re.DOTALL)

    # 4. Strip empty <rect/> and un-dimensioned background rects (Mermaid label placeholders)
    content = re.sub(r'<rect(?:\s+class="background")?\s+style="stroke:\s*none"\s*/>', '', content)
    content = re.sub(r'<rect\s*/>', '', content)
    return content

def sanitize_file(path: str) -> bool:
    with open(path, 'r', encoding='utf-8') as f:
        original = f.read()

    cleaned = sanitize_svg_content(original)
    if cleaned != original:
        with open(path, 'w', encoding='utf-8') as f:
            f.write(cleaned)
        return True
    return False

def main():
    if len(sys.argv) < 2:
        print("Usage: sanitize_svg.py <file1.svg> [file2.svg ...]")
        sys.exit(1)

    modified_count = 0
    for path in sys.argv[1:]:
        if sanitize_file(path):
            modified_count += 1
            print(f"✓ Sanitized {path}")

    print(f"Done. Modified {modified_count} file(s).")

if __name__ == '__main__':
    main()
