// SPDX-License-Identifier: AGPL-3.0-only

//! WebAuthn ceremonies. Only opaque, random handles leave the server: serialized
//! library state remains in the configured shared lookup store, never in cookies.
use super::{authenticate::Authenticator, oauth::request_origin};
use common::{
    KV_PASSKEY, Server,
    auth::{AccessToken, AuthRequest},
    cache::invalidate::CacheInvalidationBuilder,
    ipc::CacheInvalidation,
};
use directory::Credentials;
use encodify::base64::URL_SAFE_NO_PAD;
use http_proto::{HttpRequest, HttpResponse, HttpSessionData, JsonResponse, ToHttpResponse};
use hyper::{Method, header::ORIGIN};
use registry::{
    schema::{
        enums::Permission,
        prelude::{Object, ObjectType, UTCDateTime},
        structs::{Account, Credential, PasskeyCredential, UserAccount},
    },
    types::id::ObjectId,
};
use serde::{Deserialize, Serialize};
use store::{
    dispatch::lookup::KeyValue,
    registry::write::{RegistryWrite, RegistryWriteResult},
    write::now,
};
use webauthn_rs::prelude::*;
use webauthn_rs_proto::ResidentKeyRequirement;

const CEREMONY_TTL: u64 = 300;
const STEP_UP_TTL: u64 = 60;
const MAX_PASSKEYS: usize = 20;
const MAX_BODY: usize = 65_536;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(super) enum LoginContext {
    Code { query: String },
    Device { code: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "purpose")]
enum Ceremony {
    Registration {
        account_id: u32,
        stamp: String,
        name: String,
        user_handle: Uuid,
        state: PasskeyRegistration,
    },
    Authentication {
        account_id: Option<u32>,
        stamp: String,
        credential_version: u64,
        context: Option<LoginContext>,
        state: DiscoverableAuthentication,
    },
    StepUp {
        account_id: u32,
        stamp: String,
    },
}
#[derive(Debug, Serialize, Deserialize)]
struct Pending {
    origin: String,
    expires_at: u64,
    ceremony: Ceremony,
}

