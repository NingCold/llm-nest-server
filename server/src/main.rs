use cloud_server::db as sqlx;
use cloud_server::{AppState, auth};
use std::{path::PathBuf, sync::Arc};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("启动失败：{error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("CLOUD_DATABASE_URL").map_err(|_| "缺少 CLOUD_DATABASE_URL")?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(12)
        .connect(&url)
        .await
        .map_err(|_| "无法连接 PostgreSQL")?;
    sqlx::migrator()
        .run(&pool)
        .await
        .map_err(|_| "数据库升级失败")?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("invite") => {
            let days = args.get(1).map(|s| s.parse()).transpose()?.unwrap_or(7);
            println!("{}", auth::create_invite(&pool, days).await?);
            return Ok(());
        }
        Some("revoke-invite") => {
            auth::revoke_invite(&pool, args.get(1).ok_or("缺少邀请码")?).await?;
            println!("邀请已撤销");
            return Ok(());
        }
        Some(_) => {
            return Err("用法：cloud-server [invite [有效天数] | revoke-invite 邀请码]".into());
        }
        None => {}
    }
    let public_url =
        public_origin(&std::env::var("CLOUD_PUBLIC_URL").map_err(|_| "缺少 CLOUD_PUBLIC_URL")?)?;
    let attachments = PathBuf::from(
        std::env::var("CLOUD_ATTACHMENTS_DIR").map_err(|_| "缺少 CLOUD_ATTACHMENTS_DIR")?,
    );
    std::fs::create_dir_all(&attachments)?;
    let attachments = attachments.canonicalize()?;
    let web_dir = PathBuf::from(std::env::var("CLOUD_WEB_DIR").unwrap_or_else(|_| "web".into()))
        .canonicalize()?;
    if !web_dir.join("index.html").is_file() {
        return Err("请先构建 Web 界面".into());
    }
    let mail = auth::Mailer::from_env()?;
    if attachments.starts_with(&web_dir) || web_dir.starts_with(&attachments) {
        return Err("附件与网页文件须分开存放".into());
    }
    if let Some(outbox) = mail.outbox_path() {
        let outbox = outbox.canonicalize()?;
        if outbox.starts_with(&web_dir)
            || web_dir.starts_with(&outbox)
            || outbox.starts_with(&attachments)
            || attachments.starts_with(&outbox)
        {
            return Err("测试收件箱须使用独立私有目录".into());
        }
    }
    let bind = std::env::var("CLOUD_BIND").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    cloud_server::nodes::recover(&pool).await?;
    println!("云端聊天服务已启动：{bind}");
    let maintenance = tokio::spawn(cloud_server::nodes::maintain(pool.clone()));
    let require_encryption = match std::env::var("CLOUD_REQUIRE_ENCRYPTION") {
        Ok(v) => match v.as_str() {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return Err("CLOUD_REQUIRE_ENCRYPTION须为true或false".into()),
        },
        Err(std::env::VarError::NotPresent) => false,
        Err(_) => return Err("CLOUD_REQUIRE_ENCRYPTION编码无效".into()),
    };
    let state = Arc::new(AppState {
        require_encryption,
        relay: Default::default(),
        pool,
        attachments,
        public_url,
        mail,
    });
    let result = axum::serve(listener, cloud_server::router(state, web_dir))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
    maintenance.abort();
    let _ = maintenance.await;
    result?;
    Ok(())
}

/// Serialize exactly the origin browsers use in their Origin header.
fn public_origin(value: &str) -> Result<String, &'static str> {
    let invalid =
        "CLOUD_PUBLIC_URL 须为 HTTPS 站点来源，不含路径、账号或查询参数；本机开发可用 HTTP";
    let url = url::Url::parse(value).map_err(|_| invalid)?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if !(url.scheme() == "https" || url.scheme() == "http" && loopback)
        || url.host().is_none()
        || url.path() != "/"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid);
    }
    Ok(url.origin().ascii_serialization())
}

#[cfg(test)]
mod tests {
    use super::public_origin;

    #[test]
    fn matches_the_origin_sent_by_browsers() {
        assert_eq!(
            public_origin("https://Chat.EXAMPLE:443/"),
            Ok("https://chat.example".into())
        );
        assert_eq!(
            public_origin("http://[::1]:8080"),
            Ok("http://[::1]:8080".into())
        );
        for bad in [
            "http://chat.example",
            "https://chat.example/app",
            "https://chat.example/?v=1",
            "https://chat.example/#/auth",
            "https://user@chat.example",
        ] {
            assert!(public_origin(bad).is_err(), "{bad}");
        }
    }
}
