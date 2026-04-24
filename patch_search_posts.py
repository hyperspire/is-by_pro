import sys

with open("src/main.rs", "r") as f:
    content = f.read()

# 1. Duplicate render_search_posts_html -> render_search_posts_mobile_html
start_sig = "async fn render_search_posts_html("
end_sig = "async fn render_projects_html("

start_idx = content.find(start_sig)
end_idx = content.find(end_sig)
if start_idx == -1 or end_idx == -1:
    print("Could not find function bounds")
    sys.exit(1)

html_func = content[start_idx:end_idx]
mobile_func = html_func.replace("render_search_posts_html", "render_search_posts_mobile_html")
mobile_func = mobile_func.replace('TEMPLATES.render("search_posts.html", &context)', 'TEMPLATES.render("search_posts_mobile.html", &context)')

content = content[:end_idx] + mobile_func + content[end_idx:]

# 2. Update search_posts to handle mobile routing
old_route = """async fn search_posts(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<SearchPostsRequest>,
) -> impl Responder {
  match render_search_posts_html(
    &state,
    query.ib_uid,
    &query.ib_user,
    &query.tag,
    get_session_uid(&req),
  ).await {"""

new_route = """async fn search_posts(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<SearchPostsRequest>,
) -> impl Responder {
  let html_result = if is_mobile_device(&req) {
    render_search_posts_mobile_html(
      &state,
      query.ib_uid,
      &query.ib_user,
      &query.tag,
      get_session_uid(&req),
    ).await
  } else {
    render_search_posts_html(
      &state,
      query.ib_uid,
      &query.ib_user,
      &query.tag,
      get_session_uid(&req),
    ).await
  };

  match html_result {"""

content = content.replace(old_route, new_route)

with open("src/main.rs", "w") as f:
    f.write(content)
