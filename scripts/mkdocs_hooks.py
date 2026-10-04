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


def on_post_page(output, page, config, **kwargs):
    """Put the site's name in the title, and give a shared link a description.

    The theme titles a page with its own heading and nothing else, so every
    tab reads `Configuration` or `Accepted risks` with no sign of what project
    it belongs to, and a link posted anywhere previews as a bare word with no
    description. It also writes `og:site_name` behind `{% if site_name %}`,
    which is not a variable MkDocs defines, so that tag never appears.

    Fixed here rather than by overriding the template, because the block is
    inside an `{% include %}` and Jinja inheritance cannot reach it, and rather
    than by `title:` frontmatter on all 25 pages, which is the cost this setup
    exists to avoid.
    """
    import html as html_mod

    name = config["site_name"]
    description = config.get("site_description") or ""

    if page.is_homepage:
        # The tagline is a sentence and ends in a full stop; a title is not.
        tagline = description.rstrip(".")
        title = f"{name} — {tagline}" if tagline else name
    else:
        title = f"{page.title} · {name}"
    title = html_mod.escape(title, quote=True)
    described = html_mod.escape(description, quote=True)

    # Only rewrite what is actually there. If the theme stops emitting one of
    # these, the tag is left alone rather than replaced with a guess.
    substitutions = [
        (re.compile(r"<title>.*?</title>", re.S), f"<title>{title}</title>"),
        (
            re.compile(r'<meta property="og:title" content="[^"]*">'),
            f'<meta property="og:title" content="{title}">',
        ),
        (
            re.compile(r'<meta name="twitter:title" content="[^"]*">'),
            f'<meta name="twitter:title" content="{title}">',
        ),
    ]
    for pattern, replacement in substitutions:
        output = pattern.sub(lambda _m, r=replacement: r, output, count=1)

    # The tags the theme omits entirely. Added after og:title so they sit with
    # the rest, and only when the page does not already carry them.
    anchor = f'<meta property="og:title" content="{title}">'
    if anchor in output and 'property="og:site_name"' not in output:
        additions = [
            f'<meta property="og:site_name" content="{html_mod.escape(name, quote=True)}">',
            '<meta property="og:type" content="article">',
        ]
        if described and 'name="description"' not in output:
            additions += [
                f'<meta name="description" content="{described}">',
                f'<meta property="og:description" content="{described}">',
                f'<meta name="twitter:description" content="{described}">',
            ]
        output = output.replace(anchor, anchor + "\n" + "\n".join(additions), 1)

    return output
