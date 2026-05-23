use actix_web::{get, post, web, HttpResponse, Responder};
use crate::models::*;
use crate::db::*;
use crate::crypto::*;
use crate::utils::*;

#[post("/v1/project/invite")]
pub async fn send_project_invite(
    state: web::Data<AppState>,
    req: web::Form<ProjectInviteRequest>,
) -> impl Responder {
    let session_uid = req.ib_uid;
    let target_username = req.target_user.trim();

    // Verify project ownership
    let project_row = sqlx::query_as::<_, ProjectProfileRow>(
        "SELECT id, ib_uid, CAST(COALESCE(ib_uid, '') AS CHAR) AS username, 0 AS total_acknowledgments, project, description, languages, CAST(updated_at AS CHAR) AS updated_at, reinforcements, reinforcements_request FROM project_profile WHERE id = ? AND ib_uid = ? LIMIT 1"
    )
    .bind(req.project_id)
    .bind(session_uid)
    .fetch_optional(&state.db_pool)
    .await;

    let project = match project_row {
        Ok(Some(p)) => p,
        _ => return HttpResponse::Forbidden().json(PostResponse { success: false, message: "Project not found or unauthorized".to_string(), postid: None }),
    };

    // Lookup target user
    let target_uid = match lookup_user_by_username(&state, target_username).await {
        Ok(Some((uid, _))) => uid,
        _ => return HttpResponse::NotFound().json(PostResponse { success: false, message: "User not found".to_string(), postid: None }),
    };

    if is_blocked(&state, Some(session_uid), Some(target_uid)).await {
        return HttpResponse::Forbidden().json(PostResponse { success: false, message: "Cannot invite this user".to_string(), postid: None });
    }

    let invite_message = format!("I have invited you to collaborate on my project: **{}**. \n\n:[[ :project-invite: {} ]]:", project.project, project.id);
    let encrypted_message = match encode_dm_message_for_storage(&invite_message) {
        Ok(m) => m,
        Err(_) => return HttpResponse::InternalServerError().json(PostResponse { success: false, message: "Encryption failed".to_string(), postid: None }),
    };

    let result = sqlx::query(
        "INSERT INTO dm (sender_uid, recipient_uid, message) VALUES (?, ?, ?)"
    )
    .bind(session_uid)
    .bind(target_uid)
    .bind(&encrypted_message)
    .execute(&state.db_pool)
    .await;

    match result {
        Ok(_) => HttpResponse::Ok().json(PostResponse { success: true, message: "Invite sent".to_string(), postid: None }),
        Err(_) => HttpResponse::InternalServerError().json(PostResponse { success: false, message: "Failed to send invite".to_string(), postid: None }),
    }
}

#[post("/v1/project/invite/accept")]
pub async fn accept_project_invite(
    state: web::Data<AppState>,
    req: web::Form<ProjectInviteAcceptRequest>,
) -> impl Responder {
    let session_uid = req.ib_uid;
    let session_user = req.ib_user.trim().to_string();

    let project_row = sqlx::query_as::<_, ProjectProfileRow>(
        "SELECT id, ib_uid, CAST(COALESCE(ib_uid, '') AS CHAR) AS username, 0 AS total_acknowledgments, project, description, languages, CAST(updated_at AS CHAR) AS updated_at, reinforcements, reinforcements_request FROM project_profile WHERE id = ? LIMIT 1"
    )
    .bind(req.project_id)
    .fetch_optional(&state.db_pool)
    .await;

    let project = match project_row {
        Ok(Some(p)) => p,
        _ => return HttpResponse::NotFound().json(PostResponse { success: false, message: "Project not found".to_string(), postid: None }),
    };

    let current_reinforcements = project.reinforcements.unwrap_or_default();
    let mut reinforcements_list: Vec<&str> = current_reinforcements.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

    if !reinforcements_list.iter().any(|r| r.eq_ignore_ascii_case(&session_user)) {
        reinforcements_list.push(&session_user);
        let new_reinforcements = reinforcements_list.join(",");
        
        let _ = sqlx::query("UPDATE project_profile SET reinforcements = ? WHERE id = ? LIMIT 1")
            .bind(new_reinforcements)
            .bind(project.id)
            .execute(&state.db_pool)
            .await;
    }

    HttpResponse::Ok().json(PostResponse { success: true, message: "Invite accepted".to_string(), postid: None })
}

