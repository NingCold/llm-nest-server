//! Opaque Matrix crypto transport. The cloud never receives device secrets.
use crate::{AppError, AppState, Result, auth, chat, db as sqlx, nodes};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
#[path = "crypto_authority.rs"]
mod authority;
pub(crate) type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;
pub(crate) struct Actor {
    pub id: Uuid,
    pub node: Option<nodes::Node>,
}
pub(crate) fn uid(s: &str) -> Result<Uuid> {
    s.parse().map_err(|_| AppError::bad("账号或设备标识无效"))
}
pub(crate) fn field<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::bad("缺少加密协议字段"))
}
pub(crate) fn matrix_user(id: Uuid) -> String {
    format!("@{id}:llmn")
}
fn account(s: &str) -> Result<Uuid> {
    uid(s
        .strip_prefix('@')
        .and_then(|s| s.strip_suffix(":llmn"))
        .ok_or_else(|| AppError::bad("Matrix账号无效"))?)
}
fn digest(v: &Value) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(v).unwrap()))
}
pub(crate) async fn actor(s: &AppState, h: &HeaderMap, tx: &mut Tx<'_>) -> Result<Actor> {
    let a = if h.contains_key("authorization") {
        let n = nodes::authenticate(tx, h).await?;
        Actor {
            id: n.ai,
            node: Some(n),
        }
    } else {
        Actor {
            id: auth::authenticate(s, h).await?.id,
            node: None,
        }
    };
    let active: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=$1 AND status='active')")
            .bind(a.id)
            .fetch_one(&mut **tx)
            .await?;
    if !active {
        return Err(AppError::forbidden());
    }
    Ok(a)
}
pub(crate) async fn device(tx: &mut Tx<'_>, a: &Actor, id: &str) -> Result<()> {
    let r=sqlx::query("SELECT node_id,binding_version FROM cloud_crypto_devices WHERE account_id=$1 AND device_id=$2 AND NOT revoked").bind(a.id).bind(id).fetch_optional(&mut **tx).await?.ok_or_else(AppError::forbidden)?;
    match &a.node {
        Some(n)
            if r.get::<Option<Uuid>, _>("node_id") == Some(n.node)
                && r.get::<Option<i64>, _>("binding_version") == Some(n.version) =>
        {
            Ok(())
        }
        None if r.get::<Option<Uuid>, _>("node_id").is_none() => Ok(()),
        _ => Err(AppError::forbidden()),
    }
}
async fn related(tx: &mut Tx<'_>, from: Uuid, to: Uuid) -> Result<bool> {
    if from == to {
        return Ok(true);
    }
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_members a JOIN cloud_members b ON a.conversation=b.conversation WHERE a.account_id=$1 AND b.account_id=$2) OR EXISTS(SELECT 1 FROM cloud_ai_admins WHERE (ai_id=$1 AND administrator=$2) OR (ai_id=$2 AND administrator=$1))").bind(from).bind(to).fetch_one(&mut **tx).await?)
}
pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/crypto/devices", post(register).get(devices))
        .route("/crypto/devices/{id}/revoke", post(revoke))
        .route("/crypto/authority/devices/{id}", get(authority::status))
        .route("/crypto/authority/enroll", post(authority::enroll))
        .route("/crypto/requests", post(request))
        .route("/crypto/sync", get(sync))
        .route("/node/crypto/conversations", get(conversations))
        .route("/crypto/backup", get(backup_read).put(backup_save))
        .route("/crypto/backup/keys", get(backup_keys))
        .route("/crypto/recovery", get(recovery_read).put(recovery_save))
        .route("/chat/conversations/{id}/encryption", post(enable))
}
async fn conversations(State(s): State<Arc<AppState>>, h: HeaderMap) -> Result<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    nodes::lock(&mut tx).await?;
    let a = actor(&s, &h, &mut tx).await?;
    let n = a.node.as_ref().ok_or_else(AppError::forbidden)?;
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT c.id FROM cloud_conversations c JOIN cloud_members m ON m.conversation=c.id WHERE m.account_id=$1 AND c.crypto_enabled ORDER BY c.id")
        .bind(n.ai).fetch_all(&mut *tx).await?;
    let mut conversations = Vec::with_capacity(ids.len());
    for cid in ids {
        conversations.push(crate::consciousness::node_conversation(&mut tx, cid).await?);
    }
    Ok(Json(json!({"conversations":conversations})))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Registration {
    device_id: String,
    name: Option<String>,
}
async fn register(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Json(v): Json<Registration>,
) -> Result<Json<Value>> {
    uid(&v.device_id)?;
    if v.device_id.is_empty()
        || v.device_id.len() > 255
        || v.name.as_ref().is_some_and(|s| s.len() > 256)
    {
        return Err(AppError::bad("设备标识无效"));
    }
    let mut tx = s.pool.begin().await?;
    nodes::lock(&mut tx).await?;
    let a = actor(&s, &h, &mut tx).await?;
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM cloud_crypto_devices WHERE account_id=$1 AND device_id=$2)",
    )
    .bind(a.id)
    .bind(&v.device_id)
    .fetch_one(&mut *tx)
    .await?
    {
        device(&mut tx, &a, &v.device_id).await?;
    } else {
        sqlx::query("INSERT INTO cloud_crypto_devices(account_id,device_id,node_id,binding_version,display_name) VALUES($1,$2,$3,$4,$5)").bind(a.id).bind(&v.device_id).bind(a.node.as_ref().map(|n|n.node)).bind(a.node.as_ref().map(|n|n.version)).bind(v.name).execute(&mut *tx).await?;
    }
    let (authorized, fingerprint) = authority::record(&mut tx, a.id, &v.device_id).await?;
    tx.commit().await?;
    Ok(Json(
        json!({"userId":matrix_user(a.id),"deviceId":v.device_id,"authorized":authorized,"fingerprint":fingerprint}),
    ))
}
async fn devices(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Query(q): Query<DeviceQuery>,
) -> Result<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    let a = actor(&s, &h, &mut tx).await?;
    let target = q.account_id.unwrap_or(a.id);
    let rows =
        sqlx::query("SELECT * FROM cloud_crypto_devices WHERE account_id=$1 ORDER BY created_at")
            .bind(target)
            .fetch_all(&mut *tx)
            .await?;
    Ok(Json(
        json!({"devices":rows.iter().map(|r|json!({"deviceId":r.get::<String,_>("device_id"),"name":r.get::<Option<String>,_>("display_name"),"revoked":r.get::<bool,_>("revoked"),"nodeId":r.get::<Option<Uuid>,_>("node_id"),"bindingVersion":r.get::<Option<i64>,_>("binding_version").map(|v|v.to_string()),"keys":r.get::<Option<Value>,_>("device_keys")})).collect::<Vec<_>>()}),
    ))
}
async fn revoke(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<DeviceQuery>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    nodes::lock(&mut tx).await?;
    let a = actor(&s, &h, &mut tx).await?;
    let target = q.account_id.unwrap_or(a.id);
    if a.node.is_none() {
        if v["accountId"] != target.to_string() || v["target"] != id {
            return Err(AppError::forbidden());
        }
        authority::command(&mut tx, &a, "revoke", &v).await?;
    }
    if target != a.id {
        if a.node.is_some() {
            return Err(AppError::forbidden());
        }
        nodes::admin(&mut tx, target, a.id).await?;
    }
    if a.node.is_none() && target == a.id && v["deviceId"] == id {
        return Err(AppError::conflict("请用另一台可信设备撤销本机"));
    }
    let changed=sqlx::query(
        "UPDATE cloud_crypto_devices SET revoked=true WHERE account_id=$1 AND device_id=$2 AND NOT revoked",
    )
    .bind(target)
    .bind(&id)
    .execute(&mut *tx)
    .await?;
    if changed.rows_affected() > 0 && a.node.is_none() && target == a.id {
        sqlx::query(
            "UPDATE cloud_crypto_backups SET sealed=true WHERE account_id=$1 AND NOT sealed",
        )
        .bind(target)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE cloud_crypto_recovery SET enrollment_key=NULL WHERE account_id=$1")
            .bind(target)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("DELETE FROM cloud_crypto_inbox WHERE account_id=$1 AND device_id=$2")
        .bind(target)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({})))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CryptoRequest {
    device_id: String,
    request_id: String,
    kind: String,
    body: Value,
    proof: Option<String>,
}
async fn request(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Json(v): Json<CryptoRequest>,
) -> Result<Json<Value>> {
    if v.request_id.is_empty() || v.request_id.len() > 255 {
        return Err(AppError::bad("请求标识无效"));
    }
    let fp = digest(&json!([v.kind, v.body]));
    let mut tx = s.pool.begin().await?;
    nodes::lock(&mut tx).await?;
    let a = actor(&s, &h, &mut tx).await?;
    device(&mut tx, &a, &v.device_id).await?;
    if v.kind == "keysBackup" {
        authority::proof(
            &mut tx,
            &a,
            &v.device_id,
            "keysBackup",
            &v.body,
            v.proof.as_deref(),
            true,
        )
        .await?;
    }
    if v.kind == "toDevice" {
        history_delivery_authority(&mut tx, &a, &v.body).await?;
    }
    if let Some(r)=sqlx::query("SELECT fingerprint,response FROM cloud_crypto_receipts WHERE account_id=$1 AND device_id=$2 AND request_id=$3").bind(a.id).bind(&v.device_id).bind(&v.request_id).fetch_optional(&mut *tx).await?{if r.get::<String,_>(0)!=fp{return Err(AppError::conflict("加密请求标识已使用"));}return Ok(Json(r.get(1)));}
    let out = match v.kind.as_str() {
        "keysUpload" => upload(&mut tx, &a, &v.device_id, &v.body).await?,
        "keysQuery" => query_keys(&mut tx, &v.body).await?,
        "keysClaim" => claim(&mut tx, &a, &v.body).await?,
        "toDevice" => to_device(&mut tx, &a, &v.body).await?,
        "signingKeys" => {
            if a.node.is_none() {
                let fresh:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM cloud_crypto_signing_keys WHERE account_id=$1) AND NOT EXISTS(SELECT 1 FROM cloud_crypto_recovery WHERE account_id=$1) AND NOT EXISTS(SELECT 1 FROM cloud_crypto_backups WHERE account_id=$1)").bind(a.id).fetch_one(&mut *tx).await?;
                authority::proof(
                    &mut tx,
                    &a,
                    &v.device_id,
                    "signingKeys",
                    &v.body,
                    v.proof.as_deref(),
                    !fresh,
                )
                .await?;
                if fresh
                    && v.body["master_key"]["keys"]
                        .as_object()
                        .is_some_and(|k| !k.is_empty())
                {
                    sqlx::query("UPDATE cloud_crypto_devices SET authorized=true WHERE account_id=$1 AND device_id=$2").bind(a.id).bind(&v.device_id).execute(&mut *tx).await?;
                }
            }
            signing(&mut tx, &a, &v.body).await?
        }
        "signatures" => signatures(&mut tx, &a, &v.body).await?,
        "keysBackup" => backup(&mut tx, &a, &v.body).await?,
        _ => return Err(AppError::bad("不支持的加密请求类型")),
    };
    sqlx::query("INSERT INTO cloud_crypto_receipts VALUES($1,$2,$3,$4,$5)")
        .bind(a.id)
        .bind(v.device_id)
        .bind(v.request_id)
        .bind(fp)
        .bind(&out)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(out))
}
async fn counts(tx: &mut Tx<'_>, a: Uuid, d: &str) -> Result<Value> {
    let n:i64=sqlx::query_scalar("SELECT count(*) FROM cloud_crypto_one_time_keys WHERE account_id=$1 AND device_id=$2 AND NOT fallback").bind(a).bind(d).fetch_one(&mut **tx).await?;
    Ok(json!({"signed_curve25519":n}))
}
fn preserve_signatures(old: Option<&Value>, new: &Value) -> Value {
    let mut out = new.clone();
    if let Some(signatures) = old
        .and_then(|v| v.get("signatures"))
        .and_then(Value::as_object)
    {
        for (user, keys) in signatures {
            for (key, signature) in keys.as_object().into_iter().flatten() {
                if out["signatures"][user].get(key).is_none() {
                    out["signatures"][user][key] = signature.clone();
                }
            }
        }
    }
    out
}
async fn upload(tx: &mut Tx<'_>, a: &Actor, d: &str, v: &Value) -> Result<Value> {
    if let Some(keys) = v.get("device_keys") {
        if keys["user_id"] != matrix_user(a.id) || keys["device_id"] != d {
            return Err(AppError::forbidden());
        }
        let old: Option<Value> = sqlx::query_scalar(
            "SELECT device_keys FROM cloud_crypto_devices WHERE account_id=$1 AND device_id=$2",
        )
        .bind(a.id)
        .bind(d)
        .fetch_one(&mut **tx)
        .await?;
        if old.as_ref().is_some_and(|o| o["keys"] != keys["keys"]) {
            return Err(AppError::conflict("设备身份密钥已变化，请登记新设备"));
        }
        sqlx::query(
            "UPDATE cloud_crypto_devices SET device_keys=$3 WHERE account_id=$1 AND device_id=$2",
        )
        .bind(a.id)
        .bind(d)
        .bind(preserve_signatures(old.as_ref(), keys))
        .execute(&mut **tx)
        .await?;
    }
    for (name, fallback) in [("one_time_keys", false), ("fallback_keys", true)] {
        if let Some(keys) = v.get(name) {
            for (id, data) in keys
                .as_object()
                .ok_or_else(|| AppError::bad("公钥数据无效"))?
            {
                if !id.starts_with("signed_curve25519:") {
                    return Err(AppError::bad("公钥算法不支持"));
                }
                if fallback {
                    sqlx::query("DELETE FROM cloud_crypto_one_time_keys WHERE account_id=$1 AND device_id=$2 AND fallback AND key_id<>$3").bind(a.id).bind(d).bind(id).execute(&mut **tx).await?;
                }
                sqlx::query("INSERT INTO cloud_crypto_one_time_keys(account_id,device_id,key_id,key_data,fallback) VALUES($1,$2,$3,$4,$5) ON CONFLICT(account_id,device_id,key_id) DO NOTHING").bind(a.id).bind(d).bind(id).bind(data).bind(fallback).execute(&mut **tx).await?;
            }
        }
    }
    Ok(json!({"one_time_key_counts":counts(tx,a.id,d).await?}))
}
async fn query_keys(tx: &mut Tx<'_>, v: &Value) -> Result<Value> {
    let mut devices = serde_json::Map::new();
    let mut master = serde_json::Map::new();
    let mut own = serde_json::Map::new();
    let mut user = serde_json::Map::new();
    for (u, requested) in v["device_keys"]
        .as_object()
        .ok_or_else(|| AppError::bad("查询账号无效"))?
    {
        let aid = account(u)?;
        let ids = requested
            .as_array()
            .ok_or_else(|| AppError::bad("查询设备无效"))?;
        let rows=sqlx::query("SELECT device_id,device_keys,authorized OR node_id IS NOT NULL FROM cloud_crypto_devices d WHERE account_id=$1 AND NOT revoked AND device_keys IS NOT NULL AND (node_id IS NULL OR EXISTS(SELECT 1 FROM cloud_ai_bindings b WHERE b.ai_id=d.account_id AND b.node_id=d.node_id AND b.version=d.binding_version))").bind(aid).fetch_all(&mut **tx).await?;
        let mut keys = serde_json::Map::new();
        for r in rows {
            let id: String = r.get(0);
            if ids.is_empty() || ids.iter().any(|i| i.as_str() == Some(&id)) {
                let mut value: Value = r.get(1);
                if !r.get::<bool, _>(2) {
                    // An unadmitted device stays discoverable for manual fingerprint verification.
                    let own = format!("ed25519:{id}");
                    let sig = value["signatures"][u].get(&own).cloned();
                    value["signatures"] = json!({});
                    if let Some(sig) = sig {
                        value["signatures"][u][own] = sig;
                    }
                }
                keys.insert(id, value);
            }
        }
        devices.insert(u.clone(), Value::Object(keys));
        if let Some(r) = sqlx::query("SELECT * FROM cloud_crypto_signing_keys WHERE account_id=$1")
            .bind(aid)
            .fetch_optional(&mut **tx)
            .await?
        {
            for (col, map) in [
                ("master_key", &mut master),
                ("self_signing_key", &mut own),
                ("user_signing_key", &mut user),
            ] {
                if let Some(key) = r.get::<Option<Value>, _>(col) {
                    map.insert(u.clone(), key);
                }
            }
        }
    }
    Ok(
        json!({"device_keys":devices,"master_keys":master,"self_signing_keys":own,"user_signing_keys":user,"failures":{}}),
    )
}
async fn claim(tx: &mut Tx<'_>, a: &Actor, v: &Value) -> Result<Value> {
    let mut users = serde_json::Map::new();
    for (u, devices) in v["one_time_keys"]
        .as_object()
        .ok_or_else(|| AppError::bad("请求公钥无效"))?
    {
        let aid = account(u)?;
        if !related(tx, a.id, aid).await? {
            return Err(AppError::forbidden());
        }
        let mut found = serde_json::Map::new();
        for (d, alg) in devices
            .as_object()
            .ok_or_else(|| AppError::bad("请求设备无效"))?
        {
            if alg != "signed_curve25519" {
                return Err(AppError::bad("公钥算法不支持"));
            }
            let row=sqlx::query("SELECT k.* FROM cloud_crypto_one_time_keys k JOIN cloud_crypto_devices d USING(account_id,device_id) WHERE k.account_id=$1 AND k.device_id=$2 AND NOT d.revoked AND (d.node_id IS NULL OR EXISTS(SELECT 1 FROM cloud_ai_bindings b WHERE b.ai_id=d.account_id AND b.node_id=d.node_id AND b.version=d.binding_version)) ORDER BY fallback,key_id LIMIT 1").bind(aid).bind(d).fetch_optional(&mut **tx).await?;
            if let Some(r) = row {
                let key: String = r.get("key_id");
                found.insert(d.clone(), json!({key.clone():r.get::<Value,_>("key_data")}));
                if r.get::<bool, _>("fallback") {
                    sqlx::query("UPDATE cloud_crypto_one_time_keys SET used=true WHERE account_id=$1 AND device_id=$2 AND key_id=$3").bind(aid).bind(d).bind(key).execute(&mut **tx).await?;
                } else {
                    sqlx::query("DELETE FROM cloud_crypto_one_time_keys WHERE account_id=$1 AND device_id=$2 AND key_id=$3").bind(aid).bind(d).bind(key).execute(&mut **tx).await?;
                }
            }
        }
        users.insert(u.clone(), Value::Object(found));
    }
    Ok(json!({"one_time_keys":users,"failures":{}}))
}
async fn history_delivery_authority(tx: &mut Tx<'_>, a: &Actor, v: &Value) -> Result<()> {
    if v.get("llmn_purpose").is_none() {
        return Ok(());
    }
    if v["llmn_purpose"] == "conversationHistory" {
        let cid = uid(field(v, "llmn_conversation_id")?)?;
        let c = sqlx::query("SELECT kind,revision FROM cloud_conversations WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
        if c.get::<String, _>("kind") != "group"
            || field(v, "llmn_expected_revision")? != c.get::<i64, _>("revision").to_string()
        {
            return Err(AppError::conflict("群版本已变化"));
        }
        let members = chat::member(tx, cid, a.id).await?;
        for user in v["messages"]
            .as_object()
            .ok_or_else(|| AppError::bad("投递对象无效"))?
            .keys()
        {
            let target = account(user)?;
            if !members.contains(&target) {
                return Err(AppError::forbidden());
            }
            for device in v["messages"][user]
                .as_object()
                .ok_or_else(|| AppError::bad("投递设备无效"))?
                .keys()
            {
                let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_crypto_devices d WHERE account_id=$1 AND device_id=$2 AND NOT revoked AND device_keys IS NOT NULL AND (node_id IS NULL OR EXISTS(SELECT 1 FROM cloud_ai_bindings b WHERE b.ai_id=d.account_id AND b.node_id=d.node_id AND b.version=d.binding_version)))").bind(target).bind(device).fetch_one(&mut **tx).await?;
                if !active {
                    return Err(AppError::forbidden());
                }
            }
        }
        if v["event_type"] != "m.room.encrypted" {
            return Err(AppError::bad("历史密钥须经Olm密文投递"));
        }
        return Ok(());
    }
    if v["llmn_purpose"] != "aiAdminHistory" || a.node.is_none() {
        return Err(AppError::forbidden());
    }
    if v["event_type"] != "m.room.encrypted" {
        return Err(AppError::bad("历史密钥须经Olm密文投递"));
    }
    let scope = &v["llmn_chat_revisions"];
    if !scope.as_object().is_some_and(|rooms| {
        !rooms.is_empty()
            && rooms.iter().all(|(id, revision)| {
                Uuid::parse_str(id).is_ok()
                    && revision.as_str().is_some_and(|r| r.parse::<i64>().is_ok())
            })
    }) {
        return Err(AppError::bad("缺少有效的聊天历史范围"));
    }
    for (user, devices) in v["messages"]
        .as_object()
        .ok_or_else(|| AppError::bad("投递对象无效"))?
    {
        let target = account(user)?;
        let admin: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM cloud_ai_admins WHERE ai_id=$1 AND administrator=$2)",
        )
        .bind(a.id)
        .bind(target)
        .fetch_one(&mut **tx)
        .await?;
        if !admin {
            return Err(AppError::forbidden());
        }
        for device in devices
            .as_object()
            .ok_or_else(|| AppError::bad("投递设备无效"))?
            .keys()
        {
            let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_crypto_devices WHERE account_id=$1 AND device_id=$2 AND authorized AND NOT revoked AND node_id IS NULL AND device_keys IS NOT NULL)")
                .bind(target).bind(device).fetch_one(&mut **tx).await?;
            if !valid {
                return Err(AppError::forbidden());
            }
        }
    }
    if !chat_scope_current(tx, a.id, scope).await?
        || crate::crypto_resources::profile_audience_revision(tx, a.id).await?
            != v["llmn_audience_revision"]
    {
        return Err(AppError::conflict("聊天历史授权已变化"));
    }
    Ok(())
}

