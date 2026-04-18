import re

with open("/home/hyperuser/is-by_pro/src/main.rs", "r") as f:
    code = f.read()

# Extract render_war_room_html
match = re.search(r'(async fn render_war_room_html.*?\n\})', code, re.DOTALL)
if not match:
    print("Function not found!")
    exit(1)

func_code = match.group(1)

# Rename function
mobile_func = func_code.replace("async fn render_war_room_html", "async fn render_war_room_mobile_html")

# Modify head to include mobile css and viewport tag
mobile_func = mobile_func.replace(
    '<meta name="viewport" content="width=device-width, initial-scale=1">',
    '<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">\n  <link rel="stylesheet" type="text/css" href="/css/is-by_mobile.css">\n  <link rel="preconnect" href="https://fonts.googleapis.com">\n  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>\n  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;800&display=swap" rel="stylesheet">'
)

# Insert it right before render_inbox_html
insert_pos = code.find("async fn render_inbox_html")
new_code = code[:insert_pos] + mobile_func + "\n\n" + code[insert_pos:]

# Update route handler war_room
route_code = """
#[get("/v1/warroom")]
async fn war_room(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<WarRoomRequest>,
) -> impl Responder {
  let session_uid = get_session_uid(&req);
  if session_uid.is_none() {
    return HttpResponse::Unauthorized().body("Login required");
  }

  let mut is_mobile = false;
  if let Some(user_agent) = req.headers().get("user-agent") {
    if let Ok(ua_str) = user_agent.to_str() {
      let ua_lower = ua_str.to_lowercase();
      is_mobile = ua_lower.contains("mobi") || ua_lower.contains("android") || ua_lower.contains("iphone") || ua_lower.contains("ipad");
    }
  }

  if is_mobile {
    match render_war_room_mobile_html(&state, query.ib_uid, &query.ib_user, session_uid).await {
      Ok(html) => HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html),
      Err(err) => HttpResponse::InternalServerError().body(err),
    }
  } else {
    match render_war_room_html(&state, query.ib_uid, &query.ib_user, session_uid).await {
      Ok(html) => HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html),
      Err(err) => HttpResponse::InternalServerError().body(err),
    }
  }
}
"""

# Replace old route handler
new_code = re.sub(
    r'#\[get\("/v1/warroom"\)\].*?async fn war_room.*?\n\}', 
    route_code.strip(), 
    new_code, 
    flags=re.DOTALL
)

with open("/home/hyperuser/is-by_pro/src/main.rs", "w") as f:
    f.write(new_code)

print("Done")