impl Pending {
    fn validate(self, origin: &Url, now: u64) -> trc::Result<Ceremony> {
        if self.expires_at <= now || self.origin != origin.origin().ascii_serialization() {
            return Err(denied());
        }
        Ok(self.ceremony)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManageRequest {
    #[serde(default)]
    name: String,
    password: Option<String>,
    mfa_token: Option<String>,
    authorization: Option<String>,
    challenge_id: Option<String>,
    credential: Option<serde_json::Value>,
    credential_id: Option<String>,
}

fn bad_request(message: &'static str) -> trc::Error {
    trc::ResourceEvent::BadParameters
        .into_err()
        .details(message)
}
fn denied() -> trc::Error {
    trc::AuthEvent::Failed
        .into_err()
        .details("Passkey verification failed. Start again and use a registered passkey.")
}
fn json<T: Serialize>(value: T) -> HttpResponse {
    JsonResponse::new(value)
        .into_http_response()
        .with_no_store()
}
fn encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}
fn stamp(user: &UserAccount) -> String {
    let mut hasher = store::blake3::Hasher::new();
    hasher.update(
        &serde_json::to_vec(&user.password_credential())
            .expect("password credential contains only serializable fields"),
    );
    hasher.update(&user.credential_revision.to_le_bytes());
    hasher.finalize().to_hex().to_string()
}
fn passkeys(user: &UserAccount) -> impl Iterator<Item = &PasskeyCredential> {
    user.credentials.iter().filter_map(|c| {
        if let Credential::Passkey(c) = c {
            Some(c)
        } else {
            None
        }
    })
}
fn security_epoch(user: &mut UserAccount) {
    user.credential_revision = store::rand::random::<u64>().max(1);
}

/// Public-origin configuration is authoritative. Only loopback localhost has the
/// WebAuthn development exception; arbitrary Host/Forwarded values never set RP ID.
fn relying_party(
    server: &Server,
    req: &HttpRequest,
    session: &HttpSessionData,
) -> trc::Result<(Webauthn, Url)> {
    let incoming = request_origin(req, session);
    let local = Url::parse(&format!("{}://{}", incoming.scheme(), incoming.authority))
        .map_err(|_| denied())?;
    let origin = effective_origin(
        &server.core.network.http.url_https,
        &local,
        session.remote_ip,
    )?;
    verify_origin(
        &origin,
        req.headers()
            .get(ORIGIN)
            .map(|h| h.to_str().unwrap_or_default()),
        req.method() == Method::POST,
    )?;
    let rp_id = origin.host_str().ok_or_else(denied)?;
    let webauthn = WebauthnBuilder::new(rp_id, &origin)
        .map_err(|_| denied())?
        .rp_name("Stalwart")
        .timeout(std::time::Duration::from_secs(CEREMONY_TTL))
        .build()
        .map_err(|_| denied())?;
    Ok((webauthn, origin))
}

fn effective_origin(
    configured: &str,
    incoming: &Url,
    remote_ip: std::net::IpAddr,
) -> trc::Result<Url> {
    if incoming.scheme() == "http"
        && incoming.host_str() == Some("localhost")
        && remote_ip.is_loopback()
    {
        return Ok(incoming.clone());
    }
    let configured = Url::parse(configured).map_err(|_| {
        bad_request("Configure the public HTTPS server URL before enabling passkeys.")
    })?;
    // A reverse proxy may terminate TLS. Compare the authority using the public
    // HTTPS scheme so its default port remains 443, even on an internal HTTP hop.
    let mut public = incoming.clone();
    public.set_scheme("https").map_err(|_| denied())?;
    if configured.scheme() != "https"
        || public.host_str() != configured.host_str()
        || public.port_or_known_default() != configured.port_or_known_default()
    {
        return Err(denied());
    }
    Url::parse(&configured.origin().ascii_serialization()).map_err(|_| denied())
}
fn verify_origin(expected: &Url, supplied: Option<&str>, required: bool) -> trc::Result<()> {
    if supplied.is_some_and(|value| value != expected.origin().ascii_serialization())
        || (required && supplied.is_none())
    {
        return Err(denied());
    }
    Ok(())
}

async fn save_pending(
    server: &Server,
    origin: &Url,
    ceremony: Ceremony,
    ttl: u64,
) -> trc::Result<String> {
    let token = encode(&store::rand::random::<[u8; 32]>());
    let value = serde_json::to_vec(&Pending {
        origin: origin.origin().ascii_serialization(),
        expires_at: now() + ttl,
        ceremony,
    })
    .map_err(|_| denied())?;
    server
        .in_memory_store()
        .key_set(
            KeyValue::new(
                KeyValue::<()>::build_key(KV_PASSKEY, token.as_bytes()),
                value,
            )
            .expires(ttl),
        )
        .await?;
    Ok(token)
}
async fn take_pending(server: &Server, origin: &Url, token: &str) -> trc::Result<Ceremony> {
    if token.len() != 43
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(denied());
    }
    let key = KeyValue::<()>::build_key(KV_PASSKEY, token.as_bytes());
    let value = server
        .in_memory_store()
        .key_get::<String>(key.as_slice())
        .await?
        .ok_or_else(denied)?;
    // DEL is atomic for Local, Redis and the sharded lookup implementation. Only
    // its winner may verify a proof, even if two requests read the same state.
    if !server.in_memory_store().key_delete(key.as_slice()).await? {
        return Err(denied());
    }
    let pending: Pending = serde_json::from_str(&value).map_err(|_| denied())?;
    pending.validate(origin, now())
}

