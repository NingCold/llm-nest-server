use super::*;
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};
use ed25519_dalek::{Signature, VerifyingKey};

fn decode<const N: usize>(value: &str) -> Result<[u8; N]> {
    STANDARD_NO_PAD
        .decode(value)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(AppError::forbidden)
}

pub(super) fn backup_key(encoded: &str) -> Result<()> {
    let bytes = decode::<32>(encoded).map_err(|_| AppError::bad("备份公钥格式无效"))?;
    if bytes[31] & 0x80 != 0 {
        return Err(AppError::bad("备份公钥格式无效"));
    }
    Ok(())
}

pub(super) fn verify(
    key: &str,
    account: Uuid,
    operation: &str,
    body: &Value,
    signature: &str,
) -> Result<()> {
    let mut payload = json!([account, operation, body]);
    payload.sort_all_objects();
    let statement = format!("llmn.authority.v1\n{payload}");
    verify_bytes(key, statement.as_bytes(), signature)
}

fn verify_bytes(key: &str, statement: &[u8], signature: &str) -> Result<()> {
    let verifier = VerifyingKey::from_bytes(&decode(key)?).map_err(|_| AppError::forbidden())?;
    verifier
        .verify_strict(statement, &Signature::from_bytes(&decode(signature)?))
        .map_err(|_| AppError::forbidden())
}

// Matrix directory objects exclude unsigned metadata; application envelopes sign every field except signatures.
pub(super) fn unsigned(value: &Value, directory: bool) -> Result<Value> {
    let mut out = value.clone();
    let object = out
        .as_object_mut()
        .ok_or_else(|| AppError::bad("签名对象无效"))?;
    object.remove("signatures");
    if directory {
        object.remove("unsigned");
    }
    out.sort_all_objects();
    Ok(out)
}
pub(super) fn signature_on(body: &Value, key: &str, signature: &Value) -> Result<()> {
    verify_bytes(
        key,
        &serde_json::to_vec(body).map_err(|_| AppError::bad("签名正文无效"))?,
        signature.as_str().ok_or_else(AppError::forbidden)?,
    )
}
pub(super) fn signer_key<'a>(
    key: &'a Value,
    usage: &str,
    account: Uuid,
) -> Result<(&'a str, &'a str)> {
    if key["user_id"] != matrix_user(account) || key["usage"] != json!([usage]) {
        return Err(AppError::bad("交叉签名钥用途无效"));
    }
    let keys = key["keys"]
        .as_object()
        .filter(|k| k.len() == 1)
        .ok_or_else(|| AppError::bad("交叉签名钥格式无效"))?;
    let (id, key) = keys.iter().next().unwrap();
    let public = key.as_str().ok_or_else(AppError::forbidden)?;
    decode::<32>(public)?;
    if id.strip_prefix("ed25519:") != Some(public) {
        return Err(AppError::bad("交叉签名钥标识无效"));
    }
    Ok((id, public))
}
pub(super) fn check_curve(encoded: &str) -> Result<()> {
    decode::<32>(encoded).map(|_| ())
}

pub(super) async fn verify_envelope(
    tx: &mut Tx<'_>,
    a: &Actor,
    sealed: &connector::crypto::SealedEnvelope,
) -> Result<()> {
    let d = &sealed.device_id;
    device(tx, a, d).await?;
    let r=sqlx::query("SELECT d.authorized,d.device_keys,k.master_key FROM cloud_crypto_devices d JOIN cloud_crypto_signing_keys k USING(account_id) WHERE d.account_id=$1 AND d.device_id=$2").bind(a.id).bind(d).fetch_optional(&mut **tx).await?.ok_or_else(AppError::forbidden)?;
    if a.node.is_none() && !r.get::<bool, _>("authorized") {
        return Err(AppError::forbidden());
    }
    let keys: Value = r
        .get::<Option<Value>, _>("device_keys")
        .ok_or_else(AppError::forbidden)?;
    let ed_id = format!("ed25519:{d}");
    let ed = field(&keys["keys"], &ed_id)?;
    let curve = field(&keys["keys"], &format!("curve25519:{d}"))?;
    if sealed.device_keys["ed25519"] != ed
        || sealed.device_keys["curve25519"] != curve
        || sealed.content["device_id"] != *d
        || sealed.content["sender_key"] != curve
    {
        return Err(AppError::forbidden());
    }
    let master = r
        .get::<Option<Value>, _>("master_key")
        .ok_or_else(AppError::forbidden)?;
    let (master_id, root) = signer_key(&master, "master", a.id)?;
    let body = unsigned(&json!(sealed), false)?;
    let signatures = &sealed.signatures[matrix_user(a.id)];
    signature_on(&body, ed, &signatures[&ed_id])?;
    signature_on(&body, root, &signatures[master_id])
}
pub(super) async fn record(
    tx: &mut Tx<'_>,
    account: Uuid,
    id: &str,
) -> Result<(bool, Option<String>)> {
    let r=sqlx::query("SELECT authorized AND NOT revoked AND node_id IS NULL, device_keys->'keys'->>('ed25519:'||device_id) FROM cloud_crypto_devices WHERE account_id=$1 AND device_id=$2")
        .bind(account).bind(id).fetch_optional(&mut **tx).await?;
    Ok(r.map_or((false, None), |row| (row.get(0), row.get(1))))
}

