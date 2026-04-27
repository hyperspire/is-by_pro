import sys

# FIX JS
with open("webroot/js/is-by_user.js", "r") as f:
    js_content = f.read()

old_js = """function attachPinPostEventListener() {
  const pinLinks = document.querySelectorAll('.pin-post-link');

  pinLinks.forEach((link) => {
    link.addEventListener('click', async (event) => {"""

new_js = """function attachPinPostEventListener() {
  const pinLinks = document.querySelectorAll('.pin-post-link');

  pinLinks.forEach((link) => {
    if (link.dataset.pinBound === '1') return;
    link.dataset.pinBound = '1';

    link.addEventListener('click', async (event) => {"""

js_content = js_content.replace(old_js, new_js)

with open("webroot/js/is-by_user.js", "w") as f:
    f.write(js_content)


# FIX RUST
with open("src/routes/api.rs", "r") as f:
    rs_content = f.read()

import re
rs_content = re.sub(
    r'match sqlx::query_scalar\(\s*"SELECT pinned_postid FROM user WHERE CONVERT\(ib_uid USING utf8mb4\) COLLATE utf8mb4_unicode_ci = CAST\(\? AS CHAR CHARACTER SET utf8mb4\) COLLATE utf8mb4_unicode_ci LIMIT 1"\s*\)\s*\.bind\(session_uid\)',
    'match sqlx::query_scalar::<_, Option<String>>(\n    "SELECT pinned_postid FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1"\n  )\n  .bind(session_uid.to_string())',
    rs_content
)

rs_content = re.sub(
    r'sqlx::query\(\s*"UPDATE user SET pinned_postid = \? WHERE CONVERT\(ib_uid USING utf8mb4\) COLLATE utf8mb4_unicode_ci = CAST\(\? AS CHAR CHARACTER SET utf8mb4\) COLLATE utf8mb4_unicode_ci"\s*\)\s*\.bind\(new_pinned\)\s*\.bind\(session_uid\)',
    'sqlx::query(\n    "UPDATE user SET pinned_postid = ? WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ?"\n  )\n  .bind(new_pinned)\n  .bind(session_uid.to_string())',
    rs_content
)

with open("src/routes/api.rs", "w") as f:
    f.write(rs_content)