async fn chat_scope_current(tx: &mut Tx<'_>, ai: Uuid, scope: &Value) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "SELECT NOT EXISTS (SELECT FROM jsonb_each_text($2) AS room(id,revision)
         WHERE NOT EXISTS (SELECT FROM cloud_conversations c JOIN cloud_members m ON m.conversation=c.id
           WHERE c.id=room.id::uuid AND c.crypto_enabled AND m.account_id=$1 AND c.revision::text=room.revision))"
    ).bind(ai).bind(scope).fetch_one(&mut **tx).await?)
}
async fn to_device(tx: &mut Tx<'_>, a: &Actor, v: &Value) -> Result<Value> {
    let event_type = field(v, "event_type")?;
    if event_type != "m.room.encrypted"
        && !event_type.starts_with("m.key.verification.")
        && event_type != "m.room_key_request"
        && event_type != "m.room_key.withheld"
        && event_type != "m.dummy"
    {
        return Err(AppError::bad("不支持明文秘密投递"));
    }
    for (u, devices) in v["messages"]
        .as_object()
        .ok_or_else(|| AppError::bad("投递对象无效"))?
    {
        let aid = account(u)?;
        if !related(tx, a.id, aid).await? {
            return Err(AppError::forbidden());
        }
        for (target, content) in devices
            .as_object()
            .ok_or_else(|| AppError::bad("投递设备无效"))?
        {
            let ids:Vec<String>=sqlx::query_scalar("SELECT device_id FROM cloud_crypto_devices d WHERE account_id=$1 AND NOT revoked AND ($2='*' OR device_id=$2) AND ($3 <> 'm.room.encrypted' OR authorized OR node_id IS NOT NULL) AND (node_id IS NULL OR EXISTS(SELECT 1 FROM cloud_ai_bindings b WHERE b.ai_id=d.account_id AND b.node_id=d.node_id AND b.version=d.binding_version))").bind(aid).bind(target).bind(event_type).fetch_all(&mut **tx).await?;
            for id in ids {
                let mut event =
                    json!({"sender":matrix_user(a.id),"type":event_type,"content":content});
                if a.node.is_some() && event_type == "m.room.encrypted" {
                    // The server sets this routing metadata, never the supplied Matrix content.
                    event["llmn_route"] = if v["llmn_purpose"] == "aiAdminHistory" {
                        json!({"chats":v["llmn_chat_revisions"],"audience":v["llmn_audience_revision"]})
                    } else {
                        json!("ordinary")
                    };
                }
                sqlx::query(
                    "INSERT INTO cloud_crypto_inbox(account_id,device_id,event) VALUES($1,$2,$3)",
                )
                .bind(aid)
                .bind(id)
                .bind(event)
                .execute(&mut **tx)
                .await?;
            }
        }
    }
    Ok(json!({}))
}
async fn signing(tx: &mut Tx<'_>, a: &Actor, v: &Value) -> Result<Value> {
    let old = sqlx::query("SELECT * FROM cloud_crypto_signing_keys WHERE account_id=$1")
        .bind(a.id)
        .fetch_optional(&mut **tx)
        .await?;
    let mut merged = v.clone();
    for k in ["master_key", "self_signing_key", "user_signing_key"] {
        if let Some(key) = v.get(k) {
            if key["user_id"] != matrix_user(a.id) {
                return Err(AppError::forbidden());
            }
            let previous = old.as_ref().and_then(|r| r.get::<Option<Value>, _>(k));
            if previous
                .as_ref()
                .is_some_and(|old| old["keys"] != key["keys"])
            {
                return Err(AppError::conflict("加密身份已存在，请恢复原身份"));
            }
            merged[k] = preserve_signatures(previous.as_ref(), key);
        }
    }
    sqlx::query("INSERT INTO cloud_crypto_signing_keys(account_id,master_key,self_signing_key,user_signing_key) VALUES($1,$2,$3,$4) ON CONFLICT(account_id) DO UPDATE SET master_key=coalesce($2,cloud_crypto_signing_keys.master_key),self_signing_key=coalesce($3,cloud_crypto_signing_keys.self_signing_key),user_signing_key=coalesce($4,cloud_crypto_signing_keys.user_signing_key)").bind(a.id).bind(merged.get("master_key")).bind(merged.get("self_signing_key")).bind(merged.get("user_signing_key")).execute(&mut **tx).await?;
    Ok(json!({}))
}
async fn signatures(tx: &mut Tx<'_>, a: &Actor, v: &Value) -> Result<Value> {
    for (user, objects) in v.as_object().ok_or_else(|| AppError::bad("签名无效"))? {
        let aid = account(user)?;
        if !related(tx, a.id, aid).await? {
            return Err(AppError::forbidden());
        }
        for (id, signed) in objects
            .as_object()
            .ok_or_else(|| AppError::bad("签名对象无效"))?
        {
            let signatures = signed
                .get("signatures")
                .and_then(|s| s.get(matrix_user(a.id)))
                .and_then(Value::as_object)
                .ok_or_else(|| AppError::bad("缺少当前账号签名"))?;
            let device=sqlx::query_scalar::<_,Option<Value>>("SELECT device_keys FROM cloud_crypto_devices WHERE account_id=$1 AND device_id=$2 AND NOT revoked").bind(aid).bind(id).fetch_optional(&mut **tx).await?.flatten();
            if let Some(mut old) = device {
                if old["keys"] != signed["keys"] {
                    return Err(AppError::conflict("签名目标已变化"));
                }
                for (key, value) in signatures {
                    old["signatures"][matrix_user(a.id)][key] = value.clone();
                }
                sqlx::query("UPDATE cloud_crypto_devices SET device_keys=$3 WHERE account_id=$1 AND device_id=$2").bind(aid).bind(id).bind(old).execute(&mut **tx).await?;
            } else {
                let row=sqlx::query("SELECT master_key,self_signing_key,user_signing_key FROM cloud_crypto_signing_keys WHERE account_id=$1").bind(aid).fetch_optional(&mut **tx).await?;
                let mut found = false;
                if let Some(row) = row {
                    for column in ["master_key", "self_signing_key", "user_signing_key"] {
                        if let Some(mut old) = row.get::<Option<Value>, _>(column)
                            && old["keys"] == signed["keys"]
                        {
                            for (key, value) in signatures {
                                old["signatures"][matrix_user(a.id)][key] = value.clone();
                            }
                            let query = format!(
                                "UPDATE cloud_crypto_signing_keys SET {column}=$2 WHERE account_id=$1"
                            );
                            sqlx::query(&query)
                                .bind(aid)
                                .bind(old)
                                .execute(&mut **tx)
                                .await?;
                            found = true;
                        }
                    }
                }
                if !found {
                    return Err(AppError::conflict("签名目标不存在或已变化"));
                }
            }
        }
    }
    Ok(json!({"failures":{}}))
}
async fn backup(tx: &mut Tx<'_>, a: &Actor, v: &Value) -> Result<Value> {
    let version = uid(field(v, "version")?)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT coalesce($2::uuid=(SELECT version FROM cloud_crypto_backups WHERE account_id=$1 AND NOT sealed ORDER BY created_at DESC LIMIT 1),false)",
    )
    .bind(a.id)
    .bind(version)
    .fetch_one(&mut **tx)
    .await?;
    if !exists {
        return Err(AppError::conflict("历史密钥备份版本已变化"));
    }
    for (room, body) in v["rooms"]
        .as_object()
        .ok_or_else(|| AppError::bad("备份房间无效"))?
    {
        for (session, data) in body["sessions"]
            .as_object()
            .ok_or_else(|| AppError::bad("备份会话无效"))?
        {
            sqlx::query("INSERT INTO cloud_crypto_backup_sessions VALUES($1,$2,$3,$4,$5) ON CONFLICT(account_id,version,room_id,session_id) DO UPDATE SET session_data=$5").bind(a.id).bind(version).bind(room).bind(session).bind(data).execute(&mut **tx).await?;
        }
    }
    sqlx::query(
        "UPDATE cloud_crypto_backups SET revision=revision+1 WHERE account_id=$1 AND version=$2",
    )
    .bind(a.id)
    .bind(version)
    .execute(&mut **tx)
    .await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cloud_crypto_backup_sessions WHERE account_id=$1 AND version=$2",
    )
    .bind(a.id)
    .bind(version)
    .fetch_one(&mut **tx)
    .await?;
    let rev: i64 = sqlx::query_scalar(
        "SELECT revision FROM cloud_crypto_backups WHERE account_id=$1 AND version=$2",
    )
    .bind(a.id)
    .bind(version)
    .fetch_one(&mut **tx)
    .await?;
    crate::handover::changed(tx, a.id).await?;
    Ok(json!({"count":count,"etag":rev.to_string()}))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncQuery {
    device_id: String,
    after: Option<String>,
    device_lists_version: Option<String>,
}
async fn sync(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Query(q): Query<SyncQuery>,
) -> Result<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    nodes::lock(&mut tx).await?;
    let a = actor(&s, &h, &mut tx).await?;
    device(&mut tx, &a, &q.device_id).await?;
    let after: i64 = q
        .after
        .as_deref()
        .unwrap_or("0")
        .parse()
        .map_err(|_| AppError::bad("同步游标无效"))?;
    let rows = sqlx::query(
        "SELECT i.id,i.event, s.kind='ai' AS ai_sender FROM cloud_crypto_inbox i
        JOIN accounts s ON s.id=substring(i.event->>'sender' from 2 for 36)::uuid
        WHERE i.account_id=$1 AND i.device_id=$2 AND i.id>$3 ORDER BY i.id LIMIT 256",
    )
    .bind(a.id)
    .bind(&q.device_id)
    .bind(after)
    .fetch_all(&mut *tx)
    .await?;
    let next = rows.last().map_or(after, |r| r.get("id"));
    let mut events = Vec::with_capacity(rows.len());
    let mut audiences = std::collections::HashMap::new();
    for row in rows {
        let mut event: Value = row.get("event");
        if row.get::<bool, _>("ai_sender") && event["type"] == "m.room.encrypted" {
            let route = event.as_object_mut().unwrap().remove("llmn_route");
            match route {
                None if a.node.is_none() => continue, // Legacy unscoped AI envelopes cannot be classified.
                Some(route) if route.is_object() => {
                    let ai = account(field(&event, "sender")?)?;
                    if let std::collections::hash_map::Entry::Vacant(entry) = audiences.entry(ai) {
                        let admin: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_ai_admins WHERE ai_id=$1 AND administrator=$2)")
                            .bind(ai).bind(a.id).fetch_one(&mut *tx).await?;
                        let current = if admin {
                            crate::crypto_resources::profile_audience_revision(&mut tx, ai).await?
                        } else {
                            Value::Null
                        };
                        entry.insert(current);
                    }
                    if audiences[&ai] != route["audience"]
                        || !chat_scope_current(&mut tx, ai, &route["chats"]).await?
                    {
                        continue;
                    }
                }
                _ => {}
            }
        }
        events.push(event);
    }
    let keys = counts(&mut tx, a.id, &q.device_id).await?;
    let unused:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_crypto_one_time_keys WHERE account_id=$1 AND device_id=$2 AND fallback AND NOT used)").bind(a.id).bind(&q.device_id).fetch_one(&mut *tx).await?;
    let handover = if a.node.is_some() {
        sqlx::query_scalar::<_, String>(
            "SELECT handover_state FROM cloud_ai_bindings WHERE ai_id=$1",
        )
        .bind(a.id)
        .fetch_one(&mut *tx)
        .await?
    } else {
        "active".into()
    };
    let directory_version: i64 =
        sqlx::query_scalar("SELECT revision FROM cloud_crypto_device_directory WHERE singleton")
            .fetch_one(&mut *tx)
            .await?;
    let directory_version = directory_version.to_string();
    let active_backup:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('version',version,'publicKey',auth_data->>'public_key') FROM cloud_crypto_backups WHERE account_id=$1 AND NOT sealed ORDER BY created_at DESC LIMIT 1").bind(a.id).fetch_optional(&mut *tx).await?;
    let retired = active_backup.is_none()
        && sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT FROM cloud_crypto_backups WHERE account_id=$1 AND sealed)",
        )
        .bind(a.id)
        .fetch_one(&mut *tx)
        .await?;
    let (authorized, fingerprint) = authority::record(&mut tx, a.id, &q.device_id).await?;
    let changed: Vec<Uuid> = if q.device_lists_version.as_deref() == Some(&directory_version) {
        Vec::new()
    } else {
        sqlx::query_scalar("SELECT id FROM accounts WHERE status='active'")
            .fetch_all(&mut *tx)
            .await?
    };
    Ok(Json(
        json!({"llmn_backup_retired":retired,"llmn_authorized":authorized,"llmn_fingerprint":fingerprint,"llmn_active_backup":active_backup,"llmn_device_lists_version":directory_version,"llmn_handover_state":handover,"next_batch":next.to_string(),"to_device":{"events":events},"device_lists":{"changed":changed.into_iter().map(matrix_user).collect::<Vec<_>>(),"left":[]},"device_one_time_keys_count":keys,"device_unused_fallback_key_types":if unused{vec!["signed_curve25519"]}else{vec![]}}),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceQuery {
    account_id: Option<Uuid>,
}
#[derive(Deserialize)]
struct BackupVersion {
    version: Option<Uuid>,
}
async fn backup_read(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Query(q): Query<BackupVersion>,
) -> Result<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    let a = actor(&s, &h, &mut tx).await?;
    let r = sqlx::query(
        "SELECT * FROM cloud_crypto_backups WHERE account_id=$1 AND ($2::uuid IS NULL OR version=$2) ORDER BY created_at DESC LIMIT 1",
    )
    .bind(a.id).bind(q.version)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(AppError::not_found)?;
    Ok(Json(
        json!({"sealed":r.get::<bool,_>("sealed"),"version":r.get::<Uuid,_>("version"),"algorithm":r.get::<String,_>("algorithm"),"auth_data":r.get::<Value,_>("auth_data")}),
    ))
}
async fn backup_save(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    nodes::lock(&mut tx).await?;
    let a = actor(&s, &h, &mut tx).await?;
    device(&mut tx, &a, field(&v, "deviceId")?).await?;
    authority::command(&mut tx, &a, "backup", &v).await?;
    publish_backup(&mut tx, &a, &v).await?;
    crate::handover::changed(&mut tx, a.id).await?;
    tx.commit().await?;
    Ok(Json(json!({"version":v["version"]})))
}
async fn publish_backup(tx: &mut Tx<'_>, a: &Actor, v: &Value) -> Result<()> {
    let version = uid(field(&v, "version")?)?;
    let algorithm = field(&v, "algorithm")?;
    if algorithm != "m.megolm_backup.v1.curve25519-aes-sha2" || !v["auth_data"].is_object() {
        return Err(AppError::bad("历史密钥备份格式无效"));
    }
    let current = sqlx::query(
        "SELECT * FROM cloud_crypto_backups WHERE account_id=$1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(a.id)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(r) = &current
        && r.get::<Uuid, _>("version") == version
    {
        if r.get::<String, _>("algorithm") != algorithm
            || r.get::<Value, _>("auth_data") != v["auth_data"]
        {
            return Err(AppError::conflict("同一备份版本不能替换公钥或认证数据"));
        }
        return Ok(());
    }
    if current
        .as_ref()
        .map(|r| r.get::<Uuid, _>("version").to_string())
        .as_deref()
        != v.get("expectedVersion").and_then(Value::as_str)
    {
        return Err(AppError::conflict("历史密钥备份版本已变化"));
    }
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM cloud_crypto_backups WHERE account_id=$1 AND version=$2)",
    )
    .bind(a.id)
    .bind(version)
    .fetch_one(&mut **tx)
    .await?
    {
        return Err(AppError::conflict("不能恢复旧备份版本"));
    }
    if a.node.is_none() {
        let key = field(&v["auth_data"], "public_key")?;
        authority::backup_key(key)?;
        let used:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_crypto_backups WHERE account_id=$1 AND rtrim(auth_data->>'public_key','=')=$2)").bind(a.id).bind(key).fetch_one(&mut **tx).await?;
        if used {
            return Err(AppError::conflict("新备份须使用全新的备份密钥"));
        }
    }
    sqlx::query("UPDATE cloud_crypto_backups SET sealed=true WHERE account_id=$1")
        .bind(a.id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO cloud_crypto_backups(account_id,version,algorithm,auth_data) VALUES($1,$2,$3,$4)").bind(a.id).bind(version).bind(algorithm).bind(&v["auth_data"]).execute(&mut **tx).await?;
    Ok(())
}
async fn backup_keys(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Query(q): Query<BackupVersion>,
) -> Result<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    let a = actor(&s, &h, &mut tx).await?;
    let version:Uuid=sqlx::query_scalar("SELECT version FROM cloud_crypto_backups WHERE account_id=$1 AND ($2::uuid IS NULL OR version=$2) ORDER BY created_at DESC LIMIT 1").bind(a.id).bind(q.version).fetch_optional(&mut *tx).await?.ok_or_else(AppError::not_found)?;
    let rows=sqlx::query("SELECT room_id,session_id,session_data FROM cloud_crypto_backup_sessions WHERE account_id=$1 AND version=$2").bind(a.id).bind(version).fetch_all(&mut *tx).await?;
    let mut out = json!({"rooms":{}});
    for r in rows {
        let room: String = r.get(0);
        let session: String = r.get(1);
        if out["rooms"].get(&room).is_none() {
            out["rooms"][&room] = json!({"sessions":{}});
        }
        out["rooms"][&room]["sessions"][session] = r.get::<Value, _>(2);
    }
    Ok(Json(out))
}
async fn recovery_read(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Query(q): Query<BackupVersion>,
) -> Result<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    let a = actor(&s, &h, &mut tx).await?;
    let r = sqlx::query("SELECT version,ciphertext FROM cloud_crypto_recovery WHERE account_id=$1 AND $2::uuid IS NULL UNION ALL SELECT version,ciphertext FROM cloud_crypto_recovery_history WHERE account_id=$1 AND version=$2::uuid::text")
        .bind(a.id).bind(q.version)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(AppError::not_found)?;
    Ok(Json(
        json!({"version":r.get::<String,_>(0),"ciphertext":r.get::<String,_>(1)}),
    ))
}
async fn recovery_save(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    use base64::Engine;
    let version = field(&v, "version")?;
    uid(version)?;
    let ciphertext = field(&v, "ciphertext")?;
    if ciphertext.is_empty()
        || base64::engine::general_purpose::STANDARD
            .decode(ciphertext)
            .is_err()
    {
        return Err(AppError::bad("恢复密文无效"));
    }
    let mut tx = s.pool.begin().await?;
    nodes::lock(&mut tx).await?;
    let a = actor(&s, &h, &mut tx).await?;
    device(&mut tx, &a, field(&v, "deviceId")?).await?;
    authority::command(&mut tx, &a, "recovery", &v).await?;
    if a.node.is_none() {
        authority::validate_key(&v)?;
    }
    let old = sqlx::query(
        "SELECT version,ciphertext,enrollment_key FROM cloud_crypto_recovery WHERE account_id=$1",
    )
    .bind(a.id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(r) = &old
        && r.get::<String, _>("version") == version
    {
        if r.get::<String, _>("ciphertext") != ciphertext
            || r.get::<Option<String>, _>("enrollment_key").as_deref()
                != v["enrollmentKey"].as_str()
        {
            return Err(AppError::conflict("同一恢复版本不能替换密文"));
        }
        return Ok(Json(json!({"version":version})));
    }
    if old
        .as_ref()
        .map(|r| r.get::<String, _>("version"))
        .as_deref()
        != v.get("expectedVersion").and_then(Value::as_str)
    {
        return Err(AppError::conflict("恢复资料版本已变化"));
    }
    if sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM cloud_crypto_recovery_history WHERE account_id=$1 AND version=$2)").bind(a.id).bind(version).fetch_one(&mut *tx).await?{return Err(AppError::conflict("不能恢复旧的恢复资料版本"));}
    if let Some(backup) = v.get("newBackup") {
        publish_backup(&mut tx, &a, backup).await?;
    }
    if a.node.is_none() {
        let active:Option<String>=sqlx::query_scalar("SELECT version::text FROM cloud_crypto_backups WHERE account_id=$1 AND NOT sealed ORDER BY created_at DESC LIMIT 1").bind(a.id).fetch_optional(&mut *tx).await?;
        if active.is_none() || active.as_deref() != v["backupVersion"].as_str() {
            return Err(AppError::conflict("备份已封存，请重新生成恢复文件"));
        }
    }
    sqlx::query("INSERT INTO cloud_crypto_recovery_history VALUES($1,$2,$3)")
        .bind(a.id)
        .bind(version)
        .bind(ciphertext)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO cloud_crypto_recovery(account_id,version,ciphertext,enrollment_key) VALUES($1,$2,$3,$4) ON CONFLICT(account_id) DO UPDATE SET version=$2,ciphertext=$3,enrollment_key=$4").bind(a.id).bind(version).bind(ciphertext).bind(v["enrollmentKey"].as_str()).execute(&mut *tx).await?;
    crate::handover::changed(&mut tx, a.id).await?;
    tx.commit().await?;
    Ok(Json(json!({"version":version})))
}
pub(crate) fn encryption(r: &sqlx::postgres::PgRow) -> Value {
    if r.get::<bool, _>("crypto_enabled") {
        json!({"version":1,"activationPosition":r.get::<Option<i64>,_>("crypto_from_position").unwrap_or(0).to_string(),"historyComplete":r.get::<bool,_>("crypto_history_complete")})
    } else {
        Value::Null
    }
}
pub(crate) async fn enabled(tx: &mut Tx<'_>, cid: Uuid) -> Result<bool> {
    Ok(
        sqlx::query_scalar("SELECT crypto_enabled FROM cloud_conversations WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?,
    )
}
pub(crate) async fn plaintext(tx: &mut Tx<'_>, cid: Uuid) -> Result<()> {
    if enabled(tx, cid).await? {
        return Err(required());
    }
    Ok(())
}
pub(crate) fn required() -> AppError {
    let mut e = AppError::conflict("该私聊必须使用端到端密文，不能回退明文");
    e.code = "encryption_required";
    e
}
pub(crate) async fn envelope(
    tx: &mut Tx<'_>,
    a: &Actor,
    room: &str,
    event: &str,
    sealed: &connector::crypto::SealedEnvelope,
) -> Result<()> {
    if sealed.device_keys["ed25519"]
        .as_str()
        .is_none_or(str::is_empty)
        || sealed.device_keys["curve25519"]
            .as_str()
            .is_none_or(str::is_empty)
        || sealed.signatures.as_object().is_none_or(|v| v.is_empty())
    {
        return Err(AppError::bad("密文缺少设备钥或签名"));
    }
    if sealed.version != 1
        || sealed.room_id != room
        || sealed.sender != matrix_user(a.id)
        || sealed.event_id != event
        || sealed.content["algorithm"] != "m.megolm.v1.aes-sha2"
        || sealed.content["ciphertext"]
            .as_str()
            .is_none_or(str::is_empty)
    {
        return Err(AppError::bad("密文路由或格式无效"));
    }
    device(tx, a, &sealed.device_id).await
}
#[allow(clippy::too_many_arguments)] // Authenticated message routing is one protocol boundary.
pub(crate) async fn content(
    tx: &mut Tx<'_>,
    cid: Uuid,
    a: &Actor,
    mid: Uuid,
    revision: i64,
    sequence: Option<u64>,
    text: &str,
    sealed: Option<&connector::crypto::SealedEnvelope>,
) -> Result<()> {
    let on = enabled(tx, cid).await?;
    match sealed {
        Some(e) if on && text.is_empty() => {
            envelope(
                tx,
                a,
                &format!("!{cid}:llmn"),
                &format!(
                    "${mid}.{revision}.{}",
                    sequence.map_or_else(|| "final".into(), |s| s.to_string())
                ),
                e,
            )
            .await
        }
        Some(_) => Err(required()),
        None if on => Err(required()),
        None => Ok(()),
    }
}
pub(crate) async fn check_revision(
    tx: &mut Tx<'_>,
    cid: Uuid,
    expected: Option<&str>,
) -> Result<i64> {
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM cloud_conversations WHERE id=$1")
        .bind(cid)
        .fetch_one(&mut **tx)
        .await?;
    if expected != Some(revision.to_string().as_str()) {
        return Err(AppError::conflict("窗口版本已变化"));
    }
    Ok(revision)
}
async fn enable(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Path(cid): Path<Uuid>,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    nodes::lock(&mut tx).await?;
    let a = actor(&s, &h, &mut tx).await?;
    if a.node.is_some() {
        return Err(AppError::forbidden());
    }
    device(&mut tx, &a, field(&v, "deviceId")?).await?;
    let rid = uid(field(&v, "requestId")?)?;
    let fingerprint = digest(&json!(["enable", cid, v]));
    let members = chat::member(&mut tx, cid, a.id).await?;
    chat::writable(&mut tx, cid).await?;
    let c = sqlx::query("SELECT * FROM cloud_conversations WHERE id=$1")
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
    let group = c.get::<String, _>("kind") == "group";
    if group {
        let role: String = sqlx::query_scalar(
            "SELECT role FROM cloud_members WHERE conversation=$1 AND account_id=$2",
        )
        .bind(cid)
        .bind(a.id)
        .fetch_one(&mut *tx)
        .await?;
        if !matches!(role.as_str(), "owner" | "admin") {
            return Err(AppError::forbidden());
        }
    }
    if !group && crate::groups::direct_blocked(&mut tx, cid).await? {
        return Err(AppError::forbidden());
    }
    if let Some(row) =
        sqlx::query("SELECT fingerprint FROM cloud_receipts WHERE owner=$1 AND request=$2")
            .bind(a.id)
            .bind(rid)
            .fetch_optional(&mut *tx)
            .await?
    {
        if row.get::<String, _>("fingerprint") != fingerprint {
            return Err(AppError::conflict("请求ID已用于其他内容"));
        }
        return Ok(Json(chat::conversation(&mut tx, cid, a.id).await?));
    }
    check_revision(&mut tx, cid, v["expectedConversationRevision"].as_str()).await?;
    if !c.get::<bool, _>("crypto_enabled") {
        let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_node_tasks WHERE conversation=$1 AND state IN ('queued','running','uncertain'))").bind(cid).fetch_one(&mut *tx).await?;
        if busy {
            return Err(AppError::conflict("窗口仍有未完成任务"));
        }
        let contacts:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_contact_intents WHERE conversation=$1 AND status IN ('pending','claimed','executing','uncertain'))").bind(cid).fetch_one(&mut *tx).await?;
        if contacts {
            return Err(AppError::conflict("启用加密前请先取消该窗口的约定联系"));
        }
        for member in members {
            let kind: String = sqlx::query_scalar("SELECT kind FROM accounts WHERE id=$1")
                .bind(member)
                .fetch_one(&mut *tx)
                .await?;
            if kind == "ai" {
                let capable:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_ai_bindings WHERE ai_id=$1 AND capabilities ? 'sealedV1' AND connection_id IS NOT NULL)").bind(member).fetch_one(&mut *tx).await?;
                if !capable {
                    return Err(AppError::conflict("AI节点尚未具备密文能力"));
                }
            }
            let keys:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cloud_crypto_devices d WHERE account_id=$1 AND NOT revoked AND device_keys IS NOT NULL AND (node_id IS NULL OR EXISTS(SELECT 1 FROM cloud_ai_bindings b WHERE b.ai_id=d.account_id AND b.node_id=d.node_id AND b.version=d.binding_version)))").bind(member).fetch_one(&mut *tx).await?;
            if !keys {
                return Err(AppError::conflict("双方尚未准备加密设备"));
            }
        }
        sqlx::query("UPDATE cloud_conversations SET crypto_enabled=true,crypto_from_position=(SELECT coalesce(max(position),0) FROM cloud_messages WHERE conversation=$1),revision=revision+1 WHERE id=$1").bind(cid).execute(&mut *tx).await?;
        let members = chat::member(&mut tx, cid, a.id).await?;
        chat::notify(&mut tx, cid, members).await?;
    }
    let out = chat::conversation(&mut tx, cid, a.id).await?;
    sqlx::query(
        "INSERT INTO cloud_receipts(owner,request,fingerprint,response) VALUES($1,$2,$3,$4)",
    )
    .bind(a.id)
    .bind(rid)
    .bind(fingerprint)
    .bind(json!({"id":cid}))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(out))
}
#[cfg(test)]
#[path = "crypto_tests.rs"]
mod tests;
