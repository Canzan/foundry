//! instance-integration-setup — `foundry doctor rotate-secrets` over a stored
//! secrets key (ADR-IIS-001 decision 8, ADR-IIS-004 rule 1): run as the
//! binary against real Postgres, it re-wraps the secrets key under the
//! current instance key and re-seals the remaining direct seals under the
//! secrets key, all-or-nothing.

use foundry_app::integrations::credential_slot;
use foundry_app::integrations::keys::{plan_generate, plan_rewrap};
use foundry_auth::secret_box::{SealedSecret, SecretBox};
use foundry_store::integrations::{KeyWrapper, StoredSecretsKey};
use foundry_store::{run_migrations, Store};
use secrecy::{ExposeSecret, SecretString};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;
use uuid::Uuid;

const OLD_KEY: &str = "aWlzLWFkbWluLWNsaS1yb3RhdGUtb2xkLWtleS0zMmI";
const NEW_KEY: &str = "ZWlwLXBoYXNlNS1yb3RhdGVkLW5ldy1rZXktMzJieXQ";
const UNRELATED_KEY: &str = "ZWlwLXBoYXNlNS11bnJlbGF0ZWQta2V5LTMyYnl0ZXM";

fn instance_box(key: &str) -> SecretBox {
    SecretBox::from_settings(&SecretString::from(key), None).expect("a key")
}

/// `foundry doctor rotate-secrets` with `current` (and `previous`) as the
/// instance key settings: its exit code, stdout and stderr.
fn rotate(database_url: &str, current: &str, previous: Option<&str>) -> (i32, String, String) {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_foundry"));
    command
        .args(["doctor", "rotate-secrets"])
        .env("DATABASE_URL", database_url)
        .env("FOUNDRY_INSTANCE_KEY", current)
        .env_remove("FOUNDRY_SECRETS_KEY")
        .env_remove("FOUNDRY_SECRETS_KEY_PREVIOUS")
        .env_remove("FOUNDRY_INSTANCE_KEY_PREVIOUS");
    if let Some(previous) = previous {
        command.env("FOUNDRY_INSTANCE_KEY_PREVIOUS", previous);
    }
    let out = command.output().expect("run the foundry binary");
    (
        out.status.code().unwrap_or(-100),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

async fn connection(pool: &PgPool, ws: Uuid, kind: &str, id: Uuid, sealed: &[u8]) {
    sqlx::query(
        "INSERT INTO integration_connections
                (id, workspace_id, kind, state, external_id, label,
                 sealed_credential, credential_saved_at)
         VALUES ($1, $2, $3, 'connected', $4, 'Canzan Labs', $5, now())",
    )
    .bind(id)
    .bind(ws)
    .bind(kind)
    .bind(id.to_string())
    .bind(sealed)
    .execute(pool)
    .await
    .expect("connection");
}

async fn stored_key(pool: &PgPool) -> StoredSecretsKey {
    StoredSecretsKey::read_for_test_seam(pool)
        .await
        .expect("read")
        .expect("a stored secrets key")
}

async fn credential(pool: &PgPool, id: Uuid) -> Vec<u8> {
    sqlx::query_scalar("SELECT sealed_credential FROM integration_connections WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("credential")
}

#[tokio::test]
async fn admin_cli_rotate_secrets_rewraps_the_secrets_key_and_reseals_direct_seals() {
    let pg = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let url = format!(
        "postgres://postgres:postgres@{}:{}/postgres",
        pg.get_host().await.expect("host"),
        pg.get_host_port_ipv4(5432).await.expect("port")
    );
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("connect pool");
    run_migrations(&pool).await.expect("run migrations");
    let store = Store::from_pool(pool.clone());

    // A secrets key wrapped under the old instance key; one credential
    // sealed directly under the old key, one under the secrets key.
    let old = instance_box(OLD_KEY);
    let plan = plan_generate(&old, KeyWrapper::File, None).expect("plan");
    store
        .store_secrets_key(
            &plan.key,
            &plan.canary,
            time::OffsetDateTime::now_utc(),
            |_, _| Ok::<_, String>(None),
        )
        .await
        .expect("store the secrets key");
    let ws = Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Canzan Labs')")
        .bind(ws)
        .execute(&pool)
        .await
        .expect("workspace");
    let (direct, sealed) = (Uuid::now_v7(), Uuid::now_v7());
    let secret = SecretString::from("xoxb-canzan-labs");
    let direct_seal = old.seal(&secret, &credential_slot(direct)).expect("seal");
    connection(&pool, ws, "slack", direct, direct_seal.as_bytes()).await;
    let under_old = old.seal(&secret, &credential_slot(sealed)).expect("seal");
    let under_secrets = plan
        .reseal(&old, &credential_slot(sealed), under_old.as_bytes())
        .expect("sealed under the secrets key");
    connection(&pool, ws, "matrix", sealed, &under_secrets).await;
    let key_before = stored_key(&pool).await;
    let direct_before = credential(&pool, direct).await;

    // No held key opens the secrets key: nothing changes.
    let (code, _, stderr) = tokio::task::spawn_blocking({
        let url = url.clone();
        move || rotate(&url, UNRELATED_KEY, None)
    })
    .await
    .expect("join");
    assert_eq!(code, 4, "{stderr}");
    assert!(stderr.contains("nothing changed"), "{stderr}");
    assert!(
        stored_key(&pool).await == key_before,
        "the key is unchanged"
    );
    assert_eq!(credential(&pool, direct).await, direct_before);

    // The new key, the old one previous: re-wrapped and re-sealed.
    let (code, stdout, stderr) = tokio::task::spawn_blocking({
        let url = url.clone();
        move || rotate(&url, NEW_KEY, Some(OLD_KEY))
    })
    .await
    .expect("join");
    assert_eq!(code, 0, "{stderr}");
    let secrets_key_id = &plan.key.key_id;
    assert!(
        stdout.starts_with(&format!(
            "rotate-secrets: 1 credential(s) sealed again under key {secrets_key_id}"
        )),
        "{stdout}"
    );
    let new = instance_box(NEW_KEY);
    let rewrapped = stored_key(&pool).await;
    assert_eq!(rewrapped.key_id, *secrets_key_id);
    assert_eq!(
        rewrapped.wrapped_by_key_id,
        new.current_key_id().to_string()
    );
    assert_eq!(rewrapped.wrapped_by, KeyWrapper::Operator);
    let (secrets, again) =
        plan_rewrap(&new, &rewrapped, None).expect("the new key alone opens the secrets key");
    assert!(again.is_none(), "wrapped under the current key");
    for id in [direct, sealed] {
        let stored = SealedSecret::from_bytes(credential(&pool, id).await);
        assert_eq!(stored.key_id(), secrets.current_key_id(), "{id}");
        let opened = secrets.open(&stored, &credential_slot(id)).expect("opens");
        assert_eq!(opened.expose_secret(), "xoxb-canzan-labs");
    }
    assert_eq!(
        credential(&pool, sealed).await,
        under_secrets,
        "left as it was"
    );
}
