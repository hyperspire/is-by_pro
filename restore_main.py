import sys

with open("/tmp/main.rs.old", "r") as f:
    old_content = f.read()

with open("src/main.rs", "r") as f:
    new_content = f.read()

start_sig = "async fn render_profile_mobile_html("
end_sig = "async fn render_search_posts_html("

old_start = old_content.find(start_sig)
old_end = old_content.find(end_sig)
if old_start == -1 or old_end == -1:
    print("Could not find sigs in old file")
    sys.exit(1)

new_start = new_content.find(start_sig)
new_end = new_content.find(end_sig)
if new_start == -1 or new_end == -1:
    print("Could not find sigs in new file")
    sys.exit(1)

block = old_content[old_start:old_end]
restored_content = new_content[:new_start] + block + new_content[new_end:]

with open("src/main.rs", "w") as f:
    f.write(restored_content)

print("Restored blocks")