pub(super) async fn proof(
    tx: &mut Tx<'_>,
    a: &Actor,
    id: &str,
    action: &str,
    body: &Value,
    sig: Option<&str>,
    must_be_authorized: bool,
) -> Result<()> {
    if a.node.is_some() {
        return Ok(());
    }
    device(tx, a, id).await?;
    let (trusted, key) = record(tx, a.id, id).await?;
    if must_be_authorized && !trusted {
        return Err(AppError::forbidden());
    }
    verify(
        key.as_deref().ok_or_else(AppError::forbidden)?,
        a.id,
        action,
        &json!([id, body]),
        sig.ok_or_else(AppError::forbidden)?,
    )
}

pub(crate) async fn command(tx: &mut Tx<'_>, a: &Actor, action: &str, v: &Value) -> Result<()> {
    let mut body = v.clone();
    let obj = body.as_object_mut().ok_or_else(AppError::forbidden)?;
    let sig = obj.remove("proof");
    let id = field(&body, "deviceId")?;
    proof(
        tx,
        a,
        id,
        action,
        &body,
        sig.as_ref().and_then(Value::as_str),
        true,
    )
    .await
}

pub(super) async fn status(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    let a = actor(&s, &h, &mut tx).await?;
    if a.node.is_some() {
        return Err(AppError::forbidden());
    }
    let (authorized, fingerprint) = record(&mut tx, a.id, &id).await?;
    Ok(Json(
        json!({"accountId":a.id,"deviceId":id,"authorized":authorized,"fingerprint":fingerprint}),
    ))
}

pub(super) async fn enroll(
    State(s): State<Arc<AppState>>,
    h: HeaderMap,
    Json(v): Json<Value>,
) -> Result<Json<Value>> {
    let mut tx = s.pool.begin().await?;
    nodes::lock(&mut tx).await?;
    let a = actor(&s, &h, &mut tx).await?;
    if a.node.is_some() {
        return Err(AppError::forbidden());
    }
    let target = field(&v, "target")?;
    device(&mut tx, &a, target).await?;
    let (_, key) = record(&mut tx, a.id, target).await?;
    if key.as_deref() != Some(field(&v, "fingerprint")?) {
        return Err(AppError::forbidden());
    }
    if let Some(version) = v["recoveryVersion"].as_str() {
        // The file signs the target device. Its SDK also proves possession of that device's key.
        if field(&v, "deviceId")? != target {
            return Err(AppError::forbidden());
        }
        let recovery: Option<String> = sqlx::query_scalar(
            "SELECT enrollment_key FROM cloud_crypto_recovery WHERE account_id=$1 AND version=$2",
        )
        .bind(a.id)
        .bind(version)
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
        let grant = json!([version, target, key]);
        verify(
            recovery.as_deref().ok_or_else(AppError::forbidden)?,
            a.id,
            "recover",
            &grant,
            field(&v, "recoveryProof")?,
        )?;
        proof(
            &mut tx,
            &a,
            target,
            "accept",
            &grant,
            v["proof"].as_str(),
            false,
        )
        .await?;
    } else {
        command(&mut tx, &a, "enroll", &v).await?;
    }
    sqlx::query(
        "UPDATE cloud_crypto_devices SET authorized=true WHERE account_id=$1 AND device_id=$2",
    )
    .bind(a.id)
    .bind(target)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(json!({})))
}

pub(super) fn validate_key(v: &Value) -> Result<()> {
    VerifyingKey::from_bytes(&decode(field(v, "enrollmentKey")?)?)
        .map_err(|_| AppError::bad("恢复授权公钥无效"))?;
    Ok(())
}
