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
    let verifier = VerifyingKey::from_bytes(&decode(key)?).map_err(|_| AppError::forbidden())?;
    verifier
        .verify_strict(
            statement.as_bytes(),
            &Signature::from_bytes(&decode(signature)?),
        )
        .map_err(|_| AppError::forbidden())
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

pub(super) async fn command(tx: &mut Tx<'_>, a: &Actor, action: &str, v: &Value) -> Result<()> {
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
