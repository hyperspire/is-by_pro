use serde::Deserialize;
#[derive(Deserialize, Debug)]
pub struct ProjectInviteRequest {
  pub ib_uid: i64,
  pub ib_user: String,
  pub project_id: i64,
  pub target_user: String,
}

fn main() {
    let raw = "ib_uid=138945726&ib_user=hyperuser&project_id=1&target_user=hyperuser";
    let req: Result<ProjectInviteRequest, _> = serde_urlencoded::from_str(raw);
    println!("{:?}", req);
}