async fn native_account(server: &Server, id: u32) -> trc::Result<(Object, UserAccount)> {
    let object = server
        .registry()
        .get(ObjectId::new(ObjectType::Account, types::id::Id::from(id)))
        .await?
        .ok_or_else(denied)?;
    let user = Account::from(object.clone())
        .into_user()
        .ok_or_else(denied)?;
    if server
        .domain_by_id(user.domain_id.document_id())
        .await?
        .as_ref()
        .and_then(|d| server.get_directory_for_cached_domain(d))
        .is_some()
    {
        return Err(bad_request(
            "Passkeys are managed by the identity provider for this account.",
        ));
    }
    Ok((object, user))
}
async fn write_account(
    server: &Server,
    id: u32,
    old: &Object,
    user: UserAccount,
) -> trc::Result<()> {
    let object = Object::new(Account::User(user).into());
    match server
        .registry()
        .write(RegistryWrite::Update {
            object: &object,
            id: types::id::Id::from(id),
            old_object: old,
        })
        .await?
    {
        RegistryWriteResult::Success(_) => {
            let mut invalidation = CacheInvalidationBuilder::default();
            invalidation.process_update(types::id::Id::from(id), old, &object);
            invalidation.invalidate(CacheInvalidation::AccessToken(id));
            server.invalidate_caches(invalidation).await
        }
        _ => Err(denied()),
    }
}
async fn password_step_up(
    server: &Server,
    session: &HttpSessionData,
    access: &AccessToken,
    input: &ManageRequest,
) -> trc::Result<()> {
    let info = server.account_info(access.account_id()).await?;
    let authenticated = server
        .authenticate(&AuthRequest {
            credentials: Credentials::Basic {
                username: info.name().to_string(),
                secret: input
                    .password
                    .clone()
                    .ok_or_else(|| bad_request("Verify your identity before changing passkeys."))?,
                mfa_token: input.mfa_token.clone(),
            },
            remote_ip: session.remote_ip,
            session_id: session.session_id,
        })
        .await?;
    if authenticated.access_token.account_id() != access.account_id()
        || authenticated.access_token.is_secondary_credential()
    {
        return Err(denied());
    }
    Ok(())
}
async fn require_step_up(
    server: &Server,
    origin: &Url,
    session: &HttpSessionData,
    access: &AccessToken,
    user: &UserAccount,
    input: &ManageRequest,
) -> trc::Result<()> {
    if let Some(token) = &input.authorization {
        match take_pending(server, origin, token).await? {
            Ceremony::StepUp {
                account_id,
                stamp: previous,
            } if account_id == access.account_id() && previous == stamp(user) => Ok(()),
            _ => Err(denied()),
        }
    } else {
        password_step_up(server, session, access, input).await
    }
}