#[post("/v1/project/dm/send")]
pub async fn send_project_dm(
    state: web::Data<AppState>,
    req: web::Form<ProjectDMMessageRequest>,
) -> impl Responder {
    let session_uid = req.ib_uid;
    let session_user = req.ib_user.trim().to_string();

    let project_row = sqlx::query_as::<_, ProjectProfileRow>(
        "SELECT id, ib_uid, CAST(COALESCE(ib_uid, '') AS CHAR) AS username, 0 AS total_acknowledgments, project, description, languages, CAST(updated_at AS CHAR) AS updated_at, reinforcements, reinforcements_request FROM project_profile WHERE id = ? LIMIT 1"
    )
    .bind(req.project_id)
    .fetch_optional(&state.db_pool)
    .await;

    let project = match project_row {
        Ok(Some(p)) => p,
        _ => return HttpResponse::NotFound().json(DMSendResponse { success: false, message: "Project not found".to_string() }),
    };

    let is_owner = project.ib_uid == session_uid;
    let current_reinforcements = project.reinforcements.unwrap_or_default();
    let is_reinforcement = current_reinforcements.split(',').any(|s| s.trim().eq_ignore_ascii_case(&session_user));

    if !is_owner && !is_reinforcement {
        return HttpResponse::Forbidden().json(DMSendResponse { success: false, message: "Not a reinforcement".to_string() });
    }

    let encrypted_message = match encode_dm_message_for_storage(&req.message) {
        Ok(m) => m,
        Err(_) => return HttpResponse::InternalServerError().json(DMSendResponse { success: false, message: "Encryption failed".to_string() }),
    };

    let result = sqlx::query(
        "INSERT INTO project_dm (project_id, sender_uid, message) VALUES (?, ?, ?)"
    )
    .bind(req.project_id)
    .bind(session_uid)
    .bind(&encrypted_message)
    .execute(&state.db_pool)
    .await;

    match result {
        Ok(_) => HttpResponse::Ok().json(DMSendResponse { success: true, message: "Message sent".to_string() }),
        Err(_) => HttpResponse::InternalServerError().json(DMSendResponse { success: false, message: "Failed to send message".to_string() }),
    }
}

#[get("/v1/project/dm/messages")]
pub async fn get_project_dm_messages(
    state: web::Data<AppState>,
    req: web::Query<ProjectDMMessagesRequest>,
) -> impl Responder {
    let session_uid = req.ib_uid;
    let session_user = req.ib_user.trim().to_string();

    let project_row = sqlx::query_as::<_, ProjectProfileRow>(
        "SELECT id, ib_uid, CAST(COALESCE(ib_uid, '') AS CHAR) AS username, 0 AS total_acknowledgments, project, description, languages, CAST(updated_at AS CHAR) AS updated_at, reinforcements, reinforcements_request FROM project_profile WHERE id = ? LIMIT 1"
    )
    .bind(req.project_id)
    .fetch_optional(&state.db_pool)
    .await;

    let project = match project_row {
        Ok(Some(p)) => p,
        _ => return HttpResponse::NotFound().json(ProjectDMMessagesResponse { success: false, messages: vec![], has_more: false }),
    };

    let is_owner = project.ib_uid == session_uid;
    let current_reinforcements = project.reinforcements.unwrap_or_default();
    let is_reinforcement = current_reinforcements.split(',').any(|s| s.trim().eq_ignore_ascii_case(&session_user));

    if !is_owner && !is_reinforcement {
        return HttpResponse::Forbidden().json(ProjectDMMessagesResponse { success: false, messages: vec![], has_more: false });
    }

    let sql = if let Some(before_id) = req.before_id {
        "SELECT pdm.id, pdm.sender_uid, CAST(COALESCE(user.username, '') AS CHAR) AS sender_username, pdm.message, CAST(pdm.created_at AS CHAR) AS created_at FROM project_dm pdm LEFT JOIN user ON CAST(user.ib_uid AS UNSIGNED) = pdm.sender_uid WHERE pdm.project_id = ? AND pdm.id < ? ORDER BY pdm.id DESC LIMIT 50"
    } else {
        "SELECT pdm.id, pdm.sender_uid, CAST(COALESCE(user.username, '') AS CHAR) AS sender_username, pdm.message, CAST(pdm.created_at AS CHAR) AS created_at FROM project_dm pdm LEFT JOIN user ON CAST(user.ib_uid AS UNSIGNED) = pdm.sender_uid WHERE pdm.project_id = ? ORDER BY pdm.id DESC LIMIT 50"
    };

    let mut query = sqlx::query_as::<_, ProjectDMMessageRow>(sql).bind(req.project_id);
    if let Some(before_id) = req.before_id {
        query = query.bind(before_id);
    }

    let rows = match query.fetch_all(&state.db_pool).await {
        Ok(r) => r,
        Err(_) => return HttpResponse::InternalServerError().json(ProjectDMMessagesResponse { success: false, messages: vec![], has_more: false }),
    };

    let mut messages = Vec::new();
    let has_more = rows.len() == 50;

    for row in rows.into_iter().rev() {
        let plaintext = match decode_dm_message_from_storage(&row.message) {
            Ok(pt) => pt,
            Err(_) => "[Encrypted message unreadable]".to_string(),
        };

        let rendered_markdown = crate::render::render_post_with_hashtags(&plaintext, row.sender_uid, &row.sender_username);

        messages.push(ProjectDMMessageResponseItem {
            id: row.id,
            sender_user: row.sender_username,
            message: rendered_markdown,
            timestamp: row.created_at,
            is_mine: row.sender_uid == session_uid,
        });
    }

    HttpResponse::Ok().json(ProjectDMMessagesResponse { success: true, messages, has_more })
}
