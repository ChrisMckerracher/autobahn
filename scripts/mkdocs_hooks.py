"""A link that leaves docs/ becomes a link into the repository on GitHub.

The docs are read on GitHub first, so a page links a sibling file the plain
way: `../INSTALL.md`, `../spec/README.md`, `../benchmarks/2026-10-02.json`.
Those are correct there. MkDocs only serves what lives under docs/, so on the
site they would 404.

Rewriting them in the source would fix the site by making the GitHub view
worse, which is backwards — the source is the thing people read. So they are
rewritten here instead, at build time. Seven links today, and nothing for
anyone to remember when they add the eighth.
"""

import os
import re

# A markdown link or image. The target is group 2.
LINK = re.compile(r"(!?\[[^\]]*\]\()([^)\s]+)(\))")
# A scheme, a protocol-relative URL, or a bare anchor: not ours to touch.
EXTERNAL = re.compile(r"^(?:[a-z][a-z0-9+.\-]*:|//|#)", re.I)


def on_page_markdown(markdown, page, config, files):
    docs_dir = os.path.realpath(config["docs_dir"])
    # docs/ sits directly under the repository root, which is what a rewritten
    # path is relative to.
    repo_root = os.path.dirname(docs_dir)
    repo_url = config["repo_url"].rstrip("/")
    page_dir = os.path.dirname(page.file.abs_src_path)

    def rewrite(match):
        opening, target, closing = match.groups()
        if EXTERNAL.match(target):
            return match.group(0)
        path, _, fragment = target.partition("#")
        if not path:
            return match.group(0)

        resolved = os.path.realpath(os.path.join(page_dir, path))
        # Inside docs/: MkDocs handles it, including the .md -> URL rewrite.
        if resolved == docs_dir or resolved.startswith(docs_dir + os.sep):
            return match.group(0)
        # Outside the repository altogether: leave it, and let MkDocs say so.
        relative = os.path.relpath(resolved, repo_root)
        if relative.startswith(os.pardir):
            return match.group(0)

        url = f"{repo_url}/blob/main/{relative.replace(os.sep, '/')}"
        if fragment:
            url = f"{url}#{fragment}"
        return f"{opening}{url}{closing}"

    return LINK.sub(rewrite, markdown)