pub trait PasskeyApi: Sync + Send {
    fn handle_passkey_request(
        &self,
        req: &HttpRequest,
        session: &HttpSessionData,
        path: &[&str],
        body: Option<Vec<u8>>,
    ) -> impl Future<Output = trc::Result<HttpResponse>> + Send;
}
impl PasskeyApi for Server {
    async fn handle_passkey_request(
        &self,
        req: &HttpRequest,
        session: &HttpSessionData,
        path: &[&str],
        body: Option<Vec<u8>>,
    ) -> trc::Result<HttpResponse> {
        let authenticated = self.authenticate_headers(req, session).await?;
        let access = &authenticated.access_token;
        if access.is_secondary_credential() {
            return Err(denied());
        }
        let (webauthn, origin) = relying_party(self, req, session)?;
        let id = access.account_id();
        let (old, mut user) = native_account(self, id).await?;
        if req.method() == Method::GET && path.is_empty() {
            access.enforce_permission(Permission::SysAccountPasswordGet)?;
            let list = passkeys(&user).map(|c| serde_json::json!({"id": c.credential_id.to_string(), "name": c.name, "createdAt": c.created_at, "lastUsedAt": c.last_used_at, "currentOrigin": c.rp_id == origin.host_str().unwrap_or_default()})).collect::<Vec<_>>();
            return Ok(json(serde_json::json!({"list":list})));
        }
        access.enforce_permission(Permission::SysAccountPasswordUpdate)?;
        if req.method() != Method::POST {
            return Err(trc::ResourceEvent::NotFound.into_err());
        }
        let body = body
            .filter(|b| b.len() <= MAX_BODY)
            .ok_or_else(|| bad_request("Passkey request is too large."))?;
        let input: ManageRequest =
            serde_json::from_slice(&body).map_err(|_| bad_request("Invalid passkey request."))?;
        match path {
            ["verify", "begin"] => {
                let (mut options, state) = webauthn
                    .start_discoverable_authentication()
                    .map_err(|_| denied())?;
                options.mediation = None;
                let token = save_pending(
                    self,
                    &origin,
                    Ceremony::Authentication {
                        account_id: Some(id),
                        stamp: stamp(&user),
                        credential_version: self.access_token(id).await?.credential_version(),
                        context: None,
                        state,
                    },
                    CEREMONY_TTL,
                )
                .await?;
                Ok(json(
                    serde_json::json!({"challengeId":token,"options":options}),
                ))
            }
            ["verify", "finish"] => {
                let credential = serde_json::from_value(input.credential.ok_or_else(denied)?)
                    .map_err(|_| denied())?;
                let (account_id, _, context) = finish_authentication(
                    self,
                    &webauthn,
                    &origin,
                    input.challenge_id.as_deref().ok_or_else(denied)?,
                    &credential,
                    Some(id),
                )
                .await?;
                if account_id != id || context.is_some() {
                    return Err(denied());
                }
                let (_, user) = native_account(self, id).await?;
                let token = save_pending(
                    self,
                    &origin,
                    Ceremony::StepUp {
                        account_id: id,
                        stamp: stamp(&user),
                    },
                    STEP_UP_TTL,
                )
                .await?;
                Ok(json(serde_json::json!({"authorization":token})))
            }
            ["register", "begin"] => {
                require_step_up(self, &origin, session, access, &user, &input).await?;
                let name = valid_name(&input.name)?;
                if passkeys(&user).count() >= MAX_PASSKEYS {
                    return Err(bad_request(
                        "Remove an unused passkey before adding another.",
                    ));
                }
                let rp_id = origin.host_str().ok_or_else(denied)?;
                let handle = passkeys(&user)
                    .find(|c| c.rp_id == rp_id)
                    .and_then(|c| Uuid::parse_str(&c.user_handle).ok())
                    .unwrap_or_else(Uuid::new_v4);
                let exclude = passkeys(&user)
                    .filter(|c| c.rp_id == rp_id)
                    .filter_map(|c| serde_json::from_str::<Passkey>(&c.public_key).ok())
                    .map(|c| c.cred_id().clone())
                    .collect();
                let info = self.account_info(id).await?;
                let (mut options, state) = webauthn
                    .start_passkey_registration(handle, info.name(), info.name(), Some(exclude))
                    .map_err(|_| denied())?;
                // Request discoverability as well as the library's mandatory UV.
                let selection = options
                    .public_key
                    .authenticator_selection
                    .as_mut()
                    .ok_or_else(denied)?;
                selection.resident_key = Some(ResidentKeyRequirement::Required);
                selection.require_resident_key = true;
                let token = save_pending(
                    self,
                    &origin,
                    Ceremony::Registration {
                        account_id: id,
                        stamp: stamp(&user),
                        name,
                        user_handle: handle,
                        state,
                    },
                    CEREMONY_TTL,
                )
                .await?;
                Ok(json(
                    serde_json::json!({"challengeId":token,"options":options}),
                ))
            }
            ["register", "finish"] => {
                let pending = take_pending(
                    self,
                    &origin,
                    input.challenge_id.as_deref().ok_or_else(denied)?,
                )
                .await?;
                let Ceremony::Registration {
                    account_id,
                    stamp: previous,
                    name,
                    user_handle,
                    state,
                } = pending
                else {
                    return Err(denied());
                };
                if account_id != id
                    || previous != stamp(&user)
                    || passkeys(&user).count() >= MAX_PASSKEYS
                {
                    return Err(denied());
                }
                let credential: RegisterPublicKeyCredential =
                    serde_json::from_value(input.credential.ok_or_else(denied)?)
                        .map_err(|_| denied())?;
                let key = webauthn
                    .finish_passkey_registration(&credential, &state)
                    .map_err(|_| denied())?;
                let credential_key = encode(key.cred_id());
                if passkeys(&user).any(|c| {
                    c.rp_id == origin.host_str().unwrap_or_default()
                        && c.credential_key == credential_key
                }) {
                    return Err(bad_request("This passkey is already registered."));
                }
                let record = PasskeyCredential {
                    credential_id: user.next_credential_id().into(),
                    name,
                    rp_id: origin.host_str().unwrap_or_default().into(),
                    user_handle: user_handle.to_string(),
                    credential_key,
                    public_key: serde_json::to_string(&key).map_err(|_| denied())?,
                    created_at: UTCDateTime::now(),
                    last_used_at: None,
                };
                let credential_id = record.credential_id.to_string();
                user.credentials.push(Credential::Passkey(record));
                write_account(self, id, &old, user).await?;
                Ok(json(serde_json::json!({"id":credential_id})))
            }
            ["remove"] => {
                require_step_up(self, &origin, session, access, &user, &input).await?;
                let credential_id = input.credential_id.ok_or_else(denied)?;
                let before = passkeys(&user).count();
                let values = user.credentials.values().filter(|c| !matches!(c, Credential::Passkey(p) if p.credential_id.to_string() == credential_id)).cloned().collect::<Vec<_>>();
                if values.len() == user.credentials.len() {
                    return Err(trc::ResourceEvent::NotFound.into_err());
                }
                if before == 1 && user.password().is_none_or(str::is_empty) {
                    return Err(bad_request(
                        "Add another sign-in method before removing your last passkey.",
                    ));
                }
                user.credentials = values.into_iter().collect();
                security_epoch(&mut user);
                write_account(self, id, &old, user).await?;
                Ok(json(serde_json::json!({"removed":true})))
            }
            ["rename"] => {
                let name = valid_name(&input.name)?;
                let credential_id = input.credential_id.ok_or_else(denied)?;
                let found = user
                    .credentials
                    .values_mut()
                    .find_map(|c| {
                        if let Credential::Passkey(c) = c {
                            (c.credential_id.to_string() == credential_id).then_some(c)
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| trc::ResourceEvent::NotFound.into_err())?;
                found.name = name;
                write_account(self, id, &old, user).await?;
                Ok(json(serde_json::json!({"updated":true})))
            }
            _ => Err(trc::ResourceEvent::NotFound.into_err()),
        }
    }
}
fn valid_name(name: &str) -> trc::Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
        return Err(bad_request(
            "Enter a passkey name between 1 and 100 characters.",
        ));
    }
    Ok(name.to_string())
}

