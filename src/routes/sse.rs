use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use crate::{models::*, auth::*};
use futures_util::StreamExt;

#[get("/v1/events")]
pub async fn events_endpoint(req: HttpRequest, state: web::Data<AppState>) -> impl Responder {
  let Some((session_uid, _)) = get_session_identity(&req, &state).await else {
    return HttpResponse::Unauthorized().body("Login required");
  };

  let rx = state.sse_sender.subscribe();

  let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
    .filter_map(move |res| async move {
      let event = res.ok()?;
      if event.target_uid == session_uid {
        let json_data = serde_json::to_string(&event).unwrap_or_default();
        let sse_data = format!("data: {}\n\n", json_data);
        Some(Ok::<_, actix_web::Error>(actix_web::web::Bytes::from(sse_data)))
      } else {
        None
      }
    });

  HttpResponse::Ok()
    .insert_header(("Content-Type", "text/event-stream"))
    .insert_header(("Cache-Control", "no-cache"))
    .insert_header(("Connection", "keep-alive"))
    .insert_header(("X-Accel-Buffering", "no"))
    .streaming(stream)
}