pub(super) async fn begin_login(
    server: &Server,
    req: &HttpRequest,
    session: &HttpSessionData,
    username: &str,
    context: LoginContext,
) -> trc::Result<serde_json::Value> {
    let (webauthn, origin) = relying_party(server, req, session)?;
    let account_id = if username.len() <= 320 {
        server.account_id_from_email(username.trim(), false).await?
    } else {
        None
    };
    let user = if let Some(id) = account_id {
        native_account(server, id).await.ok().map(|(_, u)| u)
    } else {
        None
    };
    let account_id = account_id.filter(|_| user.is_some());
    let (mut options, state) = webauthn
        .start_discoverable_authentication()
        .map_err(|_| denied())?;
    options.mediation = None;
    let token = save_pending(
        server,
        &origin,
        Ceremony::Authentication {
            account_id,
            stamp: user.as_ref().map(stamp).unwrap_or_default(),
            credential_version: if let Some(id) = account_id {
                server.access_token(id).await?.credential_version()
            } else {
                0
            },
            context: Some(context),
            state,
        },
        CEREMONY_TTL,
    )
    .await?;
    // Unknown users receive the same options shape. No allowCredentials or user
    // handle is disclosed until the authenticator produces a verified assertion.
    Ok(serde_json::json!({"type":"passkeyOptions","challengeId":token,"options":options}))
}
pub(super) async fn finish_login(
    server: &Server,
    req: &HttpRequest,
    session: &HttpSessionData,
    token: &str,
    credential: &PublicKeyCredential,
) -> trc::Result<(u32, u64, LoginContext)> {
    let (webauthn, origin) = relying_party(server, req, session)?;
    let (id, version, context) =
        finish_authentication(server, &webauthn, &origin, token, credential, None).await?;
    let context = context.ok_or_else(denied)?;
    let access = AccessToken::new(server.access_token(id).await?, session.remote_ip)?;
    access.enforce_permission(Permission::Authenticate)?;
    if access.credential_version() != version {
        return Err(denied());
    }
    trc::event!(
        Auth(trc::AuthEvent::Success),
        AccountId = id,
        SpanId = session.session_id,
        Details = "Authenticated with passkey"
    );
    Ok((id, version, context))
}
async fn finish_authentication(
    server: &Server,
    webauthn: &Webauthn,
    origin: &Url,
    token: &str,
    assertion: &PublicKeyCredential,
    management_account: Option<u32>,
) -> trc::Result<(u32, u64, Option<LoginContext>)> {
    let pending = take_pending(server, origin, token).await?;
    let Ceremony::Authentication {
        account_id: Some(id),
        stamp: previous,
        credential_version,
        context,
        state,
    } = pending
    else {
        return Err(denied());
    };
    if match management_account {
        Some(owner) => owner != id || context.is_some(),
        None => context.is_none(),
    } {
        return Err(denied());
    }
    let (handle, credential_key) = webauthn
        .identify_discoverable_authentication(assertion)
        .map_err(|_| denied())?;
    let rp_id = origin.host_str().ok_or_else(denied)?;
    let key_id = encode(credential_key);
    // Read current registry data for revocation checks, not a potentially stale
    // challenge snapshot. Recheck after every optimistic-concurrency conflict.
    for _ in 0..3 {
        let (old, mut user) = native_account(server, id).await?;
        if stamp(&user) != previous {
            return Err(denied());
        }
        let entry = user
            .credentials
            .values_mut()
            .find_map(|c| {
                if let Credential::Passkey(c) = c {
                    (c.rp_id == rp_id
                        && c.credential_key == key_id
                        && c.user_handle == handle.to_string())
                    .then_some(c)
                } else {
                    None
                }
            })
            .ok_or_else(denied)?;
        let mut key: Passkey = serde_json::from_str(&entry.public_key).map_err(|_| denied())?;
        let result = webauthn
            .finish_discoverable_authentication_v2(assertion, state.clone(), &key)
            .map_err(|_| denied())?;
        key.update_credential(&result);
        entry.public_key = serde_json::to_string(&key).map_err(|_| denied())?;
        entry.last_used_at = Some(UTCDateTime::now());
        match write_account(server, id, &old, user).await {
            Ok(()) => return Ok((id, credential_version, context)),
            Err(e) if e.is_assertion_failure() => continue,
            Err(e) => return Err(e),
        }
    }
    Err(denied())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_origin_is_exact_and_configuration_is_authoritative() {
        let external = "203.0.113.10".parse().unwrap();
        let configured = "https://mail.example.org/admin";
        for url in ["https://mail.example.org", "http://mail.example.org"] {
            assert_eq!(
                effective_origin(configured, &Url::parse(url).unwrap(), external)
                    .unwrap()
                    .as_str(),
                "https://mail.example.org/"
            );
        }
        for url in [
            "https://attacker.invalid",
            "https://sub.mail.example.org",
            "https://mail.example.org:8443",
            "http://localhost:8080",
        ] {
            assert!(effective_origin(configured, &Url::parse(url).unwrap(), external).is_err());
        }
        let origin = Url::parse("https://mail.example.org").unwrap();
        for value in [
            None,
            Some("null"),
            Some("https://mail.example.org/"),
            Some("https://mail.example.org:8443"),
            Some("https://attacker.invalid"),
        ] {
            assert!(verify_origin(&origin, value, true).is_err());
        }
        assert!(verify_origin(&origin, Some("https://mail.example.org"), true).is_ok());
    }
    #[test]
    fn http_development_exception_requires_localhost_and_loopback() {
        let ip = "127.0.0.1".parse().unwrap();
        let local = Url::parse("http://localhost:3000").unwrap();
        assert_eq!(effective_origin("", &local, ip).unwrap(), local);
        assert!(effective_origin("", &Url::parse("http://127.0.0.1:3000").unwrap(), ip).is_err());
        assert!(
            effective_origin(
                "http://example.org",
                &Url::parse("http://example.org").unwrap(),
                ip
            )
            .is_err()
        );
    }
    #[test]
    fn stored_state_checks_expiry_and_origin_independently_of_store_ttl() {
        let origin = Url::parse("https://mail.example.org").unwrap();
        let make = || Pending {
            origin: "https://mail.example.org".into(),
            expires_at: 100,
            ceremony: Ceremony::StepUp {
                account_id: 1,
                stamp: "stamp".into(),
            },
        };
        assert!(make().validate(&origin, 99).is_ok());
        assert!(make().validate(&origin, 100).is_err());
        assert!(make().validate(&origin, 101).is_err());
        assert!(
            make()
                .validate(&Url::parse("https://mail.example.org:8443").unwrap(), 99)
                .is_err()
        );
    }
    #[test]
    fn names_are_trimmed_and_bounded_without_control_characters() {
        assert_eq!(valid_name("  Work laptop  ").unwrap(), "Work laptop");
        for name in [
            "".to_string(),
            "  ".into(),
            "bad\nname".into(),
            "x".repeat(101),
        ] {
            assert!(valid_name(&name).is_err());
        }
    }
}
